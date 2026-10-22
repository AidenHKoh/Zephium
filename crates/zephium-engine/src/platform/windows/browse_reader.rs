#![deny(unsafe_op_in_unsafe_fn)]
#![deny(clippy::undocumented_unsafe_blocks)]
#![deny(clippy::dbg_macro, clippy::print_stderr, clippy::print_stdout)]
#![cfg_attr(
    not(test),
    deny(clippy::panic, clippy::unreachable, clippy::unwrap_used)
)]

//! The Ask panel's reader for a person's own Browse tab on WebView2.
//!
//! Nothing is installed at document start. On the first ask in a document
//! the reader creates a fresh, unpredictably named isolated world for its
//! root frame with universal access denied, joins that world's execution
//! context, then installs the pinned runtime and the fixed recipes there
//! with `Runtime.callFunctionOn` and `userGesture: false`. Reads and recipes
//! are the same closed calls. All of it travels as the closed CDP vocabulary
//! of `agent_semantic_cdp_protocol`, one request at a time, on WebView2's
//! own DevTools session, so extensions and the person's DevTools keep
//! theirs. A commit forgets the context; a context that is gone reads as a
//! changed page.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use webview2_com::DevToolsProtocolEventReceivedEventHandler;
use webview2_com::Microsoft::Web::WebView2::Win32::{
    ICoreWebView2, ICoreWebView2DevToolsProtocolEventReceiver,
};
use windows_core::{HSTRING, PWSTR};
use zephium_agentic::{ContextJoin, SemanticRuntimeFault, SemanticRuntimeResultError};
use zephium_core::ids::ItemId;

use super::semantic_runtime::{call_cdp_async, NativeCdpFailure};
use crate::page_reader::{PageReadFailure, PageReadout};
use crate::platform::agent_semantic_cdp_protocol::{
    decode_empty_success, decode_invocation_response, decode_page_recipe_response,
    decode_root_frame, decode_runtime_install_response, get_frame_tree_command,
    install_page_recipes_in_context_command, install_runtime_in_context_command,
    invoke_page_recipe_command, invoke_runtime_command, runtime_disable_command,
    runtime_enable_command, FixedSemanticCdpCommand, SemanticCdpInvocationError,
    SemanticContextDiscovery, SemanticExecutionContext, SemanticWorldName, MAX_CONTEXT_EVENT_BYTES,
};
use crate::platform::page_recipe::PageRecipe;

const MAX_QUEUED: usize = 8;
const READ_TIMEOUT: Duration = Duration::from_secs(12);
const RECIPE_TIMEOUT: Duration = Duration::from_secs(5);
static NEXT_WORLD: AtomicU64 = AtomicU64::new(1);

pub(crate) type ReadDone = Box<dyn FnOnce(Result<PageReadout, PageReadFailure>) + Send>;
pub(crate) type RecipeDone = Box<dyn FnOnce(Option<String>) + Send>;

fn next_world_name() -> Option<SemanticWorldName> {
    let epoch = NEXT_WORLD
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .ok()?;
    let unpredictable = windows_core::GUID::new().ok()?.to_u128();
    SemanticWorldName::from_nonce(epoch, unpredictable).ok()
}

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

/// The operation being served, once its invocation is encoded.
enum Serving {
    Read {
        invocation: Box<zephium_agentic::SemanticRuntimeInvocation>,
        pending: Box<crate::page_reader::read::PendingRead>,
        done: ReadDone,
    },
    Recipe {
        recipe: PageRecipe,
        done: RecipeDone,
    },
}

impl Serving {
    fn fail(self, failure: PageReadFailure) {
        match self {
            Self::Read { done, .. } => done(Err(failure)),
            Self::Recipe { done, .. } => done(None),
        }
    }

    fn command(&self, context: &SemanticExecutionContext) -> Option<FixedSemanticCdpCommand> {
        match self {
            Self::Read { invocation, .. } => invoke_runtime_command(context, invocation).ok(),
            Self::Recipe { recipe, .. } => invoke_page_recipe_command(context, recipe).ok(),
        }
    }
}

