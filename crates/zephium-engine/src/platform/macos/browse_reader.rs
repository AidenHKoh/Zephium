#![deny(unsafe_op_in_unsafe_fn)]
#![deny(clippy::undocumented_unsafe_blocks)]
#![deny(clippy::dbg_macro, clippy::print_stderr, clippy::print_stdout)]
#![cfg_attr(
    not(test),
    deny(clippy::panic, clippy::unreachable, clippy::unwrap_used)
)]

//! The Ask panel's reader for a person's own Browse tab.
//!
//! Each view has one private content world holding a ~1 KB document-start
//! script. In every document it holds a single pull on its own handler and
//! does nothing else. Only when the person asks does native answer that pull
//! with the fixed program (the pinned semantic runtime plus the fixed
//! recipes), which the script instantiates inside the private world. Reads
//! and recipes then run as closed invocations over the pinned channel state
//! machine; WebKit's evaluation APIs are never used, so the page gains no
//! user activation.
//!
//! Every instantiation gets a fresh channel and handler registration, bound
//! to the committed navigation it was opened for. A navigation's commit ends
//! the previous document's channel and removes its handler, so a late answer
//! from an older document can never reach the newer one.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::hash::{BuildHasher as _, Hasher as _};
use std::panic::AssertUnwindSafe;
use std::ptr::null_mut;
use std::rc::{Rc, Weak};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{define_class, msg_send, sel, DefinedClass as _, MainThreadOnly};
use objc2_foundation::{MainThreadMarker, NSObjectProtocol, NSString, NSUTF8StringEncoding};
use objc2_web_kit::{
    WKContentWorld, WKScriptMessage, WKScriptMessageHandlerWithReply, WKUserContentController,
    WKUserScript, WKUserScriptInjectionTime, WKWebView,
};
use zephium_agentic::{ContextJoin, SemanticRuntimeFault, SemanticRuntimeResultError};
use zephium_core::ids::ItemId;

use super::semantic_runtime::{
    browse_channel, AgentSemanticRuntimeController, AgentSemanticRuntimeDispatchError,
    AgentSemanticRuntimeFailure,
};
use crate::page_reader::{PageReadFailure, PageReadout};
use crate::platform::page_recipe::PageRecipe;

const BOOT_HANDLER: &str = "zephiumPageAssistBootV1";
const CHANNEL_HANDLER: &str = "zephiumPageAssistV1";
const WORLD_PREFIX: &str = "zephium-page-assist-v1-";
/// A document's script has not received the program yet.
const BOOT_FRESH: &str = "B1";
/// A document's script holds the program and waits to serve again.
const BOOT_INSTANTIATED: &str = "B2";
/// The program could not be instantiated in this document.
const BOOT_FAILED: &str = "E1";
const RESTART: &str = "G1";
const STOP: &str = "S1";
const FIXED_ERROR: &str = "zephium page reader refused";
const MAX_QUEUED: usize = 8;
const READ_TIMEOUT: Duration = Duration::from_secs(12);
const RECIPE_TIMEOUT: Duration = Duration::from_secs(5);
/// A channel unused this long is stopped; the document's script goes back to
/// its one dormant pull.
const IDLE: Duration = Duration::from_secs(180);

pub(crate) const BOOTSTRAP: &str = include_str!("../page_reader_bootstrap.js");
const RECIPES: &str = include_str!("../page_reader_recipes.js");
const SERVE: &str = include_str!("page_reader_serve.js");

/// The fixed program a document receives when the person first asks:
/// the pinned runtime installation, the recipes, and the pull loop.
pub(crate) fn program() -> &'static str {
    static PROGRAM: OnceLock<String> = OnceLock::new();
    PROGRAM.get_or_init(|| {
        let installation = zephium_agentic::SEMANTIC_RUNTIME_PROGRAM.installation_source();
        let mut program =
            String::with_capacity(installation.len() + RECIPES.len() + SERVE.len() + 2);
        program.push_str(installation);
        program.push('\n');
        program.push_str(RECIPES);
        program.push('\n');
        program.push_str(SERVE);
        program
    })
}

type ReplyBlock = RcBlock<dyn Fn(*mut AnyObject, *mut NSString)>;
pub(crate) type ReadDone = Box<dyn FnOnce(Result<PageReadout, PageReadFailure>) + Send>;
pub(crate) type RecipeDone = Box<dyn FnOnce(Option<String>) + Send>;

