//! Per-tab threads, held only in memory. A thread follows its tab across
//! navigation: same-site moves add a divider and make old citations inert;
//! cross-site moves also start the model's context afresh, so one site's text
//! never steers answers about another. Bounded per tab and in tab count.
use std::collections::{HashMap, VecDeque};

use crate::citations::CitationBook;
use crate::ipc::{
    PageAssistChangedV1 as Changed, PageAssistEntryV1 as Entry, PageAssistEventV1 as Event,
    PageAssistIntentV1 as Intent, PageAssistPartKindV1 as PartKind, PageAssistPartV1 as Part,
    PageAssistSuggestionV1 as Suggestion, PageAssistTurnStateV1 as TurnState,
    PageAssistTurnV1 as TurnView, PAGE_ASSIST_MAX_EXCERPT_CHARS,
};
use crate::prompt::HistoryTurn;
use crate::text;

/// Most turns a tab keeps; the oldest go first.
pub const MAX_TURNS: usize = 24;
/// Most tabs with a thread; the least recently used goes first.
pub const MAX_TABS: usize = 64;
/// Summaries kept for unchanged pages.
pub const MAX_CACHED_SUMMARIES: usize = 32;

/// One turn and what only Rust keeps for it.
#[derive(Clone, Debug)]
pub struct TurnRecord {
    /// What the panel shows.
    pub view: TurnView,
    /// Its numbered sources.
    pub book: CitationBook,
}

#[derive(Clone, Debug)]
enum Slot {
    Turn(Box<TurnRecord>),
    Divider { site: String, cross_site: bool },
}

/// One tab's thread.
#[derive(Clone, Debug)]
pub struct Thread {
    site: String,
    slots: Vec<Slot>,
    /// Turns from this slot on are the model's context.
    context_start: usize,
    next_turn: u32,
    seq: u32,
    used: u64,
}

impl Thread {
    fn new(site: &str) -> Self {
        Self {
            site: site.to_owned(),
            slots: Vec::new(),
            context_start: 0,
            next_turn: 1,
            seq: 0,
            used: 0,
        }
    }

    /// The registrable site the thread's latest page is on.
    pub fn site(&self) -> &str {
        &self.site
    }

    /// Whether any turn is still working or waiting on the person.
    pub fn busy(&self) -> bool {
        self.turns().any(|record| {
            matches!(
                record.view.state,
                TurnState::Working | TurnState::Waiting | TurnState::Paused
            )
        })
    }

    fn turns(&self) -> impl Iterator<Item = &TurnRecord> {
        self.slots.iter().filter_map(|slot| match slot {
            Slot::Turn(record) => Some(record.as_ref()),
            Slot::Divider { .. } => None,
        })
    }

    /// Opens a turn and returns its `started` event.
    pub fn begin(
        &mut self,
        tab: &str,
        intent: Intent,
        suggestion: Option<Suggestion>,
        question: &str,
        selection: Option<&str>,
    ) -> Changed {
        let turn = self.next_turn;
        self.next_turn = self.next_turn.saturating_add(1);
        let view = TurnView {
            turn,
            intent,
            suggestion,
            question: text::clean(question),
            selection: selection
                .map(|selection| {
                    text::bounded(&text::clean(selection), PAGE_ASSIST_MAX_EXCERPT_CHARS)
                })
                .filter(|selection| !selection.is_empty()),
            state: TurnState::Working,
            phase: None,
            parts: Vec::new(),
            steps: Vec::new(),
            style: None,
            offer: None,
            cost: None,
            fault: None,
            inert: false,
            seq: self.seq.saturating_add(1),
        };
        self.slots.push(Slot::Turn(Box::new(TurnRecord {
            view: view.clone(),
            book: CitationBook::default(),
        })));
        self.trim();
        self.emit(
            tab,
            turn,
            Event::Started {
                turn: Box::new(view),
            },
        )
        .unwrap_or_else(|| Changed {
            tab: tab.to_owned(),
            turn,
            seq: self.seq,
            kind: Event::Invalidated,
        })
    }

    fn trim(&mut self) {
        while self.turns().count() > MAX_TURNS {
            let Some(first) = self
                .slots
                .iter()
                .position(|slot| matches!(slot, Slot::Turn(_)))
            else {
                return;
            };
            // A divider left leading the thread separates it from nothing.
            let end = self.slots[first + 1..]
                .iter()
                .position(|slot| matches!(slot, Slot::Turn(_)))
                .map_or(self.slots.len(), |next| first + 1 + next);
            self.slots.drain(..end);
            self.context_start = self.context_start.saturating_sub(end);
        }
    }

