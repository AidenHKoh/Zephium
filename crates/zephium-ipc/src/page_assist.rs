//! The Ask panel: questions about the page in front, answered from a fresh
//! read of that tab. Rust owns the thread, the read, the model call and every
//! effect; the panel renders whole blocks and sends the person's choices.
//! Tabs cross as their `TabView::id`. Nothing here is ever written to disk
//! except page styles the person applied.
use serde::{Deserialize, Serialize};
use specta::Type;
pub use zephium_core::work::runtime::{WorkConfirmCategoryV1, WorkModelFaultV1};

/// Longest question the panel may send, in UTF-8 bytes.
pub const PAGE_ASSIST_MAX_QUESTION_BYTES: usize = 2 * 1024;
/// Longest excerpt a citation, selection or style summary carries, in characters.
pub const PAGE_ASSIST_MAX_EXCERPT_CHARS: usize = 80;

/// What a turn is for. Rust picks the prompt; the panel never sends one.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq, Type)]
#[serde(rename_all = "snake_case")]
pub enum PageAssistIntentV1 {
    /// A question about the page.
    Ask,
    Summarize,
    /// A question about what the person selected on the page.
    Selection,
    /// A few everyday steps on this page.
    Act,
    /// Restyle this site (CSS only, applied on the person's click).
    Style,
}

/// One-click starts Rust offers for the page in front. The panel names them.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq, Type)]
#[serde(rename_all = "snake_case")]
pub enum PageAssistSuggestionV1 {
    Summarize,
    KeyPoints,
    KeyNumbers,
    /// "What's the catch?": fine print, conditions, costs.
    Catch,
    /// Plain-language explanation of a dense page.
    Explain,
    /// The steps a how-to or recipe asks for.
    Steps,
    /// Compare the options a product or pricing page lists.
    Compare,
    /// A darker version of this site.
    Darker,
    /// Larger, easier-to-read text on this site.
    Readable,
}

/// Whether the tab can be asked about now. Anything but `ready` means no
/// request is sent.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq, Type)]
#[serde(rename_all = "snake_case")]
pub enum PageAssistAvailabilityV1 {
    Ready,
    /// AI is switched off in Settings.
    AiOff,
    /// Private windows never send pages to a model.
    Private,
    /// Not a web page: internal pages, new tab, files, extension pages, PDFs.
    NotWeb,
    /// Banks, payments, health and government sites are never read.
    Sensitive,
    /// No model is set up: no Zephium account and no key.
    NoModel,
    /// The page is still loading; ask again when it settles.
    Loading,
    /// This platform cannot read pages yet.
    Unsupported,
    /// The tab is gone or the service is not running.
    Unavailable,
}

/// The model a turn uses, as the panel names it.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Type)]
#[serde(deny_unknown_fields)]
pub struct PageAssistModelV1 {
    /// Display name, such as Luna or GPT-5 mini.
    pub name: String,
    /// Zephium AI (credits) rather than the person's own key.
    pub hosted: bool,
}

/// Why a turn could not start or finish. Closed; never provider or page text.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq, Type)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum PageAssistFaultV1 {
    AiOff,
    Private,
    NotWeb,
    Sensitive,
    NoModel,
    Loading,
    Unsupported,
    /// Another turn on this tab is still running.
    Busy,
    /// The question is longer than [`PAGE_ASSIST_MAX_QUESTION_BYTES`] or empty.
    Invalid,
    /// Nothing is selected on the page.
    NoSelection,
    /// The page could not be read.
    Unreadable,
    /// The page navigated or closed while the turn ran.
    PageChanged,
    /// The model answered with nothing usable.
    NoAnswer,
    /// A page step did not take effect as expected.
    StepFailed,
    /// The model call failed. `retry_after_ms` is set for `rate_limited`
    /// (and hosted limits) when the provider or server said when to retry.
    Model {
        fault: WorkModelFaultV1,
        retry_after_ms: Option<u32>,
    },
    Unavailable,
}

/// What the panel shows while a turn works.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq, Type)]
#[serde(tag = "phase", rename_all = "snake_case")]
pub enum PageAssistPhaseV1 {
    /// "Reading the page…"
    Reading,
    /// "Read 214 passages"
    Read { passages: u32 },
    /// "Answering…"
    Answering,
    /// "Working on the page…"
    Acting,
    /// "Designing a style…"
    Styling,
}

/// One numbered source on the page. `excerpt` is the page's own words
/// (≤ 80 characters), never the model's.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Type)]
#[serde(deny_unknown_fields)]
pub struct PageAssistCiteV1 {
    /// Number shown on the chip, 1-based within the turn; pass it to
    /// `page_assist_cite`.
    pub n: u8,
    pub excerpt: String,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq, Type)]
#[serde(rename_all = "snake_case")]
pub enum PageAssistPartKindV1 {
    /// The short answer, first and once per turn.
    Answer,
    /// A supporting point.
    Point,
    /// A labelled value, such as "Price :: $12/month".
    Figure,
    /// Something not on the page, or a limit of the answer. Never cited.
    Caveat,
}

