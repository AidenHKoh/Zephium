//! Host side of the Ask panel's Browse reader: the shell's switch, each
//! view's navigation hooks, and the main-thread entry points the engine
//! handle dispatches to.

use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::sync::RwLock;

use zephium_core::ids::ItemId;

use super::EngineHost;
use crate::page_reader::{
    PageActFailure, PageOutline, PageReadFailure, PageReaderPolicy, PageReadout,
};
use crate::platform::page_recipe::{mark_outcome, parse_outline, MarkOutcome, PageRecipe};

#[cfg(target_os = "macos")]
pub(super) type Reader = crate::platform::macos::browse_reader::BrowseReader;
#[cfg(target_os = "windows")]
pub(super) type Reader = crate::platform::windows::browse_reader::BrowseReader;

/// A view's reader. The view owns the only strong handle; its native
/// callbacks hold weak ones, so the reader goes with the view.
pub(super) type ReaderSlot = Rc<RefCell<Option<Reader>>>;

static POLICY: RwLock<Option<PageReaderPolicy>> = RwLock::new(None);

/// The reader's probe sets the policy the shell would.
#[cfg(feature = "page-reader-probe")]
pub(crate) fn set_policy_for_probe(next: Option<PageReaderPolicy>) {
    *POLICY
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = next;
}