    /// The turn `turn`, when the thread still holds it.
    pub fn record(&self, turn: u32) -> Option<&TurnRecord> {
        self.turns().find(|record| record.view.turn == turn)
    }

    /// The turn `turn`, mutable, for its sources.
    pub fn record_mut(&mut self, turn: u32) -> Option<&mut TurnRecord> {
        self.slots.iter_mut().find_map(|slot| match slot {
            Slot::Turn(record) if record.view.turn == turn => Some(record.as_mut()),
            _ => None,
        })
    }

    /// Folds `kind` into turn `turn` and returns the event to send, with the
    /// tab's next sequence number. None when the turn is gone.
    pub fn emit(&mut self, tab: &str, turn: u32, kind: Event) -> Option<Changed> {
        let seq = self.seq.saturating_add(1);
        let record = self.record_mut(turn)?;
        fold(&mut record.view, &kind);
        record.view.seq = seq;
        self.seq = seq;
        Some(Changed {
            tab: tab.to_owned(),
            turn,
            seq,
            kind,
        })
    }

    /// The tab committed a navigation to `site`. Marks every turn inert, adds
    /// a divider and, across sites, starts the model's context afresh.
    pub fn navigated(&mut self, tab: &str, site: &str) -> Changed {
        let cross_site = site != self.site;
        for slot in &mut self.slots {
            if let Slot::Turn(record) = slot {
                record.view.inert = true;
            }
        }
        self.slots.push(Slot::Divider {
            site: site.to_owned(),
            cross_site,
        });
        if cross_site {
            self.context_start = self.slots.len();
        }
        self.site = site.to_owned();
        self.seq = self.seq.saturating_add(1);
        Changed {
            tab: tab.to_owned(),
            turn: self.next_turn.saturating_sub(1),
            seq: self.seq,
            kind: Event::Divider {
                site: site.to_owned(),
                cross_site,
            },
        }
    }

    /// Earlier finished turns in the model's current context, oldest first.
    pub fn history(&self, before: u32) -> Vec<HistoryTurn> {
        self.slots[self.context_start.min(self.slots.len())..]
            .iter()
            .filter_map(|slot| match slot {
                Slot::Turn(record)
                    if record.view.turn < before
                        && record.view.state == TurnState::Done
                        && !record.view.parts.is_empty() =>
                {
                    Some(HistoryTurn {
                        question: if record.view.question.is_empty() {
                            record
                                .view
                                .suggestion
                                .map(crate::suggest::request_text)
                                .unwrap_or_default()
                                .to_owned()
                        } else {
                            record.view.question.clone()
                        },
                        answer: protocol_lines(&record.view.parts),
                    })
                }
                _ => None,
            })
            .collect()
    }

    /// The whole thread as the panel shows it.
    pub fn entries(&self) -> Vec<Entry> {
        self.slots
            .iter()
            .map(|slot| match slot {
                Slot::Turn(record) => Entry::Turn {
                    turn: Box::new(record.view.clone()),
                },
                Slot::Divider { site, cross_site } => Entry::Divider {
                    site: site.clone(),
                    cross_site: *cross_site,
                },
            })
            .collect()
    }
}

/// Applies one event to a turn's snapshot, so a status read after any event
/// matches what the panel built from the events.
pub fn fold(view: &mut TurnView, event: &Event) {
    match event {
        Event::Started { .. } | Event::Divider { .. } | Event::Invalidated => {}
        Event::Status { phase } => view.phase = Some(*phase),
        Event::Part { part } => view.parts.push(part.clone()),
        Event::Step { step } => {
            match view.steps.iter_mut().find(|s| s.index == step.index) {
                Some(existing) => *existing = step.clone(),
                None => view.steps.push(step.clone()),
            }
            view.state = if step.state == crate::ipc::PageAssistStepStateV1::Confirm {
                TurnState::Waiting
            } else {
                TurnState::Working
            };
        }
        Event::Style { style } => {
            view.state = if style.state == crate::ipc::PageStyleStateV1::Proposed {
                TurnState::Waiting
            } else {
                view.state
            };
            view.style = Some(style.clone());
        }
        Event::Offer { offer } => view.offer = Some(*offer),
        Event::Paused { .. } => view.state = TurnState::Paused,
        Event::Done { cost } => {
            view.state = TurnState::Done;
            view.phase = None;
            view.cost = cost.clone();
        }
        Event::Cost { cost } => view.cost = Some(cost.clone()),
        Event::Failed { fault } => {
            view.state = TurnState::Failed;
            view.phase = None;
            view.fault = Some(*fault);
        }
        Event::Cancelled => {
            view.state = TurnState::Cancelled;
            view.phase = None;
        }
    }
}