/// One whole block of an answer, delivered complete (no token streaming).
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Type)]
#[serde(deny_unknown_fields)]
pub struct PageAssistPartV1 {
    pub kind: PageAssistPartKindV1,
    pub text: String,
    /// A figure's label; null for every other kind.
    pub label: Option<String>,
    pub cites: Vec<PageAssistCiteV1>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq, Type)]
#[serde(rename_all = "snake_case")]
pub enum PageAssistStepStateV1 {
    Running,
    /// Waits on the person: Approve or Decline through `page_assist_decide`.
    Confirm,
    Done,
    Declined,
    Failed,
    /// Not run because an earlier step stopped the turn.
    Skipped,
}

/// One step of a light page action.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Type)]
#[serde(deny_unknown_fields)]
pub struct PageAssistStepV1 {
    /// 0-based; at most 8 steps per turn.
    pub index: u8,
    /// What the step does, naming the exact control, such as
    /// `Click "Place order"`.
    pub label: String,
    /// Set when the step commits something and so needs the person's confirm.
    pub category: Option<WorkConfirmCategoryV1>,
    pub state: PageAssistStepStateV1,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq, Type)]
#[serde(rename_all = "snake_case")]
pub enum PageStyleStateV1 {
    /// Waits for the person's Apply (`page_assist_decide` Approve).
    Proposed,
    Applied,
    Declined,
    /// Nothing the model wrote passed the style rules; nothing applies.
    Rejected,
    /// The site already holds as many styles as it can (8, or their combined
    /// size); the person removes one in Settings or the panel to add another.
    Full,
}

/// A page style the model proposed in a Style turn.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Type)]
#[serde(deny_unknown_fields)]
pub struct PageStyleProposalV1 {
    /// The stored style's id once applied; null before.
    pub id: Option<String>,
    /// Short description, such as "Dark background, softer text".
    pub summary: String,
    /// Registrable site it applies to, such as wikipedia.org.
    pub site: String,
    /// Rules that passed validation.
    pub rules: u16,
    pub state: PageStyleStateV1,
}

/// Why Rust offers "Run in Work" instead of trying in the panel.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq, Type)]
#[serde(rename_all = "snake_case")]
pub enum PageAssistOfferV1 {
    /// It needs other pages or sites.
    MultiPage,
    /// It needs more than a few steps.
    Complex,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq, Type)]
#[serde(rename_all = "snake_case")]
pub enum PageAssistPauseV1 {
    /// The person used the page; Continue re-reads it (`page_assist_resume`).
    TookOver,
}

/// What a turn cost, for the line under the answer.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PageAssistCostV1 {
    /// Zephium AI. `credits` is a decimal string such as "0.3"; null while
    /// the charge is being confirmed.
    Hosted {
        model: String,
        credits: Option<String>,
    },
    /// The person's own key. `usd` is an estimate as a decimal string such
    /// as "0.004"; null when no price is known.
    Byok {
        model: String,
        provider: String,
        usd: Option<String>,
    },
    /// A model on this device or network; no charge.
    Local { model: String },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq, Type)]
#[serde(rename_all = "snake_case")]
pub enum PageAssistTurnStateV1 {
    Working,
    /// A confirm or a style Apply waits on the person.
    Waiting,
    Paused,
    Done,
    Failed,
    Cancelled,
}

/// One question and everything Rust produced for it.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Type)]
#[serde(deny_unknown_fields)]
pub struct PageAssistTurnV1 {
    pub turn: u32,
    pub intent: PageAssistIntentV1,
    pub suggestion: Option<PageAssistSuggestionV1>,
    /// What the person typed; empty for a suggestion.
    pub question: String,
    /// The start of the selection asked about.
    pub selection: Option<String>,
    pub state: PageAssistTurnStateV1,
    /// Latest progress line while working.
    pub phase: Option<PageAssistPhaseV1>,
    pub parts: Vec<PageAssistPartV1>,
    pub steps: Vec<PageAssistStepV1>,
    pub style: Option<PageStyleProposalV1>,
    pub offer: Option<PageAssistOfferV1>,
    pub cost: Option<PageAssistCostV1>,
    pub fault: Option<PageAssistFaultV1>,
    /// The page changed since; its citations no longer jump.
    pub inert: bool,
    /// The last event sequence folded into this snapshot.
    pub seq: u32,
}

/// The thread in order: turns, and a divider wherever the tab navigated.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PageAssistEntryV1 {
    Turn {
        turn: Box<PageAssistTurnV1>,
    },
    /// The tab moved to another page. Across sites, the model starts fresh.
    Divider {
        site: String,
        cross_site: bool,
    },
}