struct Current {
    id: u64,
    epoch: u64,
    serving: Serving,
    _timer: Option<super::ContentPolicyTimeout>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Stage {
    FrameTree,
    Enable,
    CreateWorld,
    Disable,
    InstallRuntime,
    InstallRecipes,
    Invoke,
    Cleanup,
}

struct Events {
    receiver: ICoreWebView2DevToolsProtocolEventReceiver,
    token: i64,
}

impl Drop for Events {
    fn drop(&mut self) {
        // SAFETY: `receiver` is live and `token` is the exact value its
        // successful add returned; this runs once per registration.
        let _ = unsafe {
            self.receiver
                .remove_DevToolsProtocolEventReceived(self.token)
        };
    }
}

struct State {
    core: ICoreWebView2,
    tab: Rc<Cell<ItemId>>,
    join: ContextJoin,
    enabled: bool,
    committed: Option<u64>,
    /// Changes whenever native work in flight loses its document.
    generation: u64,
    world: Option<SemanticWorldName>,
    discovery: Option<SemanticContextDiscovery>,
    events: Option<Events>,
    runtime_enabled: bool,
    context: Option<(u64, SemanticExecutionContext)>,
    in_flight: Option<(u64, Stage)>,
    queue: VecDeque<Op>,
    current: Option<Current>,
    sequence: u64,
    committed_now: Rc<dyn Fn() -> Option<u64>>,
    weak: Weak<RefCell<State>>,
}

fn schedule_pump(tab: ItemId) {
    let _ = crate::host::try_with(move |host| host.page_reader_pump(tab));
}

fn map_read_failure(error: SemanticCdpInvocationError) -> PageReadFailure {
    match error {
        SemanticCdpInvocationError::Result(SemanticRuntimeResultError::Runtime(
            SemanticRuntimeFault::DocumentLoading,
        )) => PageReadFailure::NotReady,
        SemanticCdpInvocationError::Result(_) | SemanticCdpInvocationError::Protocol(_) => {
            PageReadFailure::Unavailable
        }
    }
}

impl State {
    fn tab(&self) -> ItemId {
        self.tab.get()
    }

    /// Native work in flight belongs to a document that is gone or a
    /// request that ended: drop the context and every discovery piece.
    fn forget_document(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.world = None;
        self.discovery = None;
        self.events = None;
        self.context = None;
    }

    fn fail_current(&mut self, failure: PageReadFailure) {
        if let Some(current) = self.current.take() {
            let failure = if (self.committed_now)() == Some(current.epoch) {
                failure
            } else {
                PageReadFailure::PageChanged
            };
            current.serving.fail(failure);
        }
    }

    fn send(&mut self, stage: Stage, command: Option<FixedSemanticCdpCommand>) {
        let Some(command) = command else {
            self.fail_current(PageReadFailure::Unavailable);
            self.forget_document();
            self.cleanup();
            return;
        };
        if self.in_flight.is_some() {
            self.fail_current(PageReadFailure::Unavailable);
            return;
        }
        let generation = self.generation;
        self.in_flight = Some((generation, stage));
        if stage == Stage::Enable {
            self.runtime_enabled = true;
        }
        let weak = self.weak.clone();
        let tab = self.tab();
        let sent = call_cdp_async(
            &self.core,
            command,
            move |outcome| {
                if let Some(state) = weak.upgrade() {
                    if let Ok(mut state) = state.try_borrow_mut() {
                        state.completed(generation, stage, outcome);
                    }
                }
                schedule_pump(tab);
            },
            Rc::new(|| {}),
        );
        if sent.is_err() {
            self.in_flight = None;
            self.fail_current(PageReadFailure::Unavailable);
            self.forget_document();
        }
    }

    /// Turns Runtime reporting back off when a discovery was abandoned.
    fn cleanup(&mut self) {
        if self.runtime_enabled && self.in_flight.is_none() {
            self.runtime_enabled = false;
            self.send(Stage::Cleanup, Some(runtime_disable_command()));
        }
    }