enum Op {
    Read {
        epoch: u64,
        url: String,
        done: ReadDone,
    },
    Recipe {
        epoch: u64,
        recipe: PageRecipe,
        done: RecipeDone,
    },
}

impl Op {
    fn fail(self, failure: PageReadFailure) {
        match self {
            Self::Read { done, .. } => done(Err(failure)),
            Self::Recipe { done, .. } => done(None),
        }
    }

    const fn epoch(&self) -> u64 {
        match self {
            Self::Read { epoch, .. } | Self::Recipe { epoch, .. } => *epoch,
        }
    }
}

/// A document's dormant pull, waiting for the person to ask.
struct Pull {
    reply: ReplyBlock,
    /// The committed navigation it belongs to, once known.
    epoch: Option<u64>,
    /// The navigation attempts seen when it arrived.
    attempts: u64,
    /// The document already holds the program.
    instantiated: bool,
}

struct Channel {
    controller: AgentSemanticRuntimeController,
    epoch: u64,
    broken: Rc<Cell<bool>>,
    last_used: Instant,
}

/// The operation in flight: what its timer may end.
struct Current {
    id: u64,
    read: Option<zephium_agentic::SemanticInvocationId>,
    _timer: Option<super::ContentPolicyTimeout>,
}

struct ReaderState {
    tab: Rc<Cell<ItemId>>,
    controller: Retained<WKUserContentController>,
    view: objc2::rc::Weak<WKWebView>,
    view_address: usize,
    world: Retained<WKContentWorld>,
    context: ContextJoin,
    enabled: bool,
    /// The boot handler is registered.
    listening: bool,
    script: Option<Retained<WKUserScript>>,
    attempts: u64,
    committed: Option<u64>,
    failed: Option<u64>,
    pull: Option<Pull>,
    channel: Option<Channel>,
    sequence: u64,
    queue: VecDeque<Op>,
    busy: Rc<Cell<bool>>,
    current: Option<Current>,
    idle_timer: Option<super::ContentPolicyTimeout>,
    committed_now: Rc<dyn Fn() -> Option<u64>>,
    weak: Weak<RefCell<ReaderState>>,
}

/// Re-runs the reader's queue from a fresh main-thread turn.
fn schedule_pump(tab: ItemId) {
    let _ = crate::host::try_with(move |host| host.page_reader_pump(tab));
}

fn answer(reply: ReplyBlock, value: &str) -> bool {
    objc2::exception::catch(AssertUnwindSafe(|| {
        let value = NSString::from_str(value);
        let pointer = Retained::as_ptr(&value).cast_mut().cast::<AnyObject>();
        reply.call((pointer, null_mut()));
    }))
    .is_ok()
}

fn refuse(reply: ReplyBlock) {
    let _ = objc2::exception::catch(AssertUnwindSafe(|| {
        let error = NSString::from_str(FIXED_ERROR);
        reply.call((null_mut(), Retained::as_ptr(&error).cast_mut()));
    }));
}

fn world_name() -> String {
    let state = std::collections::hash_map::RandomState::new();
    let mut high = state.build_hasher();
    high.write_u64(0x5a45_5048);
    let mut low = state.build_hasher();
    low.write_u64(0x4153_4b31);
    format!("{WORLD_PREFIX}{:016x}{:016x}", high.finish(), low.finish())
}

/// Whether this WebKit can remove one world's scripts without touching the
/// view's others. Without it the reader never installs.
fn supported(controller: &WKUserContentController) -> bool {
    // SAFETY: `respondsToSelector:` is NSObject API on a live controller.
    unsafe {
        msg_send![
            controller,
            respondsToSelector: sel!(_removeAllUserScriptsAssociatedWithContentWorld:)
        ]
    }
}

fn remove_handler(controller: &WKUserContentController, world: &WKContentWorld, name: &str) {
    // SAFETY: main-thread controller and world are live retained objects;
    // exceptions are contained.
    let _ = objc2::exception::catch(AssertUnwindSafe(|| unsafe {
        controller.removeScriptMessageHandlerForName_contentWorld(&NSString::from_str(name), world);
    }));
}

