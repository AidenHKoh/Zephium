//! What the Ask service needs from the shell, the engine and the model
//! layer. Production adapters live beside the desktop shell; tests use
//! in-memory fakes.
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use zephium_app::work_models::WorkCallSource;
use zephium_core::ids::ProfileId;
use zephium_core::work::model::{WorkModelClient, WorkModelEntry, WorkModelError};
use zephium_ipc::page_assist::{PageAssistChangedV1, WorkConfirmCategoryV1};
use zephium_page_assist::actions::{Direction, Verb};
pub use zephium_page_assist::outline::{Outline, OutlineKind, OutlineNode, Rgb};

/// A boxed, sendable future.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// What the shell knows about a tab without reading it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TabFacts {
    /// The profile the tab belongs to.
    pub profile: ProfileId,
    /// The committed URL; none for internal pages.
    pub url: Option<String>,
    /// The tab is in a private window.
    pub private: bool,
    /// The tab shows a browser viewer (PDF, image, source).
    pub viewer: bool,
    /// The first load has not finished.
    pub loading: bool,
    /// Changes on every committed navigation; reads and steps bind to it.
    pub epoch: u64,
}

/// A form field's kind, as the reader judged it from the live control.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FieldKind {
    /// Ordinary text.
    Text,
    /// A search box.
    Search,
    /// An e-mail address.
    Email,
    /// A number.
    Number,
    /// A password: never read, never typed into.
    Password,
    /// A payment card field: never read, never typed into.
    Card,
    /// A one-time code: never read, never typed into.
    OneTimeCode,
}

impl FieldKind {
    /// Whether its value is a secret the panel never reads or writes.
    pub fn protected(self) -> bool {
        matches!(self, Self::Password | Self::Card | Self::OneTimeCode)
    }
}

/// What a passage is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Role {
    /// A heading of the given level.
    Heading(u8),
    /// Running text.
    Text,
    /// A list item; `ordered` for numbered lists.
    ListItem {
        /// The list is numbered.
        ordered: bool,
    },
    /// A table cell.
    Cell,
    /// A link; `same_site` when it stays on the tab's site.
    Link {
        /// The destination is on the same registrable site.
        same_site: bool,
    },
    /// A button.
    Button,
    /// A text field.
    Field(FieldKind),
    /// A checkbox or switch.
    Checkbox {
        /// Its current state.
        checked: bool,
    },
    /// A drop-down or list box.
    Select,
    /// Anything else readable.
    Other,
}

/// One passage of a live read.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Passage {
    /// Its number within this read, as the model cites it (`rN`).
    pub id: u16,
    /// What it is.
    pub role: Role,
    /// The visible text or accessible name.
    pub text: String,
    /// `text` is exactly what the page shows, complete.
    pub verbatim: bool,
    /// A field's current value. Adapters never fill it for protected kinds.
    pub value: Option<String>,
    /// 0 the main content, 1 beside it, 2 the page's chrome.
    pub region: u8,
}

/// One fresh read of a tab.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Readout {
    /// The navigation the read belongs to.
    pub epoch: u64,
    /// The page's title (untrusted).
    pub title: Option<String>,
    /// Passages in reading order.
    pub passages: Vec<Passage>,
}

/// Why a read did not happen.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadFailure {
    /// The page is not ready yet.
    NotReady,
    /// The tab navigated or closed.
    PageChanged,
    /// The page could not be read.
    Unavailable,
}

/// One step, as the service asks the engine to run it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StepRequest {
    /// What it does.
    pub verb: Verb,
    /// The passage of the latest read it acts on.
    pub target: Option<u16>,
    /// A page scroll with no passage.
    pub direction: Option<Direction>,
    /// What to type or choose.
    pub value: Option<String>,
}

/// What a step would do, judged from the live control and its page.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StepEffect {
    /// Reads or moves around.
    Read,
    /// Changes the page without committing anything.
    Draft,
    /// Sends, buys, deletes or saves: needs the person's confirm.
    Commit(WorkConfirmCategoryV1),
}

/// A step the engine can run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedStep {
    /// Names the exact control, such as `Click "Place order"`.
    pub label: String,
    /// What it would do.
    pub effect: StepEffect,
}