/// Everything the panel needs for one tab. Read on open, on tab switch and
/// after `invalidated`.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Type)]
#[serde(deny_unknown_fields)]
pub struct PageAssistStatusV1 {
    pub version: u16,
    pub tab: String,
    pub availability: PageAssistAvailabilityV1,
    /// Registrable site of the page, such as nytimes.com.
    pub site: Option<String>,
    pub model: Option<PageAssistModelV1>,
    pub suggestions: Vec<PageAssistSuggestionV1>,
    /// The start of what is selected on the page, when anything is.
    pub selection: Option<String>,
    /// Light page actions are available here.
    pub can_act: bool,
    /// At least one of this site's page styles is on.
    pub styled: bool,
    pub entries: Vec<PageAssistEntryV1>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Type)]
#[serde(deny_unknown_fields)]
pub struct PageAssistAskV1 {
    pub tab: String,
    /// The person's words; may be empty when `suggestion` is set.
    pub text: String,
    pub intent: PageAssistIntentV1,
    pub suggestion: Option<PageAssistSuggestionV1>,
}

/// Admission only: the answer arrives as events for `turn`.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Type)]
#[serde(deny_unknown_fields)]
pub struct PageAssistAskResponseV1 {
    pub turn: Option<u32>,
    pub fault: Option<PageAssistFaultV1>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq, Type)]
#[serde(rename_all = "snake_case")]
pub enum PageAssistDecisionV1 {
    Approve,
    Decline,
}

/// "Run in Work" result: the new Work's id, or why it did not start.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Type)]
#[serde(deny_unknown_fields)]
pub struct PageAssistHandoffV1 {
    pub work: Option<String>,
    pub fault: Option<PageAssistFaultV1>,
}

/// One stored style for a site.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Type)]
#[serde(deny_unknown_fields)]
pub struct PageStyleV1 {
    pub id: String,
    pub summary: String,
    pub enabled: bool,
}

/// The styles of the site in a tab.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Type)]
#[serde(deny_unknown_fields)]
pub struct PageStylesV1 {
    pub version: u16,
    pub tab: String,
    pub site: Option<String>,
    pub styles: Vec<PageStyleV1>,
}

/// One site's styles, for the Settings manager.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Type)]
#[serde(deny_unknown_fields)]
pub struct PageSiteStylesV1 {
    pub site: String,
    pub styles: Vec<PageStyleV1>,
}

/// Every site's styles in a profile, sites in name order.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Type)]
#[serde(deny_unknown_fields)]
pub struct PageStylesAllV1 {
    pub version: u16,
    pub profile: String,
    pub sites: Vec<PageSiteStylesV1>,
}

/// One change to a tab's thread, in `seq` order per tab. Apply it to the
/// matching turn; on `invalidated`, read `page_assist_status` again.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Type)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PageAssistEventV1 {
    /// The first event of every turn: its whole starting snapshot, with the
    /// question and selection excerpt.
    Started {
        turn: Box<PageAssistTurnV1>,
    },
    /// The tab committed a navigation; append this divider. Sent for every
    /// committed navigation of a tab that has a thread; `turn` is the latest
    /// turn, whose citations (and all earlier ones) are now inert.
    Divider {
        site: String,
        cross_site: bool,
    },
    Status {
        phase: PageAssistPhaseV1,
    },
    Part {
        part: PageAssistPartV1,
    },
    /// A step was added or changed state; replaces the step with its index.
    Step {
        step: PageAssistStepV1,
    },
    Style {
        style: PageStyleProposalV1,
    },
    /// Better done in Work; the panel offers "Run in Work".
    Offer {
        offer: PageAssistOfferV1,
    },
    Paused {
        reason: PageAssistPauseV1,
    },
    /// The turn finished. `cost` may still be confirming; a later `cost`
    /// event replaces it.
    Done {
        cost: Option<PageAssistCostV1>,
    },
    Cost {
        cost: PageAssistCostV1,
    },
    Failed {
        fault: PageAssistFaultV1,
    },
    Cancelled,
    /// The thread changed shape (navigation, clear, tab closed, AI off).
    Invalidated,
}

/// Payload of `zephium:page-assist-changed`.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Type)]
#[serde(deny_unknown_fields)]
pub struct PageAssistChangedV1 {
    pub tab: String,
    pub turn: u32,
    pub seq: u32,
    pub kind: PageAssistEventV1,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_serialize_with_a_type_tag_inside_kind() {
        let event = PageAssistChangedV1 {
            tab: "t".into(),
            turn: 2,
            seq: 5,
            kind: PageAssistEventV1::Part {
                part: PageAssistPartV1 {
                    kind: PageAssistPartKindV1::Point,
                    text: "Ships in May".into(),
                    label: None,
                    cites: vec![PageAssistCiteV1 {
                        n: 1,
                        excerpt: "Shipping begins in May".into(),
                    }],
                },
            },
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["kind"]["type"], "part");
        assert_eq!(json["kind"]["part"]["kind"], "point");
        let fault = serde_json::to_value(PageAssistFaultV1::Model {
            fault: WorkModelFaultV1::DailyLimit,
            retry_after_ms: None,
        })
        .unwrap();
        assert_eq!(fault["reason"], "model");
        assert_eq!(fault["fault"], "daily_limit");
    }
}