fn map_failure(failure: &AgentSemanticRuntimeFailure) -> PageReadFailure {
    match failure {
        AgentSemanticRuntimeFailure::Dispatch(
            AgentSemanticRuntimeDispatchError::NotReady | AgentSemanticRuntimeDispatchError::Busy,
        ) => PageReadFailure::NotReady,
        AgentSemanticRuntimeFailure::Cancelled
        | AgentSemanticRuntimeFailure::DocumentReplaced
        | AgentSemanticRuntimeFailure::Retired => PageReadFailure::PageChanged,
        AgentSemanticRuntimeFailure::Result(SemanticRuntimeResultError::Runtime(
            SemanticRuntimeFault::DocumentLoading,
        )) => PageReadFailure::NotReady,
        AgentSemanticRuntimeFailure::Dispatch(_)
        | AgentSemanticRuntimeFailure::RendererLost
        | AgentSemanticRuntimeFailure::TimedOut
        | AgentSemanticRuntimeFailure::Transport
        | AgentSemanticRuntimeFailure::Result(_) => PageReadFailure::Unavailable,
    }
}

impl ReaderState {
    fn tab(&self) -> ItemId {
        self.tab.get()
    }

    fn publish(&self) {
        let Some(epoch) = self.committed else {
            return;
        };
        let ready = self.enabled
            && self.failed != Some(epoch)
            && (self
                .pull
                .as_ref()
                .is_some_and(|pull| pull.epoch == Some(epoch))
                || self
                    .channel
                    .as_ref()
                    .is_some_and(|channel| channel.epoch == epoch));
        crate::page_reader::publish_ready(self.tab(), epoch, ready);
    }

    fn listen(&mut self) {
        if self.listening {
            return;
        }
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let handler_name = NSString::from_str(BOOT_HANDLER);
        let boot = BootHandler::alloc(mtm).set_ivars(BootIvars {
            controller: self.controller.clone(),
            world: self.world.clone(),
            name: handler_name.clone(),
            view: self.view_address,
            reader: self.weak.clone(),
        });
        // SAFETY: a freshly allocated instance of the declared class with
        // initialized ivars; `init` is NSObject's designated initializer.
        let boot: Retained<BootHandler> = unsafe { msg_send![super(boot), init] };
        // SAFETY: main thread proven above; every argument is retained for
        // the call and WebKit retains the handler. Exceptions are contained.
        self.listening = objc2::exception::catch(AssertUnwindSafe(|| unsafe {
            self.controller
                .addScriptMessageHandlerWithReply_contentWorld_name(
                    objc2::runtime::ProtocolObject::from_ref(&*boot),
                    &self.world,
                    &handler_name,
                );
        }))
        .is_ok();
    }

    /// The bootstrap present (for the next document) or absent.
    fn set_script(&mut self, present: bool) {
        if present == self.script.is_some() {
            return;
        }
        if !present {
            self.script = None;
            // SAFETY: `supported` proved the selector before this reader
            // existed; it takes one retained WKContentWorld, ours alone.
            let _ = objc2::exception::catch(AssertUnwindSafe(|| unsafe {
                let _: () = msg_send![
                    &*self.controller,
                    _removeAllUserScriptsAssociatedWithContentWorld: &*self.world
                ];
            }));
            return;
        }
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let source = NSString::from_str(BOOTSTRAP);
        // SAFETY: main thread proven above; source and world are retained
        // and WebKit retains the script. Exceptions become no script.
        self.script = objc2::exception::catch(AssertUnwindSafe(|| unsafe {
            let script = WKUserScript::initWithSource_injectionTime_forMainFrameOnly_inContentWorld(
                WKUserScript::alloc(mtm),
                &source,
                WKUserScriptInjectionTime::AtDocumentStart,
                true,
                &self.world,
            );
            self.controller.addUserScript(&script);
            script
        }))
        .ok();
    }

    fn stop_pull(&mut self) {
        if let Some(pull) = self.pull.take() {
            let _ = answer(pull.reply, STOP);
        }
    }

    /// Ends the document's channel; its handler goes with it, so nothing
    /// more from that document can reach the next one's.
    fn stop_channel(&mut self) {
        if let Some(channel) = self.channel.take() {
            channel.controller.cancel();
            remove_handler(&self.controller, &self.world, CHANNEL_HANDLER);
        }
    }