/// Why a step did not run or did not take.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StepFailure {
    /// The tab navigated or closed.
    PageChanged,
    /// The passage is not a control this verb can use.
    Unsuitable,
    /// A protected field, another site, or a sensitive page.
    Refused,
    /// It ran without the expected effect.
    Failed,
}

/// The live tab: facts, reads, highlights, steps and page styles.
pub trait PagePort: Send + Sync {
    /// Whether this platform can read live tabs.
    fn supported(&self) -> bool;
    /// What the shell knows about `tab`.
    fn facts(&self, tab: &str) -> Option<TabFacts>;
    /// A fresh read of `tab`, bound to `epoch`.
    fn read<'a>(&'a self, tab: &'a str, epoch: u64) -> BoxFuture<'a, Result<Readout, ReadFailure>>;
    /// The page's structure and palette for a style turn, bound to `epoch`.
    fn outline<'a>(
        &'a self,
        tab: &'a str,
        epoch: u64,
    ) -> BoxFuture<'a, Result<Outline, ReadFailure>>;
    /// The selected text, when anything is selected.
    fn selection<'a>(&'a self, tab: &'a str, epoch: u64) -> BoxFuture<'a, Option<String>>;
    /// Scrolls to and marks `text` without the find bar.
    fn highlight(&self, tab: &str, epoch: u64, text: &str) -> bool;
    /// Judges a step against the latest read of `epoch`.
    fn resolve(
        &self,
        tab: &str,
        epoch: u64,
        step: &StepRequest,
    ) -> Result<ResolvedStep, StepFailure>;
    /// Runs a step under the tab's lease.
    fn act<'a>(
        &'a self,
        tab: &'a str,
        epoch: u64,
        step: &'a StepRequest,
    ) -> BoxFuture<'a, Result<(), StepFailure>>;
    /// Replaces the page-style slot of `site` in `profile`; empty removes it.
    fn apply_styles(&self, profile: &ProfileId, site: &str, css: &str);
}

/// A model and its client for one turn.
#[derive(Clone)]
pub struct ResolvedModel {
    /// The catalog entry: name, provider, price.
    pub entry: WorkModelEntry,
    /// The client; dropping a call cancels it.
    pub client: Arc<dyn WorkModelClient>,
}

/// The person's Page model.
pub trait ModelSource: Send + Sync {
    /// The model a turn in `profile` would use, without a client.
    fn entry<'a>(&'a self, profile: &'a ProfileId) -> BoxFuture<'a, Option<WorkModelEntry>>;
    /// The model and a client for one turn's calls.
    fn resolve<'a>(
        &'a self,
        profile: &'a ProfileId,
        source: WorkCallSource,
    ) -> BoxFuture<'a, Result<ResolvedModel, WorkModelError>>;
}

/// Where the panel's events go.
pub trait EventSink: Send + Sync {
    /// Delivers one event.
    fn emit(&self, event: PageAssistChangedV1);
}

/// Milliseconds on a monotonic clock.
pub trait Clock: Send + Sync {
    /// Now.
    fn now_ms(&self) -> u64;
}

/// The shell's switches.
pub trait Settings: Send + Sync {
    /// AI is switched on for `profile`.
    fn ai_enabled(&self, profile: &ProfileId) -> bool;
}

/// Starts a Work run.
pub trait WorkHandoff: Send + Sync {
    /// Begins Work in `profile` with the question as its objective and the
    /// tab, at `url`, as context; returns the new Work's id.
    fn begin<'a>(
        &'a self,
        profile: &'a ProfileId,
        tab: &'a str,
        question: String,
        url: String,
    ) -> BoxFuture<'a, Option<String>>;
}

/// One stored page style.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredStyle {
    /// Stable id.
    pub id: String,
    /// Registrable site.
    pub site: String,
    /// What it does, in a few words.
    pub summary: String,
    /// Canonical validated CSS.
    pub css: String,
    /// Applied to the site's pages.
    pub enabled: bool,
}

/// Where page styles persist per profile.
pub trait StyleStore: Send + Sync {
    /// Every stored style of `profile`.
    fn load(&self, profile: &ProfileId) -> Vec<StoredStyle>;
    /// Replaces every stored style of `profile`; false when it could not be
    /// stored, and then nothing changed.
    fn save(&self, profile: &ProfileId, styles: &[StoredStyle]) -> bool;
}