    fn completed(
        &mut self,
        generation: u64,
        stage: Stage,
        outcome: Result<String, NativeCdpFailure>,
    ) {
        if self.in_flight != Some((generation, stage)) {
            return;
        }
        self.in_flight = None;
        let ok = |outcome: &Result<String, NativeCdpFailure>| {
            outcome
                .as_deref()
                .is_ok_and(|response| decode_empty_success(response).is_ok())
        };
        if matches!(stage, Stage::Disable | Stage::Cleanup) && ok(&outcome) {
            self.runtime_enabled = false;
        }
        if generation != self.generation || stage == Stage::Cleanup {
            self.cleanup();
            return;
        }
        let Ok(response) = outcome else {
            // A context that is gone (the document changed under the call)
            // ends the request; the next ask joins a fresh world.
            self.fail_current(PageReadFailure::PageChanged);
            self.forget_document();
            self.cleanup();
            return;
        };
        match stage {
            Stage::FrameTree => {
                let (Ok(root), Some(world)) = (decode_root_frame(&response), self.world.clone())
                else {
                    return self.abandon();
                };
                self.discovery = Some(SemanticContextDiscovery::new(world, root));
                if self.listen().is_err() {
                    return self.abandon();
                }
                self.send(Stage::Enable, Some(runtime_enable_command()));
            }
            Stage::Enable => {
                if decode_empty_success(&response).is_err() {
                    return self.abandon();
                }
                let command = self
                    .discovery
                    .as_ref()
                    .and_then(|discovery| discovery.create_world_command().ok());
                self.send(Stage::CreateWorld, command);
            }
            Stage::CreateWorld => {
                let Some(discovery) = self.discovery.as_mut() else {
                    return self.abandon();
                };
                if discovery.record_created_context(&response).is_err() {
                    return self.abandon();
                }
                if discovery.resolved().ok().flatten().is_some() {
                    self.send(Stage::Disable, Some(runtime_disable_command()));
                }
            }
            Stage::Disable => {
                if decode_empty_success(&response).is_err() {
                    return self.abandon();
                }
                self.events = None;
                let context = self
                    .discovery
                    .take()
                    .and_then(|discovery| discovery.resolved().ok().flatten());
                let (Some(context), Some(current)) = (context, self.current.as_ref()) else {
                    return self.abandon();
                };
                self.context = Some((current.epoch, context.clone()));
                self.send(
                    Stage::InstallRuntime,
                    install_runtime_in_context_command(&context).ok(),
                );
            }
            Stage::InstallRuntime => {
                if decode_runtime_install_response(&response).is_err() {
                    return self.abandon();
                }
                let command = self
                    .context
                    .as_ref()
                    .and_then(|(_, context)| install_page_recipes_in_context_command(context).ok());
                self.send(Stage::InstallRecipes, command);
            }
            Stage::InstallRecipes => {
                if decode_runtime_install_response(&response).is_err() {
                    return self.abandon();
                }
                self.invoke();
            }
            Stage::Invoke => self.settle(&response),
            Stage::Cleanup => {}
        }
    }

    fn abandon(&mut self) {
        self.fail_current(PageReadFailure::Unavailable);
        self.forget_document();
        self.cleanup();
    }