    fn attempt(&mut self, admitted: bool) {
        self.attempts = self.attempts.wrapping_add(1);
        self.set_script(self.enabled && admitted);
    }

    fn commit(&mut self, epoch: u64) {
        self.committed = Some(epoch);
        crate::page_reader::publish_commit(self.tab(), epoch);
        if self
            .channel
            .as_ref()
            .is_some_and(|channel| channel.epoch != epoch)
        {
            self.stop_channel();
        }
        let attempts = self.attempts;
        match self.pull.as_mut() {
            // A pull that arrived after this navigation's attempt and before
            // its commit was reported is this document's.
            Some(pull) if pull.epoch != Some(epoch) && pull.attempts == attempts => {
                pull.epoch = Some(epoch);
            }
            Some(pull) if pull.epoch != Some(epoch) => self.stop_pull(),
            _ => {}
        }
        self.publish();
    }

    fn boot_message(&mut self, body: &str, reply: ReplyBlock) {
        if !self.enabled {
            let _ = answer(reply, STOP);
            return;
        }
        if body == BOOT_FAILED {
            self.failed = self.committed;
            let _ = answer(reply, STOP);
            self.publish();
            return;
        }
        self.stop_pull();
        self.pull = Some(Pull {
            reply,
            epoch: self.committed,
            attempts: self.attempts,
            instantiated: body == BOOT_INSTANTIATED,
        });
        self.publish();
    }

    /// The document's live channel, or a fresh one opened by answering its
    /// dormant pull with the program (first time) or a restart.
    fn channel_for(
        &mut self,
        epoch: u64,
    ) -> Result<AgentSemanticRuntimeController, PageReadFailure> {
        if let Some(channel) = self
            .channel
            .as_mut()
            .filter(|channel| channel.epoch == epoch && !channel.broken.get())
        {
            channel.last_used = Instant::now();
            return Ok(channel.controller.clone());
        }
        self.stop_channel();
        if self.failed == Some(epoch) {
            return Err(PageReadFailure::Unavailable);
        }
        let view = self.view.load().ok_or(PageReadFailure::PageChanged)?;
        let Some(pull) = self.pull.take_if(|pull| pull.epoch == Some(epoch)) else {
            return Err(PageReadFailure::NotReady);
        };
        let broken = Rc::new(Cell::new(false));
        let (on_invariant, on_panic) = (broken.clone(), broken.clone());
        let Ok(controller) = browse_channel(
            &self.controller,
            &self.world,
            &view,
            &NSString::from_str(CHANNEL_HANDLER),
            Rc::new(move || on_invariant.set(true)),
            Rc::new(move || on_panic.set(true)),
        ) else {
            self.pull = Some(pull);
            return Err(PageReadFailure::Unavailable);
        };
        let message = if pull.instantiated {
            RESTART
        } else {
            program()
        };
        if !answer(pull.reply, message) {
            controller.cancel();
            remove_handler(&self.controller, &self.world, CHANNEL_HANDLER);
            return Err(PageReadFailure::Unavailable);
        }
        self.channel = Some(Channel {
            controller: controller.clone(),
            epoch,
            broken,
            last_used: Instant::now(),
        });
        Ok(controller)
    }

    fn pump(&mut self) {
        while !self.busy.get() {
            let Some(op) = self.queue.pop_front() else {
                return;
            };
            if !self.enabled {
                op.fail(PageReadFailure::Unavailable);
                continue;
            }
            let epoch = op.epoch();
            if (self.committed_now)() != Some(epoch) || self.committed != Some(epoch) {
                op.fail(PageReadFailure::PageChanged);
                continue;
            }
            match self.channel_for(epoch) {
                Ok(controller) => self.dispatch(controller, op),
                Err(failure) => op.fail(failure),
            }
        }
    }

