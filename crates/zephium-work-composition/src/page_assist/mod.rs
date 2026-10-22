//! The Ask panel's service: one thread per tab in memory, a fresh read of
//! the live tab for every turn, one model call, and whole answer blocks as
//! paced events. Every effect on the page (steps, styles) waits for the
//! person where it commits anything, and everything stops on navigation,
//! tab close, AI off or Stop.
#![deny(clippy::print_stderr, clippy::print_stdout, clippy::dbg_macro)]

mod fault;
pub mod port;
pub use zephium_page_assist as core;
mod readout;
mod turn;

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use tokio::sync::{oneshot, Notify};
use zephium_core::ids::ProfileId;
use zephium_ipc::page_assist::*;
use zephium_page_assist::page::PageTraits;
use zephium_page_assist::policy::{self, TabFacts as PolicyFacts};
use zephium_page_assist::thread::{SummaryCache, Threads};
use zephium_page_assist::{suggest, text};

use port::*;

const VERSION: u16 = 1;
/// Most styles one site keeps.
pub const MAX_SITE_STYLES: usize = 8;
/// Most validated CSS one site keeps across its styles, in bytes, so its
/// stored row stays inside a settings value.
pub const MAX_SITE_CSS_BYTES: usize = 48 * 1024;

/// The ports the service works through.
#[derive(Clone)]
pub struct Ports {
    /// The live tab.
    pub page: Arc<dyn PagePort>,
    /// The person's Page model.
    pub models: Arc<dyn ModelSource>,
    /// The panel's event channel.
    pub events: Arc<dyn EventSink>,
    /// A monotonic clock.
    pub clock: Arc<dyn Clock>,
    /// The shell's switches.
    pub settings: Arc<dyn Settings>,
    /// Starts Work.
    pub work: Arc<dyn WorkHandoff>,
    /// Page-style persistence.
    pub styles: Arc<dyn StyleStore>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CancelReason {
    Stopped,
    Navigated,
    Gone,
}

#[derive(Default)]
struct Cancel {
    reason: Mutex<Option<CancelReason>>,
    notify: Notify,
}

impl Cancel {
    fn cancel(&self, reason: CancelReason) {
        let mut slot = self.reason.lock().unwrap_or_else(|p| p.into_inner());
        if slot.is_none() {
            *slot = Some(reason);
        }
        drop(slot);
        self.notify.notify_waiters();
    }

    fn reason(&self) -> Option<CancelReason> {
        *self.reason.lock().unwrap_or_else(|p| p.into_inner())
    }