fn policy() -> Option<PageReaderPolicy> {
    POLICY
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

/// Whether the reader is switched on.
pub(super) fn enabled() -> bool {
    policy().is_some()
}

/// Whether a document at `target` may receive the reader's script.
pub(super) fn admits(target: &str) -> bool {
    policy().is_some_and(|policy| !(policy.sensitive)(target))
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(super) fn back_forward(slot: &Weak<RefCell<Option<Reader>>>, back_forward: bool) {
    let Some(slot) = slot.upgrade() else {
        return;
    };
    if let Ok(reader) = slot.try_borrow() {
        if let Some(reader) = reader.as_ref() {
            reader.back_forward(back_forward);
        }
    };
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(super) fn attempt(slot: &Weak<RefCell<Option<Reader>>>, target: &str) {
    let Some(slot) = slot.upgrade() else {
        return;
    };
    let admitted = admits(target);
    if let Ok(reader) = slot.try_borrow() {
        if let Some(reader) = reader.as_ref() {
            reader.attempt(admitted);
        }
    };
}

pub(super) fn commit(slot: &Weak<RefCell<Option<Reader>>>, epoch: u64, url: &str) {
    let Some(slot) = slot.upgrade() else {
        return;
    };
    if let Ok(reader) = slot.try_borrow() {
        if let Some(reader) = reader.as_ref() {
            reader.commit(epoch, url);
        }
    };
}

/// The person used `tab` while steps held it.
pub(crate) fn report_human_input(tab: ItemId) {
    if let Some(policy) = policy() {
        (policy.human_input)(tab);
    }
}

type ReadDone = Box<dyn FnOnce(Result<PageReadout, PageReadFailure>) + Send>;
type RecipeDone = Box<dyn FnOnce(Option<String>) + Send>;

impl EngineHost {
    fn with_reader(&self, id: ItemId, apply: impl FnOnce(&Reader)) -> bool {
        let Some(slot) = self.views.get(&id).map(|view| view.page_reader.clone()) else {
            return false;
        };
        let Ok(reader) = slot.try_borrow() else {
            return false;
        };
        match reader.as_ref() {
            Some(reader) => {
                apply(reader);
                true
            }
            None => false,
        }
    }

    /// Switches the reader on or off for every view. Off removes every
    /// script, handler and channel; on prepares the next document of each
    /// view, while documents already open stay unread.
    pub(crate) fn set_page_reader(&mut self, next: Option<PageReaderPolicy>) {
        let on = next.is_some();
        *POLICY
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = next;
        for view in self.views.values() {
            if let Ok(reader) = view.page_reader.try_borrow() {
                if let Some(reader) = reader.as_ref() {
                    reader.set_enabled(on);
                }
            }
        }
    }

    /// The committed address of `id`'s document at `epoch`.
    fn reader_target(&self, id: ItemId, epoch: u64) -> Result<String, PageReadFailure> {
        let view = self.views.get(&id).ok_or(PageReadFailure::PageChanged)?;
        let (committed, url) = view
            .navigation
            .committed_snapshot()
            .ok_or(PageReadFailure::NotReady)?;
        if committed.presentation_id().into_raw() != epoch {
            return Err(PageReadFailure::PageChanged);
        }
        if !admits(&url) {
            return Err(PageReadFailure::Unavailable);
        }
        Ok(url)
    }

    pub(crate) fn page_read(&mut self, id: ItemId, epoch: u64, done: ReadDone) {
        let url = match self.reader_target(id, epoch) {
            Ok(url) => url,
            Err(failure) => return done(Err(failure)),
        };
        let mut done = Some(done);
        let found = self.with_reader(id, |reader| {
            if let Some(done) = done.take() {
                reader.read(epoch, url, done);
            }
        });
        if let (false, Some(done)) = (found, done) {
            done(Err(PageReadFailure::Unavailable));
        }
    }

    fn page_recipe(&mut self, id: ItemId, epoch: u64, recipe: PageRecipe, done: RecipeDone) {
        if let Err(_failure) = self.reader_target(id, epoch) {
            return done(None);
        }
        let mut done = Some(done);
        let found = self.with_reader(id, |reader| {
            if let Some(done) = done.take() {
                reader.recipe(epoch, recipe, done);
            }
        });
        if let (false, Some(done)) = (found, done) {
            done(None);
        }
    }

    pub(crate) fn page_selection(
        &mut self,
        id: ItemId,
        epoch: u64,
        done: Box<dyn FnOnce(Option<String>) + Send>,
    ) {
        self.page_recipe(
            id,
            epoch,
            PageRecipe::selection(),
            Box::new(move |answer| done(answer.filter(|text| !text.is_empty()))),
        );
    }

    pub(crate) fn page_outline(
        &mut self,
        id: ItemId,
        epoch: u64,
        done: Box<dyn FnOnce(Result<PageOutline, PageReadFailure>) + Send>,
    ) {
        self.page_recipe(
            id,
            epoch,
            PageRecipe::outline(),
            Box::new(move |answer| {
                done(
                    answer
                        .as_deref()
                        .and_then(parse_outline)
                        .ok_or(PageReadFailure::Unavailable),
                );
            }),
        );
    }

    pub(crate) fn page_highlight(
        &mut self,
        id: ItemId,
        epoch: u64,
        quote: &str,
        done: Box<dyn FnOnce(bool) + Send>,
    ) {
        let Some(recipe) = PageRecipe::mark(quote) else {
            return done(false);
        };
        self.page_recipe(
            id,
            epoch,
            recipe,
            Box::new(move |answer| {
                done(matches!(
                    answer.as_deref().and_then(mark_outcome),
                    Some(MarkOutcome::Marked | MarkOutcome::Scrolled)
                ));
            }),
        );
    }

    /// Runs one person-directed step on `id`'s document at `epoch`.
    pub(crate) fn page_act(
        &mut self,
        id: ItemId,
        epoch: u64,
        read: u64,
        proposal: zephium_agentic::SemanticActionProposal,
        done: Box<dyn FnOnce(Result<(), PageActFailure>) + Send>,
    ) {
        let url = match self.reader_target(id, epoch) {
            Ok(url) => url,
            Err(PageReadFailure::PageChanged) => return done(Err(PageActFailure::PageChanged)),
            Err(_) => return done(Err(PageActFailure::Refused)),
        };
        let mut done = Some(done);
        let found = self.with_reader(id, |reader| {
            if let Some(done) = done.take() {
                reader.act(epoch, read, url, proposal, done);
            }
        });
        if let (false, Some(done)) = (found, done) {
            done(Err(PageActFailure::Failed));
        }
    }

    pub(crate) fn page_reader_poll(&mut self, id: ItemId) {
        self.with_reader(id, Reader::poll);
    }

    pub(crate) fn page_reader_pump(&mut self, id: ItemId) {
        self.with_reader(id, Reader::pump);
    }

    pub(crate) fn page_reader_timeout(&mut self, id: ItemId, operation: u64) {
        self.with_reader(id, |reader| reader.timeout(operation));
    }

    /// WebView2 keeps nothing running between asks, so only macOS idles.
    #[cfg(target_os = "macos")]
    pub(crate) fn page_reader_idle(&mut self, id: ItemId) {
        self.with_reader(id, Reader::idle);
    }
}