    fn dispatch(&mut self, controller: AgentSemanticRuntimeController, op: Op) {
        let tab = self.tab();
        self.sequence = self.sequence.saturating_add(1);
        let id = self.sequence;
        let busy = self.busy.clone();
        busy.set(true);
        let finish = move || {
            busy.set(false);
            schedule_pump(tab);
        };
        let timeout = |limit| {
            super::schedule_content_policy_timeout(limit, move || {
                let _ = crate::host::try_with(move |host| host.page_reader_timeout(tab, id));
            })
        };
        match op {
            Op::Read { epoch, url, done } => {
                let Some((invocation, pending)) =
                    crate::page_reader::read::invocation(self.context, &url, id)
                else {
                    done(Err(PageReadFailure::Unavailable));
                    finish();
                    return;
                };
                let read = invocation.invocation();
                let committed_now = self.committed_now.clone();
                let view = self.view.clone();
                let started = Instant::now();
                let dispatched = controller.dispatch(invocation, move |result| {
                    let outcome = match result {
                        Ok(_) if committed_now() != Some(epoch) => {
                            Err(PageReadFailure::PageChanged)
                        }
                        Ok(snapshot) => {
                            let captured = u64::try_from(started.elapsed().as_millis())
                                .unwrap_or(u64::MAX)
                                .max(1);
                            crate::page_reader::read::passages(pending, snapshot, captured).map(
                                |passages| PageReadout {
                                    epoch,
                                    title: view.load().and_then(|view| title(&view)),
                                    passages,
                                },
                            )
                        }
                        Err(failure) => Err(map_failure(&failure)),
                    };
                    done(outcome);
                    finish();
                });
                if dispatched.is_ok() {
                    self.current = Some(Current {
                        id,
                        read: Some(read),
                        _timer: timeout(READ_TIMEOUT),
                    });
                }
            }
            Op::Recipe {
                epoch,
                recipe,
                done,
            } => {
                let committed_now = self.committed_now.clone();
                self.current = Some(Current {
                    id,
                    read: None,
                    _timer: timeout(RECIPE_TIMEOUT),
                });
                controller.dispatch_recipe(recipe, move |answer| {
                    done(answer.filter(|_| committed_now() == Some(epoch)));
                    finish();
                });
            }
        }
        self.idle_timer = super::schedule_content_policy_timeout(IDLE, move || {
            let _ = crate::host::try_with(move |host| host.page_reader_idle(tab));
        });
    }

    fn timeout(&mut self, id: u64) {
        let Some(current) = self.current.take_if(|current| current.id == id) else {
            return;
        };
        let Some(channel) = self.channel.as_ref().filter(|_| self.busy.get()) else {
            return;
        };
        // A read the page never took ends timed out and the document stays
        // usable; anything it took may still answer, so the channel ends.
        let ended = match current.read {
            Some(read) => channel.controller.timeout(read),
            None => {
                channel.controller.cancel();
                true
            }
        };
        if ended {
            channel.broken.set(true);
        }
    }

    fn idle(&mut self) {
        self.idle_timer = None;
        if self.busy.get() {
            return;
        }
        if self
            .channel
            .as_ref()
            .is_some_and(|channel| channel.last_used.elapsed() >= IDLE)
        {
            self.stop_channel();
            self.publish();
        }
    }

    fn enable(&mut self) {
        self.enabled = true;
        self.listen();
        self.set_script(true);
    }

    fn disable(&mut self) {
        self.enabled = false;
        self.stop_pull();
        self.stop_channel();
        self.set_script(false);
        if self.listening {
            remove_handler(&self.controller, &self.world, BOOT_HANDLER);
            self.listening = false;
        }
        while let Some(op) = self.queue.pop_front() {
            op.fail(PageReadFailure::Unavailable);
        }
        self.idle_timer = None;
        self.publish();
    }
}

fn title(view: &WKWebView) -> Option<String> {
    // SAFETY: `title` is a main-thread WKWebView property read; objc2
    // retains the returned string.
    let title = unsafe { view.title() }?;
    if title.length() > 4 * 1024 {
        return None;
    }
    Some(zephium_core::item::sanitize_page_title(&title.to_string())).filter(|t| !t.is_empty())
}

/// One Browse view's reader. Lives with the view and is dropped before it.
pub(crate) struct BrowseReader {
    state: Rc<RefCell<ReaderState>>,
}