    async fn cancelled(&self) -> CancelReason {
        loop {
            let notified = self.notify.notified();
            if let Some(reason) = self.reason() {
                return reason;
            }
            notified.await;
        }
    }
}

/// The person's answer to a waiting confirm or style.
type DecisionSender = oneshot::Sender<PageAssistDecisionV1>;

struct Running {
    turn: u32,
    cancel: Arc<Cancel>,
    decision: Option<DecisionSender>,
    resume: Option<oneshot::Sender<()>>,
    /// Steps are running: human input in the tab pauses them.
    leased: Arc<AtomicBool>,
    took_over: Arc<AtomicBool>,
    /// The turn's own step is navigating on purpose.
    expects_navigation: bool,
}

#[derive(Default)]
struct State {
    threads: Threads,
    summaries: SummaryCache,
    running: HashMap<String, Running>,
    /// The structure of each tab's latest read, for suggestions.
    traits: HashMap<String, (u64, PageTraits)>,
    styles: HashMap<ProfileId, Vec<StoredStyle>>,
    next_style: u64,
}

struct Shared {
    state: Mutex<State>,
    ports: Ports,
}

/// The Ask panel service.
#[derive(Clone)]
pub struct PageAssist {
    shared: Arc<Shared>,
}

/// A turn's work, for the caller to spawn on its runtime.
pub type TurnTask = port::BoxFuture<'static, ()>;

fn site_of(url: Option<&str>) -> Option<String> {
    url.and_then(zephium_app::work_sites::site_of)
}

impl PageAssist {
    /// A service over `ports`.
    pub fn new(ports: Ports) -> Self {
        Self {
            shared: Arc::new(Shared {
                state: Mutex::new(State::default()),
                ports,
            }),
        }
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.shared
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn ports(&self) -> &Ports {
        &self.shared.ports
    }

    fn emit(&self, event: Option<PageAssistChangedV1>) {
        if let Some(event) = event {
            self.ports().events.emit(event);
        }
    }

    async fn availability(&self, facts: Option<&TabFacts>) -> PageAssistAvailabilityV1 {
        let Some(facts) = facts else {
            return PageAssistAvailabilityV1::Unavailable;
        };
        let site = site_of(facts.url.as_deref());
        let sensitive = site
            .as_deref()
            .is_some_and(zephium_app::work_sites::is_sensitive);
        let mut policy = PolicyFacts {
            ai_enabled: self.ports().settings.ai_enabled(&facts.profile),
            private: facts.private,
            url: facts.url.as_deref(),
            viewer: facts.viewer || site.is_none(),
            sensitive,
            loading: facts.loading,
            has_model: true,
            supported: self.ports().page.supported(),
        };
        if policy::availability(&policy) == PageAssistAvailabilityV1::Ready {
            policy.has_model = self.ports().models.entry(&facts.profile).await.is_some();
        }
        policy::availability(&policy)
    }

    /// The tab's availability, suggestions and whole thread. Served from
    /// memory, plus one read per page for suggestions and the selection.
    pub async fn status(&self, tab: &str) -> PageAssistStatusV1 {
        let facts = self.ports().page.facts(tab);
        let availability = self.availability(facts.as_ref()).await;
        let site = facts.as_ref().and_then(|f| site_of(f.url.as_deref()));
        let entries = self
            .state()
            .threads
            .get(tab)
            .map(|thread| thread.entries())
            .unwrap_or_default();
        let mut status = PageAssistStatusV1 {
            version: VERSION,
            tab: tab.to_owned(),
            availability,
            site: site.clone(),
            model: None,
            suggestions: Vec::new(),
            selection: None,
            can_act: false,
            styled: false,
            entries,
        };
        let (Some(facts), PageAssistAvailabilityV1::Ready) = (facts, availability) else {
            return status;
        };
        status.model = self
            .ports()
            .models
            .entry(&facts.profile)
            .await
            .map(|entry| PageAssistModelV1 {
                hosted: entry.model.provider == zephium_core::work::model::WorkModelProvider::Cloud,
                name: entry.display_name,
            });
        status.can_act = true;
        if let Some(site) = &site {
            status.styled = self
                .styles_of(&facts.profile)
                .iter()
                .any(|style| style.enabled && &style.site == site);
        }
        let cached = self
            .state()
            .traits
            .get(tab)
            .filter(|(epoch, _)| *epoch == facts.epoch)
            .map(|(_, traits)| *traits);
        let traits = match cached {
            Some(traits) => Some(traits),
            None => match self.ports().page.read(tab, facts.epoch).await {
                Ok(readout) if readout.epoch == facts.epoch => {
                    let read = readout::page_read(
                        site.as_deref().unwrap_or_default(),
                        &readout,
                        &zephium_agentic::contains_recognized_secret,
                    );
                    self.state()
                        .traits
                        .insert(tab.to_owned(), (facts.epoch, read.traits));
                    Some(read.traits)
                }
                _ => None,
            },
        };
        status.suggestions = suggest::suggestions(&traits.unwrap_or_default());
        status.selection = self
            .ports()
            .page
            .selection(tab, facts.epoch)
            .await
            .map(|selection| text::bounded(&text::clean(&selection), PAGE_ASSIST_MAX_EXCERPT_CHARS))
            .filter(|selection| !selection.is_empty());
        status
    }

    /// Admits a turn. On success the `started` event has been sent and the
    /// returned task, once spawned, produces the rest.
    pub async fn ask(
        &self,
        request: PageAssistAskV1,
    ) -> Result<(u32, TurnTask), PageAssistFaultV1> {
        let question = request.text.trim();
        if request.text.len() > PAGE_ASSIST_MAX_QUESTION_BYTES
            || (question.is_empty()
                && request.suggestion.is_none()
                && request.intent != PageAssistIntentV1::Summarize)
        {
            return Err(PageAssistFaultV1::Invalid);
        }
        let tab = request.tab.clone();
        let facts = self.ports().page.facts(&tab);
        let availability = self.availability(facts.as_ref()).await;
        if let Some(fault) = policy::refusal(availability) {
            return Err(fault);
        }
        let facts = facts.ok_or(PageAssistFaultV1::Unavailable)?;
        let site = site_of(facts.url.as_deref()).ok_or(PageAssistFaultV1::NotWeb)?;
        let selection = if request.intent == PageAssistIntentV1::Selection {
            let selection = self
                .ports()
                .page
                .selection(&tab, facts.epoch)
                .await
                .filter(|selection| !selection.trim().is_empty())
                .ok_or(PageAssistFaultV1::NoSelection)?;
            Some(selection)
        } else {
            None
        };
        let cancel = Arc::new(Cancel::default());
        let leased = Arc::new(AtomicBool::new(false));
        let took_over = Arc::new(AtomicBool::new(false));
        let (turn, started, divider) = {
            let mut state = self.state();
            if state.running.contains_key(&tab) {
                return Err(PageAssistFaultV1::Busy);
            }
            let thread = state.threads.open(&tab, &site);
            // A navigation the shell did not report still splits the thread.
            let divider = (thread.site() != site).then(|| thread.navigated(&tab, &site));
            let started = thread.begin(
                &tab,
                request.intent,
                request.suggestion,
                question,
                selection.as_deref(),
            );
            let turn = started.turn;
            state.running.insert(
                tab.clone(),
                Running {
                    turn,
                    cancel: cancel.clone(),
                    decision: None,
                    resume: None,
                    leased: leased.clone(),
                    took_over: took_over.clone(),
                    expects_navigation: false,
                },
            );
            (turn, started, divider)
        };
        self.emit(divider);
        self.emit(Some(started));
        let context = turn::Context {
            service: self.clone(),
            tab,
            turn,
            profile: facts.profile,
            epoch: facts.epoch,
            site,
            intent: request.intent,
            suggestion: request.suggestion,
            question: question.to_owned(),
            selection,
            cancel,
            leased,
            took_over,
        };
        Ok((turn, Box::pin(turn::run(context))))
    }

    fn running_turn<T>(
        &self,
        tab: &str,
        turn: u32,
        apply: impl FnOnce(&mut Running) -> T,
    ) -> Option<T> {
        let mut state = self.state();
        let running = state
            .running
            .get_mut(tab)
            .filter(|running| running.turn == turn)?;
        Some(apply(running))
    }

    /// Stops the tab's running turn.
    pub fn stop(&self, tab: &str) -> bool {
        let state = self.state();
        let Some(running) = state.running.get(tab) else {
            return false;
        };
        running.cancel.cancel(CancelReason::Stopped);
        true
    }

    /// Scrolls to and marks citation `n` of `turn`, while the page is the
    /// one the turn read.
    pub fn cite(&self, tab: &str, turn: u32, n: u8) -> bool {
        let Some(facts) = self.ports().page.facts(tab) else {
            return false;
        };
        let highlight = {
            let state = self.state();
            let Some(record) = state
                .threads
                .get(tab)
                .and_then(|thread| thread.record(turn))
            else {
                return false;
            };
            if record.view.inert {
                return false;
            }
            record.book.highlight(n).map(str::to_owned)
        };
        highlight.is_some_and(|text| self.ports().page.highlight(tab, facts.epoch, &text))
    }

    /// Answers the turn's waiting confirm or style.
    pub fn decide(&self, tab: &str, turn: u32, decision: PageAssistDecisionV1) -> bool {
        self.running_turn(tab, turn, |running| running.decision.take())
            .flatten()
            .is_some_and(|sender| sender.send(decision).is_ok())
    }

    /// Continues a paused turn; it reads the page again first.
    pub fn resume(&self, tab: &str, turn: u32) -> bool {
        self.running_turn(tab, turn, |running| running.resume.take())
            .flatten()
            .is_some_and(|sender| sender.send(()).is_ok())
    }

    /// Ends the tab's thread.
    pub fn clear(&self, tab: &str) -> bool {
        let mut state = self.state();
        if let Some(running) = state.running.remove(tab) {
            running.cancel.cancel(CancelReason::Gone);
        }
        state.traits.remove(tab);
        state.threads.remove(tab).is_some()
    }

    /// Starts Work with the turn's question and this tab's page.
    pub async fn handoff(&self, tab: &str, turn: u32) -> PageAssistHandoffV1 {
        let failed = |fault| PageAssistHandoffV1 {
            work: None,
            fault: Some(fault),
        };
        let Some(facts) = self.ports().page.facts(tab) else {
            return failed(PageAssistFaultV1::Unavailable);
        };
        if let Some(fault) = policy::refusal(self.availability(Some(&facts)).await) {
            return failed(fault);
        }
        let question = {
            let state = self.state();
            let Some(record) = state
                .threads
                .get(tab)
                .and_then(|thread| thread.record(turn))
            else {
                return failed(PageAssistFaultV1::Unavailable);
            };
            match (&record.view.question, record.view.suggestion) {
                (question, _) if !question.is_empty() => question.clone(),
                (_, Some(suggestion)) => suggest::request_text(suggestion).to_owned(),
                _ => return failed(PageAssistFaultV1::Invalid),
            }
        };
        let url = facts.url.unwrap_or_default();
        match self
            .ports()
            .work
            .begin(&facts.profile, tab, question, url)
            .await
        {
            Some(work) => PageAssistHandoffV1 {
                work: Some(work),
                fault: None,
            },
            None => failed(PageAssistFaultV1::Unavailable),
        }
    }

    /// The tab committed a navigation: the running turn stops (unless its
    /// own step navigated) and the thread gains a divider.
    pub fn navigated(&self, tab: &str, url: Option<&str>) {
        let site = site_of(url).unwrap_or_default();
        let divider = {
            let mut state = self.state();
            state.traits.remove(tab);
            if let Some(running) = state.running.get_mut(tab) {
                if running.expects_navigation {
                    running.expects_navigation = false;
                } else {
                    running.cancel.cancel(CancelReason::Navigated);
                }
            }
            state
                .threads
                .get_mut(tab)
                .map(|thread| thread.navigated(tab, &site))
        };
        self.emit(divider);
    }

    /// The tab closed: its thread ends.
    pub fn tab_closed(&self, tab: &str) {
        self.clear(tab);
    }

    /// The person used the tab: running steps pause.
    pub fn human_input(&self, tab: &str) {
        let state = self.state();
        if let Some(running) = state.running.get(tab) {
            if running.leased.load(Ordering::Acquire) {
                running.took_over.store(true, Ordering::Release);
            }
        }
    }

    /// Ends every thread: AI off, profile switch or quit.
    pub fn end_all(&self) {
        let invalidated: Vec<PageAssistChangedV1> = {
            let mut state = self.state();
            for running in state.running.values() {
                running.cancel.cancel(CancelReason::Gone);
            }
            let tabs: Vec<(String, u32)> = state
                .running
                .iter()
                .map(|(tab, running)| (tab.clone(), running.turn))
                .collect();
            state.running.clear();
            state.threads.clear();
            state.summaries.clear();
            state.traits.clear();
            tabs.into_iter()
                .map(|(tab, turn)| PageAssistChangedV1 {
                    tab,
                    turn,
                    seq: 0,
                    kind: PageAssistEventV1::Invalidated,
                })
                .collect()
        };
        for event in invalidated {
            self.ports().events.emit(event);
        }
    }

    /// The hosted charge of a turn is confirmed.
    pub fn settle_hosted(&self, tab: &str, turn: u32, credits: String) {
        let event = {
            let mut state = self.state();
            let Some(thread) = state.threads.get_mut(tab) else {
                return;
            };
            let model = match thread
                .record(turn)
                .and_then(|record| record.view.cost.clone())
            {
                Some(PageAssistCostV1::Hosted { model, .. }) => model,
                _ => return,
            };
            thread.emit(
                tab,
                turn,
                PageAssistEventV1::Cost {
                    cost: PageAssistCostV1::Hosted {
                        model,
                        credits: Some(credits),
                    },
                },
            )
        };
        self.emit(event);
    }

    // ------------------------------------------------------------ page styles

    fn styles_of(&self, profile: &ProfileId) -> Vec<StoredStyle> {
        let mut state = self.state();
        if !state.styles.contains_key(profile) {
            let loaded = self.ports().styles.load(profile);
            state.styles.insert(*profile, loaded);
        }
        state.styles.get(profile).cloned().unwrap_or_default()
    }

    fn apply_site(&self, profile: &ProfileId, site: &str, styles: &[StoredStyle]) {
        let css: String = styles
            .iter()
            .filter(|style| style.enabled && style.site == site)
            .map(|style| style.css.as_str())
            .collect();
        self.ports().page.apply_styles(profile, site, &css);
    }

    fn change_styles(
        &self,
        profile: &ProfileId,
        site: &str,
        change: impl FnOnce(&mut Vec<StoredStyle>) -> bool,
    ) -> bool {
        let mut styles = self.styles_of(profile);
        if !change(&mut styles) || !self.ports().styles.save(profile, &styles) {
            return false;
        }
        self.apply_site(profile, site, &styles);
        self.state().styles.insert(*profile, styles);
        true
    }

    /// Whether `site` can take one more style of `bytes` CSS.
    pub(crate) fn site_has_room(&self, profile: &ProfileId, site: &str, bytes: usize) -> bool {
        let styles = self.styles_of(profile);
        let (count, size) = styles
            .iter()
            .filter(|style| style.site == site)
            .fold((0, 0), |(count, size), style| (count + 1, size + style.css.len()));
        count < MAX_SITE_STYLES && size + bytes <= MAX_SITE_CSS_BYTES
    }

    /// Stores and applies a validated style; none when the site is full or
    /// it could not be stored.
    pub(crate) fn add_style(
        &self,
        profile: &ProfileId,
        site: &str,
        summary: &str,
        css: String,
    ) -> Option<String> {
        if !self.site_has_room(profile, site, css.len()) {
            return None;
        }
        let id = {
            let mut state = self.state();
            state.next_style += 1;
            format!("s{}-{}", self.ports().clock.now_ms(), state.next_style)
        };
        let style = StoredStyle {
            id: id.clone(),
            site: site.to_owned(),
            summary: summary.to_owned(),
            css,
            enabled: true,
        };
        self.change_styles(profile, site, |styles| {
            styles.push(style);
            true
        })
        .then_some(id)
    }

    fn site_styles(&self, profile: &ProfileId, site: &str) -> Vec<PageStyleV1> {
        self.styles_of(profile)
            .into_iter()
            .filter(|style| style.site == site)
            .map(|style| PageStyleV1 {
                id: style.id,
                summary: style.summary,
                enabled: style.enabled,
            })
            .collect()
    }

    fn tab_site(&self, tab: &str) -> Option<(ProfileId, String)> {
        let facts = self.ports().page.facts(tab)?;
        Some((facts.profile, site_of(facts.url.as_deref())?))
    }

    /// The styles of the site in `tab`.
    pub fn styles(&self, tab: &str) -> PageStylesV1 {
        let (site, styles) = match self.tab_site(tab) {
            Some((profile, site)) => {
                let styles = self.site_styles(&profile, &site);
                (Some(site), styles)
            }
            None => (None, Vec::new()),
        };
        PageStylesV1 {
            version: VERSION,
            tab: tab.to_owned(),
            site,
            styles,
        }
    }

    /// Turns one of the tab's site styles on or off.
    pub fn toggle_style(&self, tab: &str, style: &str, enabled: bool) -> PageStylesV1 {
        if let Some((profile, site)) = self.tab_site(tab) {
            self.set_style(&profile, &site, style, Some(enabled));
        }
        self.styles(tab)
    }

    /// Deletes one of the tab's site styles.
    pub fn remove_style(&self, tab: &str, style: &str) -> PageStylesV1 {
        if let Some((profile, site)) = self.tab_site(tab) {
            self.set_style(&profile, &site, style, None);
        }
        self.styles(tab)
    }

    fn set_style(&self, profile: &ProfileId, site: &str, id: &str, enabled: Option<bool>) -> bool {
        self.change_styles(profile, site, |styles| {
            let Some(index) = styles
                .iter()
                .position(|style| style.id == id && style.site == site)
            else {
                return false;
            };
            match enabled {
                Some(enabled) => styles[index].enabled = enabled,
                None => {
                    styles.remove(index);
                }
            }
            true
        })
    }

    /// Every site's styles in `profile`, for Settings.
    pub fn all_styles(&self, profile: &ProfileId) -> PageStylesAllV1 {
        let mut sites: Vec<PageSiteStylesV1> = Vec::new();
        for style in self.styles_of(profile) {
            let row = PageStyleV1 {
                id: style.id,
                summary: style.summary,
                enabled: style.enabled,
            };
            match sites.iter_mut().find(|site| site.site == style.site) {
                Some(site) => site.styles.push(row),
                None => sites.push(PageSiteStylesV1 {
                    site: style.site,
                    styles: vec![row],
                }),
            }
        }
        sites.sort_by(|a, b| a.site.cmp(&b.site));
        PageStylesAllV1 {
            version: VERSION,
            profile: profile.to_string(),
            sites,
        }
    }

    /// Turns a site's style on or off from Settings.
    pub fn toggle_style_for(
        &self,
        profile: &ProfileId,
        site: &str,
        style: &str,
        enabled: bool,
    ) -> PageStylesAllV1 {
        self.set_style(profile, site, style, Some(enabled));
        self.all_styles(profile)
    }

    /// Deletes a site's style from Settings.
    pub fn remove_style_for(
        &self,
        profile: &ProfileId,
        site: &str,
        style: &str,
    ) -> PageStylesAllV1 {
        self.set_style(profile, site, style, None);
        self.all_styles(profile)
    }

    /// The enabled styles of `site`, for a page that just loaded.
    pub fn site_css(&self, profile: &ProfileId, site: &str) -> String {
        self.styles_of(profile)
            .iter()
            .filter(|style| style.enabled && style.site == site)
            .map(|style| style.css.as_str())
            .collect()
    }
}