    fn listen(&mut self) -> windows_core::Result<()> {
        let event_name = HSTRING::from("Runtime.executionContextCreated");
        // SAFETY: `core` is a live STA-bound COM interface and `event_name`
        // a valid HSTRING for this synchronous getter.
        let receiver = unsafe { self.core.GetDevToolsProtocolEventReceiver(&event_name)? };
        let weak = self.weak.clone();
        let tab = self.tab();
        let handler =
            DevToolsProtocolEventReceivedEventHandler::create(Box::new(move |_, args| {
                let Some(args) = args else {
                    return Ok(());
                };
                let mut raw = PWSTR::null();
                // SAFETY: WebView2 supplied a live argument interface; `raw` is
                // initialized out storage the bounded helper frees exactly once.
                if unsafe { args.ParameterObjectAsJson(&mut raw) }.is_err() {
                    return Ok(());
                }
                let Some(event) = super::take_pwstr_bounded(
                    raw,
                    MAX_CONTEXT_EVENT_BYTES,
                    MAX_CONTEXT_EVENT_BYTES,
                ) else {
                    return Ok(());
                };
                if let Some(state) = weak.upgrade() {
                    if let Ok(mut state) = state.try_borrow_mut() {
                        state.context_created(&event);
                    }
                }
                schedule_pump(tab);
                Ok(())
            }));
        let mut token = 0_i64;
        // SAFETY: receiver and handler are live COM interfaces and `token` is
        // exact out storage; removal happens when `Events` drops.
        unsafe { receiver.add_DevToolsProtocolEventReceived(&handler, &mut token)? };
        self.events = Some(Events { receiver, token });
        Ok(())
    }

    fn context_created(&mut self, event: &str) {
        let Some(discovery) = self.discovery.as_mut() else {
            return;
        };
        if discovery.observe_context_created(event).is_err() {
            return self.abandon();
        }
        if discovery.resolved().ok().flatten().is_some() && self.in_flight.is_none() {
            self.send(Stage::Disable, Some(runtime_disable_command()));
        }
    }

    fn invoke(&mut self) {
        let command = match (self.context.as_ref(), self.current.as_ref()) {
            (Some((epoch, context)), Some(current)) if *epoch == current.epoch => {
                current.serving.command(context)
            }
            _ => None,
        };
        self.send(Stage::Invoke, command);
    }

    fn settle(&mut self, response: &str) {
        let Some(current) = self.current.take() else {
            return;
        };
        if (self.committed_now)() != Some(current.epoch) {
            return current.serving.fail(PageReadFailure::PageChanged);
        }
        let epoch = current.epoch;
        match current.serving {
            Serving::Read {
                invocation,
                pending,
                done,
            } => {
                let outcome = decode_invocation_response(&invocation, response)
                    .map_err(map_read_failure)
                    .and_then(|snapshot| crate::page_reader::read::passages(*pending, snapshot, 1))
                    .map(|passages| PageReadout {
                        epoch,
                        title: title(&self.core),
                        passages,
                    });
                if outcome == Err(PageReadFailure::Unavailable) {
                    self.forget_document();
                }
                done(outcome);
            }
            Serving::Recipe { recipe, done } => {
                let answer = decode_page_recipe_response(&recipe, response).ok();
                if answer.is_none() {
                    self.forget_document();
                }
                done(answer);
            }
        }
    }

    fn pump(&mut self) {
        while self.current.is_none() && self.in_flight.is_none() {
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
            self.sequence = self.sequence.saturating_add(1);
            let id = self.sequence;
            let (serving, limit) = match op {
                Op::Read { url, done, .. } => {
                    let Some((invocation, pending)) =
                        crate::page_reader::read::invocation(self.join, &url, id)
                    else {
                        done(Err(PageReadFailure::Unavailable));
                        continue;
                    };
                    (
                        Serving::Read {
                            invocation: Box::new(invocation),
                            pending: Box::new(pending),
                            done,
                        },
                        READ_TIMEOUT,
                    )
                }
                Op::Recipe { recipe, done, .. } => {
                    (Serving::Recipe { recipe, done }, RECIPE_TIMEOUT)
                }
            };
            let tab = self.tab();
            self.current = Some(Current {
                id,
                epoch,
                serving,
                _timer: super::schedule_browser_timeout(limit, move || {
                    let _ = crate::host::try_with(move |host| host.page_reader_timeout(tab, id));
                }),
            });
            if self
                .context
                .as_ref()
                .is_some_and(|(bound, _)| *bound == epoch)
            {
                self.invoke();
            } else {
                self.forget_document();
                self.world = next_world_name();
                if self.world.is_none() {
                    self.fail_current(PageReadFailure::Unavailable);
                    continue;
                }
                self.send(Stage::FrameTree, Some(get_frame_tree_command()));
            }
        }
    }