impl BrowseReader {
    /// A reader for `view`, or none when this WebKit cannot remove one
    /// world's scripts alone.
    pub(crate) fn new(
        view: &WKWebView,
        tab: Rc<Cell<ItemId>>,
        enabled: bool,
        committed_now: Rc<dyn Fn() -> Option<u64>>,
    ) -> Option<Self> {
        let mtm = MainThreadMarker::new()?;
        // SAFETY: main thread proven above; objc2 retains the configuration
        // and its controller.
        let controller = unsafe { view.configuration().userContentController() };
        if !supported(&controller) {
            return None;
        }
        let name = NSString::from_str(&world_name());
        // SAFETY: `mtm` proves main-thread creation; `name` is a live
        // retained string and exceptions are contained.
        let world = objc2::exception::catch(AssertUnwindSafe(|| unsafe {
            WKContentWorld::worldWithName(&name, mtm)
        }))
        .ok()?;
        let context = crate::page_reader::read::reader_context()?;
        let state = Rc::new_cyclic(|weak| {
            RefCell::new(ReaderState {
                tab,
                controller,
                view: objc2::rc::Weak::new(view),
                view_address: std::ptr::from_ref(view).addr(),
                world,
                context,
                enabled: false,
                listening: false,
                script: None,
                attempts: 0,
                committed: None,
                failed: None,
                pull: None,
                channel: None,
                sequence: 0,
                queue: VecDeque::new(),
                busy: Rc::new(Cell::new(false)),
                current: None,
                idle_timer: None,
                committed_now,
                weak: weak.clone(),
            })
        });
        if enabled {
            state.borrow_mut().enable();
        }
        Some(Self { state })
    }

    /// An admitted main-frame load is about to start: the next document
    /// gets the script, or none when `admitted` is false.
    pub(crate) fn attempt(&self, admitted: bool) {
        if let Ok(mut state) = self.state.try_borrow_mut() {
            state.attempt(admitted);
        }
    }

    pub(crate) fn commit(&self, epoch: u64) {
        if let Ok(mut state) = self.state.try_borrow_mut() {
            state.commit(epoch);
            state.pump();
        }
    }

    pub(crate) fn set_enabled(&self, enabled: bool) {
        let Ok(mut state) = self.state.try_borrow_mut() else {
            return;
        };
        match (enabled, state.enabled) {
            (true, false) => state.enable(),
            (false, true) => state.disable(),
            _ => {}
        }
    }

    fn enqueue(&self, op: Op) {
        let Ok(mut state) = self.state.try_borrow_mut() else {
            op.fail(PageReadFailure::NotReady);
            return;
        };
        if state.queue.len() >= MAX_QUEUED {
            drop(state);
            op.fail(PageReadFailure::Unavailable);
            return;
        }
        state.queue.push_back(op);
        state.pump();
    }

    pub(crate) fn read(&self, epoch: u64, url: String, done: ReadDone) {
        self.enqueue(Op::Read { epoch, url, done });
    }

    pub(crate) fn recipe(&self, epoch: u64, recipe: PageRecipe, done: RecipeDone) {
        self.enqueue(Op::Recipe {
            epoch,
            recipe,
            done,
        });
    }

    pub(crate) fn pump(&self) {
        if let Ok(mut state) = self.state.try_borrow_mut() {
            state.pump();
        }
    }

    pub(crate) fn timeout(&self, id: u64) {
        if let Ok(mut state) = self.state.try_borrow_mut() {
            state.timeout(id);
        }
    }

    pub(crate) fn idle(&self) {
        if let Ok(mut state) = self.state.try_borrow_mut() {
            state.idle();
        }
    }

    /// Stops the channel now, as the idle timer would.
    #[cfg(feature = "page-reader-probe")]
    pub(crate) fn stop_now(&self) {
        if let Ok(mut state) = self.state.try_borrow_mut() {
            state.stop_channel();
            state.publish();
        }
    }
}

impl Drop for BrowseReader {
    fn drop(&mut self) {
        if let Ok(mut state) = self.state.try_borrow_mut() {
            state.disable();
            crate::page_reader::forget(state.tab());
        }
    }
}