/// An answer's parts as protocol lines, citations left out.
pub fn protocol_lines(parts: &[Part]) -> String {
    let mut lines = String::new();
    for part in parts {
        let line = match (part.kind, &part.label) {
            (PartKind::Answer, _) => format!("= {}", part.text),
            (PartKind::Point, _) => format!("- {}", part.text),
            (PartKind::Figure, Some(label)) => format!("# {label} :: {}", part.text),
            (PartKind::Figure, None) => format!("- {}", part.text),
            (PartKind::Caveat, _) => format!("! {}", part.text),
        };
        lines.push_str(&line);
        lines.push('\n');
    }
    lines
}

/// Every tab's thread, least recently used dropped beyond [`MAX_TABS`].
#[derive(Debug, Default)]
pub struct Threads {
    tabs: HashMap<String, Thread>,
    clock: u64,
}

impl Threads {
    /// The tab's thread, created on the first ask.
    pub fn open(&mut self, tab: &str, site: &str) -> &mut Thread {
        self.clock += 1;
        if !self.tabs.contains_key(tab) && self.tabs.len() >= MAX_TABS {
            let idle = self
                .tabs
                .iter()
                .filter(|(_, thread)| !thread.busy())
                .min_by_key(|(_, thread)| thread.used)
                .or_else(|| self.tabs.iter().min_by_key(|(_, thread)| thread.used))
                .map(|(tab, _)| tab.clone());
            if let Some(idle) = idle {
                self.tabs.remove(&idle);
            }
        }
        let clock = self.clock;
        let thread = self
            .tabs
            .entry(tab.to_owned())
            .or_insert_with(|| Thread::new(site));
        thread.used = clock;
        thread
    }

    /// The tab's thread, if it has one.
    pub fn get(&self, tab: &str) -> Option<&Thread> {
        self.tabs.get(tab)
    }

    /// The tab's thread, mutable, marking it used.
    pub fn get_mut(&mut self, tab: &str) -> Option<&mut Thread> {
        self.clock += 1;
        let clock = self.clock;
        let thread = self.tabs.get_mut(tab)?;
        thread.used = clock;
        Some(thread)
    }

    /// Ends the tab's thread (tab closed or cleared).
    pub fn remove(&mut self, tab: &str) -> Option<Thread> {
        self.tabs.remove(tab)
    }

    /// Ends every thread (profile switch, quit, AI off).
    pub fn clear(&mut self) {
        self.tabs.clear();
    }

    /// Tabs with a thread.
    pub fn len(&self) -> usize {
        self.tabs.len()
    }

    /// Whether no tab has a thread.
    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }
}

/// Summaries of unchanged pages, by read fingerprint, newest last.
#[derive(Debug, Default)]
pub struct SummaryCache {
    entries: VecDeque<(u64, Vec<Part>, CitationBook)>,
}

impl SummaryCache {
    /// The summary of the page whose read had `fingerprint`.
    pub fn get(&self, fingerprint: u64) -> Option<(&[Part], &CitationBook)> {
        self.entries
            .iter()
            .find(|(key, ..)| *key == fingerprint)
            .map(|(_, parts, book)| (parts.as_slice(), book))
    }

    /// Remembers a finished summary.
    pub fn put(&mut self, fingerprint: u64, parts: Vec<Part>, book: CitationBook) {
        self.entries.retain(|(key, ..)| *key != fingerprint);
        if self.entries.len() >= MAX_CACHED_SUMMARIES {
            self.entries.pop_front();
        }
        self.entries.push_back((fingerprint, parts, book));
    }

    /// Forgets everything.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::{PageAssistFaultV1, PageAssistPhaseV1};

    fn part(kind: PartKind, text: &str) -> Part {
        Part {
            kind,
            text: text.into(),
            label: None,
            cites: Vec::new(),
        }
    }

    fn answered(thread: &mut Thread, question: &str) -> u32 {
        let started = thread.begin("t", Intent::Ask, None, question, None);
        let turn = started.turn;
        thread.emit(
            "t",
            turn,
            Event::Part {
                part: part(PartKind::Answer, &format!("answer to {question}")),
            },
        );
        thread.emit("t", turn, Event::Done { cost: None });
        turn
    }