    fn commit(&mut self, epoch: u64) {
        self.committed = Some(epoch);
        crate::page_reader::publish_commit(self.tab(), epoch);
        crate::page_reader::publish_ready(self.tab(), epoch, self.enabled);
        if self
            .context
            .as_ref()
            .is_some_and(|(bound, _)| *bound != epoch)
            || self.current.is_some()
            || self.discovery.is_some()
        {
            self.fail_current(PageReadFailure::PageChanged);
            self.forget_document();
            self.cleanup();
        }
    }

    fn timeout(&mut self, id: u64) {
        if self
            .current
            .as_ref()
            .is_some_and(|current| current.id == id)
        {
            self.fail_current(PageReadFailure::Unavailable);
            self.forget_document();
            self.cleanup();
        }
    }

    fn disable(&mut self) {
        self.enabled = false;
        self.fail_current(PageReadFailure::Unavailable);
        while let Some(op) = self.queue.pop_front() {
            op.fail(PageReadFailure::Unavailable);
        }
        self.forget_document();
        self.cleanup();
        if let Some(epoch) = self.committed {
            crate::page_reader::publish_ready(self.tab(), epoch, false);
        }
    }
}

fn title(core: &ICoreWebView2) -> Option<String> {
    let mut raw = PWSTR::null();
    // SAFETY: `core` is live on its STA and `raw` is initialized out storage
    // the bounded helper frees exactly once.
    unsafe { core.DocumentTitle(&mut raw) }.ok()?;
    let title = super::take_pwstr_bounded(raw, 4 * 1_024, 16 * 1_024)?;
    Some(zephium_core::item::sanitize_page_title(&title)).filter(|title| !title.is_empty())
}

/// One Browse view's reader. Lives with the view and is dropped before it.
pub(crate) struct BrowseReader {
    state: Rc<RefCell<State>>,
}

impl BrowseReader {
    pub(crate) fn new(
        core: ICoreWebView2,
        tab: Rc<Cell<ItemId>>,
        enabled: bool,
        committed_now: Rc<dyn Fn() -> Option<u64>>,
    ) -> Option<Self> {
        let join = crate::page_reader::read::reader_context()?;
        Some(Self {
            state: Rc::new_cyclic(|weak| {
                RefCell::new(State {
                    core,
                    tab,
                    join,
                    enabled,
                    committed: None,
                    generation: 1,
                    world: None,
                    discovery: None,
                    events: None,
                    runtime_enabled: false,
                    context: None,
                    in_flight: None,
                    queue: VecDeque::new(),
                    current: None,
                    sequence: 0,
                    committed_now,
                    weak: weak.clone(),
                })
            }),
        })
    }

    /// WebView2 has no document-start piece; the attempt changes nothing.
    pub(crate) fn attempt(&self, _admitted: bool) {}

    /// Each ask joins a fresh world on WebView2, so restores need nothing.
    pub(crate) fn back_forward(&self, _back_forward: bool) {}

    pub(crate) fn commit(&self, epoch: u64, _url: &str) {
        if let Ok(mut state) = self.state.try_borrow_mut() {
            state.commit(epoch);
            state.pump();
        }
    }

    pub(crate) fn set_enabled(&self, enabled: bool) {
        let Ok(mut state) = self.state.try_borrow_mut() else {
            return;
        };
        if enabled {
            state.enabled = true;
            if let Some(epoch) = state.committed {
                crate::page_reader::publish_ready(state.tab(), epoch, true);
            }
        } else if state.enabled {
            state.disable();
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
            state.pump();
        }
    }
}

impl Drop for BrowseReader {
    fn drop(&mut self) {
        let Ok(mut state) = self.state.try_borrow_mut() else {
            return;
        };
        state.enabled = false;
        state.fail_current(PageReadFailure::PageChanged);
        while let Some(op) = state.queue.pop_front() {
            op.fail(PageReadFailure::PageChanged);
        }
        state.forget_document();
        crate::page_reader::forget(state.tab());
    }
}