struct BootIvars {
    controller: Retained<WKUserContentController>,
    world: Retained<WKContentWorld>,
    name: Retained<NSString>,
    view: usize,
    reader: Weak<RefCell<ReaderState>>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "ZephiumPageAssistBootHandler"]
    #[ivars = BootIvars]
    struct BootHandler;

    // SAFETY: `define_class!` fixes `NSObject` as the superclass and objc2
    // initializes the declared ivars before exposing an instance.
    unsafe impl NSObjectProtocol for BootHandler {}

    // SAFETY: the selector and parameters exactly implement WebKit's
    // reply-handler protocol; `MainThreadOnly` keeps the object on the main
    // thread and callback state lives in ivars.
    unsafe impl WKScriptMessageHandlerWithReply for BootHandler {
        #[unsafe(method(userContentController:didReceiveScriptMessage:replyHandler:))]
        unsafe fn user_content_controller_did_receive_script_message_reply_handler(
            &self,
            controller: &WKUserContentController,
            message: &WKScriptMessage,
            reply: &block2::DynBlock<dyn Fn(*mut AnyObject, *mut NSString)>,
        ) {
            let ivars = self.ivars();
            let reply = reply.copy();
            // SAFETY: WebKit supplied live callback objects for this exact
            // selector; objc2 retains each returned property.
            let (world, name, webview, frame) = unsafe {
                (
                    message.world(),
                    message.name(),
                    message.webView(),
                    message.frameInfo(),
                )
            };
            // SAFETY: `frame` is the live retained WKFrameInfo of this
            // message, read on WebKit's main-thread delivery path.
            let (frame_webview, main_frame) = unsafe { (frame.webView(), frame.isMainFrame()) };
            let same_view = |view: Option<&WKWebView>| {
                view.is_some_and(|view| std::ptr::from_ref(view).addr() == ivars.view)
            };
            if !std::ptr::eq(controller, &*ivars.controller)
                || Retained::as_ptr(&world) != Retained::as_ptr(&ivars.world)
                || !name.isEqualToString(&ivars.name)
                || !same_view(webview.as_deref())
                || !same_view(frame_webview.as_deref())
                || !main_frame
            {
                refuse(reply);
                return;
            }
            // SAFETY: `message` is live for this callback; objc2 retains its
            // body before the type and size checks.
            let body = unsafe { message.body() };
            let Ok(body) = body.downcast::<NSString>() else {
                refuse(reply);
                return;
            };
            if body.length() > 2 || body.lengthOfBytesUsingEncoding(NSUTF8StringEncoding) > 2 {
                refuse(reply);
                return;
            }
            let body = body.to_string();
            if ![BOOT_FRESH, BOOT_INSTANTIATED, BOOT_FAILED].contains(&body.as_str()) {
                refuse(reply);
                return;
            }
            let Some(reader) = ivars.reader.upgrade() else {
                let _ = answer(reply, STOP);
                return;
            };
            let Ok(mut state) = reader.try_borrow_mut() else {
                let _ = answer(reply, STOP);
                return;
            };
            state.boot_message(&body, reply);
            let tab = state.tab();
            drop(state);
            schedule_pump(tab);
        }
    }
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_document_start_piece_stays_small_and_closed() {
        assert!(BOOTSTRAP.len() <= 1_024, "{}", BOOTSTRAP.len());
        assert!(BOOTSTRAP.is_ascii());
        assert_eq!(BOOTSTRAP.matches(BOOT_HANDLER).count(), 1);
        for forbidden in ["eval(", "document.", "fetch(", "XMLHttpRequest", "import("] {
            assert!(!BOOTSTRAP.contains(forbidden), "{forbidden}");
        }
        for token in [BOOT_FRESH, BOOT_INSTANTIATED, BOOT_FAILED, RESTART, STOP] {
            assert!(BOOTSTRAP.contains(&format!("\"{token}\"")), "{token}");
        }
    }

    #[test]
    fn the_program_is_the_pinned_runtime_then_the_fixed_recipes() {
        let program = program();
        assert!(program.is_ascii());
        assert!(
            program.starts_with(zephium_agentic::SEMANTIC_RUNTIME_PROGRAM.installation_source())
        );
        assert_eq!(program.matches(RECIPES).count(), 1);
        assert!(program.ends_with(SERVE));
        assert_eq!(SERVE.matches("handlers.zephiumPageAssistV1").count(), 1);
    }

    #[test]
    fn world_names_are_unpredictable() {
        let a = world_name();
        let b = world_name();
        assert_ne!(a, b);
        assert!(a.starts_with(WORLD_PREFIX) && a.len() == WORLD_PREFIX.len() + 32);
    }
}