    #[test]
    fn events_fold_into_the_snapshot_in_sequence() {
        let mut thread = Thread::new("example.com");
        let started = thread.begin(
            "t",
            Intent::Selection,
            None,
            " what\nis this ",
            Some("a\u{202E}b"),
        );
        assert_eq!(started.seq, 1);
        let Event::Started { turn } = &started.kind else {
            panic!()
        };
        assert_eq!(turn.question, "what is this");
        assert_eq!(turn.selection.as_deref(), Some("ab"));
        let status = thread
            .emit(
                "t",
                1,
                Event::Status {
                    phase: PageAssistPhaseV1::Read { passages: 3 },
                },
            )
            .unwrap();
        assert_eq!(status.seq, 2);
        thread.emit(
            "t",
            1,
            Event::Failed {
                fault: PageAssistFaultV1::Unreadable,
            },
        );
        let view = &thread.record(1).unwrap().view;
        assert_eq!(view.state, TurnState::Failed);
        assert_eq!(view.phase, None);
        assert_eq!(view.seq, 3);
        assert!(thread.emit("t", 99, Event::Cancelled).is_none());
    }

    #[test]
    fn same_site_navigation_keeps_context_and_cross_site_resets_it() {
        let mut thread = Thread::new("a.com");
        answered(&mut thread, "one");
        let divider = thread.navigated("t", "a.com");
        assert_eq!(
            divider.kind,
            Event::Divider {
                site: "a.com".into(),
                cross_site: false
            }
        );
        assert!(thread.record(1).unwrap().view.inert);
        let second = answered(&mut thread, "two");
        assert_eq!(thread.history(second + 1).len(), 2);
        let divider = thread.navigated("t", "b.com");
        assert_eq!(
            divider.kind,
            Event::Divider {
                site: "b.com".into(),
                cross_site: true
            }
        );
        let third = answered(&mut thread, "three");
        let history = thread.history(third + 1);
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].question, "three");
        assert!(thread.history(third).is_empty());
        assert_eq!(thread.entries().len(), 5);
        assert_eq!(thread.site(), "b.com");
    }

    #[test]
    fn turns_are_capped_oldest_first_with_their_dividers() {
        let mut thread = Thread::new("a.com");
        answered(&mut thread, "first");
        thread.navigated("t", "b.com");
        for i in 0..MAX_TURNS {
            answered(&mut thread, &format!("q{i}"));
        }
        let entries = thread.entries();
        assert_eq!(entries.len(), MAX_TURNS);
        assert!(entries
            .iter()
            .all(|entry| matches!(entry, Entry::Turn { .. })));
        assert_eq!(thread.history(u32::MAX).len(), MAX_TURNS);
        assert!(thread.record(1).is_none());
    }

    #[test]
    fn tabs_are_capped_least_recently_used_first_sparing_busy_ones() {
        let mut threads = Threads::default();
        threads
            .open("busy", "a.com")
            .begin("busy", Intent::Ask, None, "q", None);
        for i in 0..MAX_TABS - 1 {
            threads.open(&format!("tab{i}"), "a.com");
        }
        assert_eq!(threads.len(), MAX_TABS);
        threads.get_mut("tab0");
        threads.open("new", "a.com");
        assert_eq!(threads.len(), MAX_TABS);
        assert!(
            threads.get("busy").is_some(),
            "a working turn is never dropped"
        );
        assert!(threads.get("tab0").is_some());
        assert!(threads.get("tab1").is_none());
        threads.clear();
        assert!(threads.is_empty());
    }

    #[test]
    fn history_replays_protocol_without_citations() {
        let lines = protocol_lines(&[
            part(PartKind::Answer, "A"),
            Part {
                label: Some("Price".into()),
                ..part(PartKind::Figure, "$5")
            },
            part(PartKind::Caveat, "Not on this page: x"),
        ]);
        assert_eq!(lines, "= A\n# Price :: $5\n! Not on this page: x\n");
    }

    #[test]
    fn summary_cache_is_bounded_and_keyed_by_fingerprint() {
        let mut cache = SummaryCache::default();
        for key in 0..(MAX_CACHED_SUMMARIES as u64 + 5) {
            cache.put(
                key,
                vec![part(PartKind::Answer, "s")],
                CitationBook::default(),
            );
        }
        assert!(cache.get(0).is_none());
        assert!(cache.get(MAX_CACHED_SUMMARIES as u64 + 4).is_some());
        cache.put(10, Vec::new(), CitationBook::default());
        assert!(cache.get(10).unwrap().0.is_empty());
    }
}
