//! Headless loopback qualification of the Ask panel's Browse reader.
//!
//! A WKWebView in a window that is never ordered front loads fixed loopback
//! fixtures and is driven through the production reader exactly as a Browse
//! view drives it: the main-frame navigation attempt hook and the committed
//! navigation epoch. The page reports only through its title; nothing here
//! evaluates JavaScript, so nothing here can confer user activation either.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ffi::c_void;
use std::io::{Read as _, Write as _};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::ptr::NonNull;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use objc2::rc::Retained;
use objc2::MainThreadOnly as _;
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSView, NSWindow,
    NSWindowStyleMask,
};
use objc2_foundation::{MainThreadMarker, NSDate, NSPoint, NSRect, NSRunLoop, NSSize};
use raw_window_handle::{
    AppKitWindowHandle, HandleError, HasWindowHandle, RawWindowHandle, WindowHandle,
};
use zephium_core::ids::ItemId;

use super::browse_reader::BrowseReader;
use crate::navigation_epoch::{NavigationEpochTracker, NavigationTransition};
use crate::page_reader::{PageReadFailure, PageReadout, PageRole};
use crate::platform::page_recipe::{mark_outcome, parse_outline, MarkOutcome, PageRecipe};

const STEP_TIMEOUT: Duration = Duration::from_secs(20);

const ARTICLE: &str = r#"<!doctype html><html><head><meta charset="utf-8"><title>Rivers of the North</title>
<style>body{font-family:Georgia,serif;background:#fbfaf7;color:#1d1d1f}a{color:#0b57d0}.card{padding:12px;margin:8px;background:#fff;width:300px;height:60px}button{background:#d93025;color:#fff}</style></head>
<body><header class="site-head css-1x2y3z"><nav class="top-nav"><a href="/next">Next story</a> <a href="https://example.org/">Elsewhere</a></nav></header>
<main id="content"><article class="story">
<h1>Rivers of the North</h1>
<p id="lead">The northern rivers rose two metres overnight after the storm.</p>
<h2>What happened</h2>
<p>Engineers opened the <span id="pick">eastern floodgates at dawn</span> to relieve the pressure on the old weir.</p>
<ul><li>Roads near the Ness were closed.</li><li>Schools opened late.</li></ul>
<table><tr><th>River</th><th>Rise</th></tr><tr><td>Ness</td><td>2 m</td></tr></table>
<div class="card kCsWcF">Card one</div><div class="card kCsWcF">Card two</div><div class="card kCsWcF">Card three</div>
<form onsubmit="return false"><label>Email <input type="email" name="email" value="person@example.com"></label>
<label>Password <input type="password" name="password" value="hunter2-secret"></label>
<label>Card number <input type="text" name="cardnumber" autocomplete="cc-number" value="4111 1111 1111 1111"></label>
<label>Code <input type="text" name="otp" autocomplete="one-time-code" value="928374"></label>
<label>Search <input type="search" name="q" value="flood maps"></label><button type="submit">Send report</button></form>
</article><aside class="related">Related stories</aside></main>
<footer class="site-foot">Fixture footer</footer>
<script src="/watch.js"></script></body></html>"#;

// Runs in the page's own world. It watches, as tightly as the page can,
// for any user activation, any popup that gets through, and any sign of the
// reader's private world; it reports only when the probe asks.
const WATCH: &str = r##"(() => {
  const pick = document.getElementById("pick");
  if (pick) getSelection().selectAllChildren(pick);
  const shape = () => [document.documentElement.scrollHeight, document.getElementById("lead").offsetHeight, pick.offsetWidth, pick.offsetLeft].join(",");
  const initial = shape();
  let active = false, been = false, opened = 0, attempts = 0, samples = 0, marked = false;
  const leaks = new Set();
  const names = ["__zephiumSemanticRuntimeV1", "__zephiumPageAssistV1", "__zephiumHumanSurveyV1", "__zephiumRecordMediaV1", "__zephiumPixelPrivacyV1"];
  let running = true;
  const channel = new MessageChannel();
  channel.port1.onmessage = () => {
    if (!running) return;
    samples++;
    const ua = navigator.userActivation;
    if (ua && ua.isActive) active = true;
    if (ua && ua.hasBeenActive) been = true;
    attempts++;
    try { const w = window.open("about:blank", "_blank"); if (w) { opened++; try { w.close(); } catch (_) {} } } catch (_) {}
    if ((samples & 63) === 0) {
      for (const name of names) if (name in globalThis) leaks.add(name);
      try {
        const h = window.webkit && window.webkit.messageHandlers;
        if (h && (h.zephiumPageAssistBootV1 || h.zephiumPageAssistV1 || h.zephiumSemanticRuntimeV1)) leaks.add("handler");
      } catch (_) {}
    }
    try { if (globalThis.CSS && CSS.highlights && CSS.highlights.size > 0) marked = true; } catch (_) {}
    channel.port2.postMessage(0);
  };
  channel.port2.postMessage(0);
  addEventListener("hashchange", () => {
    if (!location.hash.startsWith("#report")) return;
    running = false;
    document.title = `report ${location.hash.slice(7)} sheets=${document.adoptedStyleSheets.length} layout=${shape() === initial ? "same" : "changed"} active=${active} been=${been} opened=${opened} attempts=${attempts} samples=${samples} marked=${marked} highlights=${globalThis.CSS && CSS.highlights ? [...CSS.highlights.keys()].join(",") : "none"} leaks=${[...leaks].join("|") || "none"}`;
  });
})();"##;

const CACHE_A: &str = r#"<!doctype html><meta charset="utf-8"><title>A</title><h1>Page A</h1><p>Alpha text only page A has.</p><script>addEventListener("pageshow", e => { if (e.persisted) document.title = "A restored"; });</script>"#;
const CACHE_B: &str = r#"<!doctype html><meta charset="utf-8"><title>B</title><h1>Page B</h1><p>Beta text only page B has.</p><script>addEventListener("pageshow", e => { if (e.persisted) document.title = "B restored"; });</script>"#;

// A form whose name field behaves like a controlled input: the page keeps
// its own state from input events and renders the field from it.
const FORM: &str = r#"<!doctype html><meta charset="utf-8"><title>Checkout</title>
<style>body{font:16px sans-serif} input,button,select{display:block;margin:8px;width:240px;height:28px}</style>
<main><h1>Checkout</h1>
<label>Name <input id="name" type="text" value=""></label>
<label>Password <input id="pw" type="password" value=""></label>
<label>Size <select id="size"><option>Small</option><option>Large</option></select></label>
<label><input id="gift" type="checkbox" style="width:auto"> Gift wrap</label>
<button id="slow" type="button">Check stock</button>
<button id="order" type="button">Place order</button>
<p id="echo">state:</p><a href="/next">Next story</a></main>
<script src="/form.js"></script>"#;
const FORM_JS: &str = r#"(() => {
  const name = document.getElementById("name"), echo = document.getElementById("echo");
  let state = "";
  const render = () => { name.value = state; echo.textContent = "state:" + state; };
  name.addEventListener("input", () => { state = name.value; render(); });
  name.addEventListener("blur", render);
  document.getElementById("order").addEventListener("click", () => { echo.textContent = "ordered"; });
  document.getElementById("slow").addEventListener("click", () => { const end = performance.now() + 600; while (performance.now() < end) {} });
})();"#;

const NEXT: &str = r#"<!doctype html><meta charset="utf-8"><title>Next</title><h1>The next story</h1><p>A second document for navigation checks.</p>"#;

fn heavy() -> String {
    let mut page =
        String::from("<!doctype html><meta charset=\"utf-8\"><title>Heavy</title><main><article>");
    for section in 0..200 {
        page.push_str(&format!("<h2>Section {section}</h2>"));
        for paragraph in 0..15 {
            page.push_str(&format!(
                "<p>Paragraph {paragraph} of section {section}: the committee reviewed the long record of river levels, \
                 rainfall, and the maintenance history of every gate along the northern channel before it agreed.</p>"
            ));
        }
        page.push_str("<ul><li>One point</li><li>Another point</li><li>A last point</li></ul>");
    }
    page.push_str("</article></main>");
    page
}

struct Fixture {
    address: SocketAddr,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl Fixture {
    fn start() -> Result<Self, String> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(|e| format!("bind: {e}"))?;
        let address = listener.local_addr().map_err(|e| format!("address: {e}"))?;
        listener
            .set_nonblocking(true)
            .map_err(|e| format!("nonblocking: {e}"))?;
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = stop.clone();
        let heavy = Arc::new(heavy());
        let worker = std::thread::Builder::new()
            .name("zephium-page-reader-probe-http".into())
            .spawn(move || {
                while !thread_stop.load(Ordering::Acquire) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            let heavy = heavy.clone();
                            let _ = std::thread::Builder::new()
                                .name("zephium-page-reader-probe-connection".into())
                                .spawn(move || serve(stream, &heavy));
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(2));
                        }
                        Err(_) => break,
                    }
                }
            })
            .map_err(|e| format!("spawn: {e}"))?;
        Ok(Self {
            address,
            stop,
            worker: Some(worker),
        })
    }

    fn url(&self, path: &str) -> String {
        format!("http://{}{path}", self.address)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn serve(mut stream: TcpStream, heavy: &str) {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let mut request = [0_u8; 8 * 1024];
    let mut received = 0;
    while received < request.len() {
        match stream.read(&mut request[received..]) {
            Ok(0) | Err(_) => return,
            Ok(read) => {
                received += read;
                if request[..received].windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
        }
    }
    let head = String::from_utf8_lossy(&request[..received]);
    let path = head
        .split_whitespace()
        .nth(1)
        .unwrap_or("/")
        .split('#')
        .next()
        .unwrap_or("/")
        .to_owned();
    let (body, kind, csp): (&str, &str, Option<&str>) = match path.as_str() {
        "/watch.js" => (WATCH, "text/javascript", None),
        "/form.js" => (FORM_JS, "text/javascript", None),
        "/form" => (FORM, "text/html; charset=utf-8", None),
        "/article" | "/sensitive" => (ARTICLE, "text/html; charset=utf-8", None),
        "/plain" => (
            ARTICLE.trim_end_matches("<script src=\"/watch.js\"></script></body></html>"),
            "text/html; charset=utf-8",
            None,
        ),
        "/csp" => (
            ARTICLE,
            "text/html; charset=utf-8",
            Some("script-src 'self'; object-src 'none'; base-uri 'none'"),
        ),
        "/tt" => (
            ARTICLE,
            "text/html; charset=utf-8",
            Some("script-src 'self'; require-trusted-types-for 'script'; trusted-types 'none'"),
        ),
        "/strict" => (
            ARTICLE,
            "text/html; charset=utf-8",
            Some("default-src 'self'; style-src 'self' 'unsafe-inline'"),
        ),
        "/heavy" => (heavy, "text/html; charset=utf-8", None),
        "/cache-a" => (CACHE_A, "text/html; charset=utf-8", None),
        "/cache-b" => (CACHE_B, "text/html; charset=utf-8", None),
        "/next" => (NEXT, "text/html; charset=utf-8", None),
        _ => ("not found", "text/plain", None),
    };
    // The back-forward pages are cacheable, as most of the web is.
    let cache = if path.starts_with("/cache-") {
        "Cache-Control: max-age=60"
    } else {
        "Cache-Control: no-store"
    };
    let mut response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {kind}\r\nContent-Length: {}\r\n{cache}\r\nConnection: close\r\n",
        body.len()
    );
    if let Some(csp) = csp {
        response.push_str(&format!("Content-Security-Policy: {csp}\r\n"));
    }
    response.push_str("\r\n");
    response.push_str(body);
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

struct Host {
    view: Retained<NSView>,
}

impl HasWindowHandle for Host {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let pointer = NonNull::from(&*self.view).cast::<c_void>();
        let raw = RawWindowHandle::AppKit(AppKitWindowHandle::new(pointer));
        // SAFETY: `self.view` owns the exact NSView for this borrow and the
        // retained window outlives the Wry child.
        Ok(unsafe { WindowHandle::borrow_raw(raw) })
    }
}

#[derive(Default)]
struct PageState {
    title: String,
    popups: usize,
    finished: u64,
}

struct Probe {
    _window: Retained<NSWindow>,
    _host: Host,
    view: wry::WebView,
    reader: Rc<RefCell<Option<BrowseReader>>>,
    navigation: NavigationEpochTracker,
    state: Rc<RefCell<PageState>>,
    sensitive: Rc<Cell<bool>>,
    tab: ItemId,
    tab_cell: Rc<Cell<ItemId>>,
}

fn pump(until: impl Fn() -> bool, what: &str) -> Result<(), String> {
    let run_loop = NSRunLoop::mainRunLoop();
    let deadline = Instant::now() + STEP_TIMEOUT;
    while !until() {
        if Instant::now() >= deadline {
            return Err(format!("timed out {what}"));
        }
        objc2::rc::autoreleasepool(|_| {
            run_loop.runUntilDate(&NSDate::dateWithTimeIntervalSinceNow(0.002));
        });
    }
    Ok(())
}

fn settle(duration: Duration) {
    let end = Instant::now() + duration;
    let _ = pump(|| Instant::now() >= end, "settling");
}

impl Probe {
    fn new(mtm: MainThreadMarker, enabled: bool) -> Result<Self, String> {
        // SAFETY: `mtm` proves AppKit affinity; the window is never ordered
        // front and outlives its host view and the Wry child.
        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(mtm),
                NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(1024.0, 768.0)),
                NSWindowStyleMask::Borderless,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        // SAFETY: the probe keeps the retained window until teardown.
        unsafe { window.setReleasedWhenClosed(false) };
        let host = Host {
            view: window
                .contentView()
                .ok_or_else(|| "probe window has no content view".to_owned())?,
        };
        let state = Rc::new(RefCell::new(PageState::default()));
        let reader: Rc<RefCell<Option<BrowseReader>>> = Rc::default();
        let navigation = NavigationEpochTracker::new();
        let sensitive = Rc::new(Cell::new(false));
        let tab = ItemId::generate();
        let title_state = state.clone();
        let popup_state = state.clone();
        let attempt_reader = Rc::downgrade(&reader);
        let attempt_sensitive = sensitive.clone();
        let commit_reader = Rc::downgrade(&reader);
        let kind_reader = Rc::downgrade(&reader);
        let commit_navigation = navigation.clone();
        let finish_state = state.clone();
        let view = wry::WebViewBuilder::new()
            .with_bounds(wry::Rect {
                position: wry::dpi::LogicalPosition::new(0.0, 0.0).into(),
                size: wry::dpi::LogicalSize::new(1024.0, 768.0).into(),
            })
            .with_incognito(true)
            .with_document_title_changed_handler(move |title| {
                title_state.borrow_mut().title = title;
            })
            .with_new_window_req_handler(move |_url, _features| {
                popup_state.borrow_mut().popups += 1;
                wry::NewWindowResponse::Deny
            })
            .with_apple_navigation_action_handler(move |_target, action| {
                if action.target_is_main_frame == Some(true) {
                    if let Some(reader) = kind_reader.upgrade() {
                        if let Some(reader) = reader.borrow().as_ref() {
                            reader.back_forward(
                                action.navigation_type == wry::AppleNavigationType::BackForward,
                            );
                        }
                    }
                }
                true
            })
            .with_main_frame_navigation_attempt_handler(move |target| {
                let admitted = enabled_and_admitted(&attempt_sensitive, &target);
                if let Some(reader) = attempt_reader.upgrade() {
                    if let Some(reader) = reader.borrow().as_ref() {
                        reader.attempt(admitted);
                    }
                }
            })
            .with_navigation_event_handler(move |event| {
                match commit_navigation.observe_navigation(&event) {
                    Some(NavigationTransition::Committed(epoch)) => {
                        if let Some(reader) = commit_reader.upgrade() {
                            if let Some(reader) = reader.borrow().as_ref() {
                                reader.commit(epoch.presentation_id().into_raw(), &event.url);
                            }
                        }
                    }
                    Some(NavigationTransition::Finished(_)) => {
                        finish_state.borrow_mut().finished += 1;
                    }
                    _ => {}
                }
            })
            .build_as_child(&host)
            .map_err(|e| format!("cannot build probe WebView: {e}"))?;
        // A window never shown is a hidden page; let it run at full speed
        // so the page's watch loop samples as tightly as it can.
        let page = super::native_webview(&view);
        // SAFETY: main-thread WebKit access on the probe's own view.
        unsafe {
            page.configuration()
                .preferences()
                .setInactiveSchedulingPolicy(objc2_web_kit::WKInactiveSchedulingPolicy::None);
        }
        let committed = navigation.clone();
        let settled = navigation.clone();
        let tab_cell = Rc::new(Cell::new(tab));
        *reader.borrow_mut() = BrowseReader::new(
            &super::native_webview(&view),
            tab_cell.clone(),
            enabled,
            Rc::new(move || {
                committed
                    .current_committed()
                    .map(|epoch| epoch.presentation_id().into_raw())
            }),
            Rc::new(move || settled.current() == settled.current_committed()),
        );
        if reader.borrow().is_none() {
            return Err("this WebKit cannot host the reader".to_owned());
        }
        Ok(Self {
            _window: window,
            _host: host,
            view,
            reader,
            navigation,
            state,
            sensitive,
            tab,
            tab_cell,
        })
    }

    /// Goes back, or forward, as the person's history buttons do.
    fn history(&self, forward: bool) {
        let page = super::native_webview(&self.view);
        // SAFETY: main-thread WebKit navigation on the probe's own view.
        unsafe {
            if forward {
                let _ = page.goForward();
            } else {
                let _ = page.goBack();
            }
        }
    }

    fn interaction_state(&self) -> Option<Retained<objc2_foundation::NSData>> {
        let page = super::native_webview(&self.view);
        // SAFETY: main-thread property read; objc2 retains the result.
        unsafe { page.interactionState() }?
            .downcast::<objc2_foundation::NSData>()
            .ok()
    }

    /// Restores a saved session, as a discarded tab is brought back.
    fn restore(&self, state: &objc2_foundation::NSData) {
        let page = super::native_webview(&self.view);
        // SAFETY: the exact data came from interactionState on a view of
        // the same kind; the setter starts the navigation itself.
        unsafe { page.setInteractionState(Some(state)) };
    }

    fn epoch(&self) -> Option<u64> {
        self.navigation
            .current_committed()
            .map(|epoch| epoch.presentation_id().into_raw())
    }

    fn load(&self, url: &str) -> Result<u64, String> {
        let before = self.state.borrow().finished;
        let _ = self.navigation.begin(url);
        self.view
            .load_url(url)
            .map_err(|e| format!("load {url}: {e}"))?;
        pump(
            || self.state.borrow().finished > before,
            &format!("loading {url}"),
        )?;
        self.epoch().ok_or_else(|| "no committed epoch".to_owned())
    }

    fn wait_readable(&self) -> Result<(), String> {
        pump(
            || crate::page_reader::readable(self.tab_cell.get()),
            "for the boot pull",
        )
        .map_err(|error| {
            format!(
                "{error} (published {:?}, committed {:?}, current {:?})",
                crate::page_reader::committed_epoch(self.tab_cell.get()),
                self.epoch(),
                self.navigation
                    .current()
                    .map(|epoch| epoch.presentation_id().into_raw())
            )
        })
    }

    fn read(&self, epoch: u64, url: &str) -> (Result<PageReadout, PageReadFailure>, Duration) {
        let slot: Arc<Mutex<Option<Result<PageReadout, PageReadFailure>>>> = Arc::default();
        let answer = slot.clone();
        let started = Instant::now();
        if let Some(reader) = self.reader.borrow().as_ref() {
            reader.read(
                epoch,
                url.to_owned(),
                Box::new(move |result| {
                    *answer.lock().unwrap_or_else(|p| p.into_inner()) = Some(result);
                }),
            );
        }
        // The host's timers re-run the queue in the app; here the wait does.
        let done = || {
            if let Some(reader) = self.reader.borrow().as_ref() {
                reader.pump();
            }
            slot.lock().unwrap_or_else(|p| p.into_inner()).is_some()
        };
        if pump(done, "for a read").is_err() {
            return (Err(PageReadFailure::Unavailable), started.elapsed());
        }
        let elapsed = started.elapsed();
        let result = slot
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take()
            .unwrap_or(Err(PageReadFailure::Unavailable));
        (result, elapsed)
    }

    fn recipe(&self, epoch: u64, recipe: PageRecipe) -> (Option<String>, Duration) {
        let slot: Arc<Mutex<Option<Option<String>>>> = Arc::default();
        let answer = slot.clone();
        let started = Instant::now();
        if let Some(reader) = self.reader.borrow().as_ref() {
            reader.recipe(
                epoch,
                recipe,
                Box::new(move |result| {
                    *answer.lock().unwrap_or_else(|p| p.into_inner()) = Some(result);
                }),
            );
        }
        // The host's timers re-run the queue in the app; here the wait does.
        let done = || {
            if let Some(reader) = self.reader.borrow().as_ref() {
                reader.pump();
            }
            slot.lock().unwrap_or_else(|p| p.into_inner()).is_some()
        };
        if pump(done, "for a recipe").is_err() {
            return (None, started.elapsed());
        }
        let elapsed = started.elapsed();
        let result = slot
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take()
            .flatten();
        (result, elapsed)
    }

    /// Runs one step and waits for it, driving the reader's queue and its
    /// lease the way the host's timers do.
    fn act(
        &self,
        epoch: u64,
        url: &str,
        proposal: zephium_agentic::SemanticActionProposal,
    ) -> Result<(), crate::page_reader::PageActFailure> {
        let slot: Arc<Mutex<Option<Result<(), crate::page_reader::PageActFailure>>>> =
            Arc::default();
        let answer = slot.clone();
        if let Some(reader) = self.reader.borrow().as_ref() {
            reader.act(
                epoch,
                url.to_owned(),
                proposal,
                Box::new(move |result| {
                    *answer.lock().unwrap_or_else(|p| p.into_inner()) = Some(result);
                }),
            );
        }
        let done = || {
            if let Some(reader) = self.reader.borrow().as_ref() {
                reader.pump();
            }
            slot.lock().unwrap_or_else(|p| p.into_inner()).is_some()
        };
        pump(done, "for a step").map_err(|_| crate::page_reader::PageActFailure::Failed)?;
        let result = slot
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take()
            .unwrap_or(Err(crate::page_reader::PageActFailure::Failed));
        result
    }

    /// Presses a key into the page as the person's keyboard would.
    fn press_key(&self) {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let page = super::native_webview(&self.view);
        let characters = objc2_foundation::NSString::from_str("a");
        let event = {
            objc2_app_kit::NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
                objc2_app_kit::NSEventType::KeyDown,
                NSPoint::new(10.0, 10.0),
                objc2_app_kit::NSEventModifierFlags::empty(),
                0.0,
                self._window.windowNumber(),
                None,
                &characters,
                &characters,
                false,
                0,
            )
        };
        let _ = mtm;
        if let Some(event) = event {
            page.keyDown(&event);
        }
    }

    /// The view's pixels, through WebKit's snapshot API (no script).
    fn snapshot(&self) -> Option<Retained<objc2_app_kit::NSBitmapImageRep>> {
        let mtm = MainThreadMarker::new()?;
        let page = super::native_webview(&self.view);
        // SAFETY: main-thread WebKit object creation and configuration.
        let configuration = unsafe { objc2_web_kit::WKSnapshotConfiguration::new(mtm) };
        // SAFETY: as above; a live configuration on the main thread.
        unsafe { configuration.setAfterScreenUpdates(true) };
        let slot: Rc<RefCell<Option<Option<Retained<objc2_app_kit::NSBitmapImageRep>>>>> =
            Rc::default();
        let answer = slot.clone();
        let callback = block2::RcBlock::new(
            move |image: *mut objc2_app_kit::NSImage, _error: *mut objc2_foundation::NSError| {
                let rep = (!image.is_null())
                    .then(|| {
                        // SAFETY: WebKit keeps a non-null image valid for the
                        // callback; a null proposed rectangle is allowed.
                        let cg = unsafe {
                            (*image).CGImageForProposedRect_context_hints(
                                std::ptr::null_mut(),
                                None,
                                None,
                            )
                        }?;
                        Some(objc2_app_kit::NSBitmapImageRep::initWithCGImage(
                            <objc2_app_kit::NSBitmapImageRep as objc2::AnyThread>::alloc(),
                            &cg,
                        ))
                    })
                    .flatten();
                *answer.borrow_mut() = Some(rep);
            },
        );
        // SAFETY: WebKit copies the block and replies once on the main thread.
        unsafe {
            page.takeSnapshotWithConfiguration_completionHandler(Some(&configuration), &callback)
        };
        pump(|| slot.borrow().is_some(), "for a snapshot").ok()?;
        let rep = slot.borrow_mut().take().flatten();
        rep
    }

    fn footprint(&self) -> Option<u64> {
        super::page_footprint(&self.view)
    }

    fn cpu_ns(&self) -> Option<u64> {
        process_cpu_ns(&self.view)
    }

    fn user_scripts(&self) -> usize {
        let view = super::native_webview(&self.view);
        // SAFETY: main-thread property reads on a live view; objc2 retains
        // the configuration, controller and the script array.
        unsafe {
            view.configuration()
                .userContentController()
                .userScripts()
                .count()
        }
    }
}

thread_local! {
    static ENABLED: Cell<bool> = const { Cell::new(true) };
}

// As in the app, an address with no site (about:blank) never gets the script.
fn enabled_and_admitted(sensitive: &Cell<bool>, target: &str) -> bool {
    ENABLED.with(Cell::get)
        && target.starts_with("http")
        && !(sensitive.get() && target.contains("/sensitive"))
}

// libc marks the timebase deprecated in favour of another crate; this
// probe needs only the one call.
#[allow(deprecated)]
fn process_cpu_ns(view: &wry::WebView) -> Option<u64> {
    let page = super::native_webview(view);
    // SAFETY: main-thread WebKit access; the SPI is used only when present.
    let supported: bool =
        unsafe { objc2::msg_send![&*page, respondsToSelector: objc2::sel!(_webProcessIdentifier)] };
    if !supported {
        return None;
    }
    // SAFETY: probed above; returns the WebContent process identifier.
    let pid: libc::pid_t = unsafe { objc2::msg_send![&*page, _webProcessIdentifier] };
    if pid <= 0 {
        return None;
    }
    // SAFETY: a correctly sized, zeroed rusage_info_v4 for this flavor.
    let mut info: libc::rusage_info_v4 = unsafe { std::mem::zeroed() };
    // SAFETY: `info` is a live, correctly sized buffer for this flavor.
    let status = unsafe {
        libc::proc_pid_rusage(
            pid,
            libc::RUSAGE_INFO_V4,
            (&mut info as *mut libc::rusage_info_v4).cast(),
        )
    };
    if status != 0 {
        return None;
    }
    let mut timebase = libc::mach_timebase_info { numer: 0, denom: 0 };
    // SAFETY: a live out-parameter for the kernel's fixed timebase.
    if unsafe { libc::mach_timebase_info(&mut timebase) } != 0 || timebase.denom == 0 {
        return None;
    }
    let ticks = info.ri_user_time.saturating_add(info.ri_system_time);
    Some(ticks.saturating_mul(u64::from(timebase.numer)) / u64::from(timebase.denom))
}

fn rgb(rep: &objc2_app_kit::NSBitmapImageRep, x: isize, y: isize) -> Option<(f64, f64, f64)> {
    let color = rep.colorAtX_y(x, y)?;
    let color = color.colorUsingColorSpace(&objc2_app_kit::NSColorSpace::sRGBColorSpace())?;
    Some((
        color.redComponent(),
        color.greenComponent(),
        color.blueComponent(),
    ))
}

/// Samples both frames on a grid: how many points turned warm.
fn compare(
    before: &objc2_app_kit::NSBitmapImageRep,
    after: &objc2_app_kit::NSBitmapImageRep,
) -> usize {
    let (width, height) = (
        before.pixelsWide().min(after.pixelsWide()),
        before.pixelsHigh().min(after.pixelsHigh()),
    );
    let mut tinted = 0;
    for y in (0..height).step_by(3) {
        for x in (0..width).step_by(3) {
            let (Some(a), Some(b)) = (rgb(before, x, y), rgb(after, x, y)) else {
                continue;
            };
            let delta = (a.0 - b.0).abs() + (a.1 - b.1).abs() + (a.2 - b.2).abs();
            if delta < 0.04 {
                continue;
            }
            // Under the quote the mark only adds warmth: blue drops most.
            if b.2 < a.2 && (a.2 - b.2) > (a.0 - b.0).abs() {
                tinted += 1;
            }
        }
    }
    tinted
}

fn epoch_of_old(epoch: u64) -> u64 {
    epoch.saturating_sub(1)
}

fn next_url(fixture: &Fixture) -> String {
    fixture.url("/next")
}

fn check(condition: bool, what: &str) -> Result<(), String> {
    if condition {
        println!("  ok    {what}");
        Ok(())
    } else {
        Err(format!("FAILED {what}"))
    }
}

fn fixtures(mtm: MainThreadMarker, fixture: &Fixture) -> Result<(), String> {
    let probe = Probe::new(mtm, true)?;
    let baseline_scripts = probe.user_scripts();

    println!("fixture: read under page CSP and Trusted Types");
    for path in ["/csp", "/tt", "/strict"] {
        let url = fixture.url(path);
        let epoch = probe.load(&url)?;
        probe.wait_readable()?;
        let (read, took) = probe.read(epoch, &url);
        let read = read.map_err(|f| format!("{path} read failed: {f:?}"))?;
        check(
            read.passages.iter().any(|p| {
                p.text == "The northern rivers rose two metres overnight after the storm."
            }),
            &format!(
                "{path}: instantiated in the private world and read ({} passages, {} ms)",
                read.passages.len(),
                took.as_millis()
            ),
        )?;
    }

    println!("fixture: read shape (public only, verbatim, never secrets)");
    let url = fixture.url("/article");
    let epoch = probe.load(&url)?;
    probe.wait_readable()?;

    let (read, first) = probe.read(epoch, &url);
    let read = read.map_err(|f| format!("article read failed: {f:?}"))?;
    let (_, again) = probe.read(epoch, &url);
    let find = |text: &str| read.passages.iter().find(|p| p.text == text);
    check(
        read.title.as_deref() == Some("Rivers of the North"),
        "title is the native title",
    )?;
    check(
        find("Rivers of the North").is_some_and(|p| p.role == PageRole::Heading(1) && p.verbatim),
        "h1 is a verbatim level-1 heading",
    )?;
    check(
        read.passages.iter().all(|p| {
            !p.text.contains("hunter2") && p.value.as_deref().is_none_or(|v| !v.contains("hunter2"))
        }),
        "the password's value is never read",
    )?;
    check(
        read.passages.iter().all(|p| {
            let leaked = |text: &str| text.contains("4111") || text.contains("928374");
            !leaked(&p.text) && p.value.as_deref().is_none_or(|v| !leaked(v))
        }),
        "card and one-time-code values are never read",
    )?;
    check(
        read.passages
            .iter()
            .any(|p| matches!(p.role, PageRole::Link { same_site: false })),
        "a link off the site is marked as leaving it",
    )?;
    check(
        read.passages.iter().any(|p| p.region == 2),
        "page chrome is marked as region 2",
    )?;
    println!(
        "  first ask {} ms, next read {} ms",
        first.as_millis(),
        again.as_millis()
    );

    println!("fixture: selection recipe");
    let (selection, took) = probe.recipe(epoch, PageRecipe::selection());
    check(
        selection.as_deref() == Some("eastern floodgates at dawn"),
        &format!("selected text read in {} ms", took.as_millis()),
    )?;

    println!("fixture: mark recipe (no find bar)");
    let quote = "The northern rivers rose two metres overnight after the storm.";
    let before = probe.snapshot().ok_or("snapshot before the mark")?;
    let (marked, took) = probe.recipe(epoch, PageRecipe::mark(quote).ok_or("mark")?);
    check(
        marked.as_deref().and_then(mark_outcome) == Some(MarkOutcome::Marked),
        &format!(
            "verbatim quote scrolled to and marked in {} ms",
            took.as_millis()
        ),
    )?;
    settle(Duration::from_millis(150));
    let after = probe.snapshot().ok_or("snapshot after the mark")?;
    let tinted = compare(&before, &after);
    check(
        tinted > 20,
        &format!("the mark paints a warm highlight ({tinted} samples)"),
    )?;
    let (missing, _) = probe.recipe(
        epoch,
        PageRecipe::mark("Not on this page at all").ok_or("mark")?,
    );
    check(
        missing.as_deref().and_then(mark_outcome) == Some(MarkOutcome::Missing),
        "a quote not on the page is reported missing",
    )?;
    let (split, _) = probe.recipe(
        epoch,
        PageRecipe::mark("Engineers opened the eastern floodgates at dawn").ok_or("mark")?,
    );
    check(
        split.as_deref().and_then(mark_outcome) == Some(MarkOutcome::Marked),
        "a quote across inline elements is found",
    )?;

    println!("fixture: outline recipe");
    let (outline, took) = probe.recipe(epoch, PageRecipe::outline());
    let outline = outline
        .as_deref()
        .and_then(parse_outline)
        .ok_or("outline did not parse")?;
    let selectors: Vec<&str> = outline.nodes.iter().map(|n| n.selector.as_str()).collect();
    println!("  outline in {} ms: {selectors:?}", took.as_millis());
    check(
        selectors.contains(&"header.site-head"),
        "header with its stable class",
    )?;
    check(
        selectors
            .iter()
            .all(|s| !s.contains("css-1x2y3z") && !s.contains("kCsWcF")),
        "hashed and generated classes are skipped",
    )?;
    check(selectors.contains(&"div.card"), "repeated cards are found")?;
    check(
        outline.background == Some(crate::page_reader::Rgb(0xfb, 0xfa, 0xf7)) && !outline.dark,
        "page background and lightness",
    )?;
    check(
        outline.fonts.first().map(String::as_str) == Some("Georgia"),
        "page font",
    )?;

    println!("fixture: activation and isolation (page watched throughout)");
    let report_url = format!("{url}#report");
    probe
        .view
        .load_url(&report_url)
        .map_err(|e| format!("report: {e}"))?;
    pump(
        || probe.state.borrow().title.starts_with("report "),
        "for the page's report",
    )?;
    let title = probe.state.borrow().title.clone();
    println!("  page report: {title}");
    check(
        title.contains("active=false"),
        "no user activation from automation",
    )?;
    check(
        title.contains("been=false"),
        "the page never became activated",
    )?;
    check(title.contains("opened=0"), "window.open stayed blocked")?;
    check(probe.state.borrow().popups == 0, "no popup reached native")?;
    check(
        title.contains("leaks=none"),
        "the page saw no private global or handler",
    )?;
    check(title.contains("layout=same"), "marking shifted no layout")?;
    check(
        title.contains("marked=true"),
        "the mark was on the page while it watched",
    )?;
    check(
        !title.contains("zephium"),
        "nothing the page can see names the panel",
    )?;
    check(
        title
            .split_whitespace()
            .find_map(|part| part.strip_prefix("samples="))
            .and_then(|n| n.parse::<u64>().ok())
            .is_some_and(|n| n > 200),
        "the page polled continuously",
    )?;

    println!("fixture: mark cleanup");
    let engineers = "Engineers opened the eastern floodgates at dawn";
    let (_, _) = probe.recipe(epoch, PageRecipe::mark(quote).ok_or("mark")?);
    let (_, _) = probe.recipe(epoch, PageRecipe::mark(engineers).ok_or("mark")?);
    let report = |label: &str| -> Result<String, String> {
        probe
            .view
            .load_url(&format!("{url}#report{label}"))
            .map_err(|e| format!("report: {e}"))?;
        let prefix = format!("report {label} ");
        pump(
            || probe.state.borrow().title.starts_with(&prefix),
            "for a page report",
        )?;
        Ok(probe.state.borrow().title.clone())
    };
    let replaced = report("replaced")?;
    check(
        replaced.contains("sheets=1"),
        "a new mark replaces the old one: one sheet",
    )?;
    settle(Duration::from_millis(5_500));
    let clean = probe.snapshot().ok_or("clean snapshot")?;
    let faded = report("faded")?;
    check(
        faded.contains("sheets=0"),
        "after the fade the page has no sheet of ours",
    )?;
    let (_, _) = probe.recipe(epoch, PageRecipe::mark(engineers).ok_or("mark")?);
    settle(Duration::from_millis(150));
    let marked = probe.snapshot().ok_or("marked snapshot")?;
    check(compare(&clean, &marked) > 20, "a mark paints")?;
    settle(Duration::from_millis(5_500));
    let gone = probe.snapshot().ok_or("faded snapshot")?;
    check(
        compare(&clean, &gone) == 0,
        "and after its fade nothing of it is painted",
    )?;

    println!("fixture: a program that will not start");
    let epoch = probe.load(&url)?;
    probe.wait_readable()?;
    super::browse_reader::probe::break_starts(1);
    let started = Instant::now();
    let (first, _) = probe.read(epoch, &url);
    check(
        first == Err(PageReadFailure::Unavailable) && started.elapsed() < Duration::from_secs(3),
        "the failed start ends that ask as unavailable at once",
    )?;
    probe.wait_readable()?;
    check(
        probe.read(epoch, &url).0.is_ok(),
        "the next ask retries once and reads",
    )?;
    let epoch = probe.load(&url)?;
    probe.wait_readable()?;
    super::browse_reader::probe::break_starts(2);
    let (first, _) = probe.read(epoch, &url);
    settle(Duration::from_millis(200));
    let started = Instant::now();
    let (second, _) = probe.read(epoch, &url);
    let (third, _) = probe.read(epoch, &url);
    check(
        first == Err(PageReadFailure::Unavailable)
            && second == Err(PageReadFailure::Unavailable)
            && third == Err(PageReadFailure::Unavailable)
            && started.elapsed() < Duration::from_secs(3),
        "after the one retry fails the page reads as unavailable, with no loop",
    )?;
    check(
        crate::page_reader::readable(probe.tab),
        "and it never reads as loading",
    )?;
    super::browse_reader::probe::break_starts(0);
    let epoch = probe.load(&url)?;
    probe.wait_readable()?;

    println!("fixture: idle stop and restart");
    if let Some(reader) = probe.reader.borrow().as_ref() {
        reader.stop_now();
    }
    probe.wait_readable()?;
    let (restarted, took) = probe.read(epoch, &url);
    if let Err(failure) = &restarted {
        println!("  restart read failed: {failure:?}");
    }
    check(
        restarted.is_ok_and(|r| r.passages.iter().any(|p| p.text == quote)),
        &format!(
            "after an idle stop the same document reads again without reinstalling ({} ms)",
            took.as_millis()
        ),
    )?;

    println!("fixture: epoch binding");
    let next = fixture.url("/next");
    let old = epoch;
    let new = probe.load(&next)?;
    probe.wait_readable()?;
    check(new != old, "a new document has a new epoch")?;
    let (stale, _) = probe.read(old, &url);
    check(
        stale == Err(PageReadFailure::PageChanged),
        "a read for the old document is discarded",
    )?;
    let (fresh, _) = probe.read(new, &next);
    check(
        fresh
            .is_ok_and(|r| r.epoch == new && r.passages.iter().any(|p| p.text == "The next story")),
        "the new document reads under its own epoch",
    )?;
    let reloaded = probe.load(&next)?;
    probe.wait_readable()?;
    check(
        reloaded != new,
        "a reload of the same address is a new epoch",
    )?;
    check(
        probe.user_scripts() == baseline_scripts + 1,
        "one reader script per view, however many navigations",
    )?;

    println!("fixture: steps on the person's own tab");
    let inputs: Arc<Mutex<Vec<ItemId>>> = Arc::default();
    let seen = inputs.clone();
    crate::host::page_reader::set_policy_for_probe(Some(crate::page_reader::PageReaderPolicy {
        sensitive: Arc::new(|_| false),
        human_input: Arc::new(move |tab| seen.lock().unwrap_or_else(|p| p.into_inner()).push(tab)),
    }));
    let form = fixture.url("/form");
    let epoch = probe.load(&form)?;
    probe.wait_readable()?;
    let (read, _) = probe.read(epoch, &form);
    let read = read.map_err(|f| format!("form read: {f:?}"))?;
    let stored = crate::page_reader::latest_read(probe.tab).ok_or("stored read")?;
    let passage = |text: &str| {
        read.passages
            .iter()
            .find(|p| p.text == text)
            .and_then(|p| stored.reference(p.id))
            .ok_or_else(|| {
                format!(
                    "no passage {text:?} in {:?}",
                    read.passages.iter().map(|p| &p.text).collect::<Vec<_>>()
                )
            })
    };
    let proposal = |intent, effect, verification| {
        zephium_agentic::SemanticActionProposal::try_new(
            intent,
            effect,
            zephium_agentic::SemanticWaitCondition::Immediate,
            verification,
            zephium_agentic::SemanticSettleBudget::try_new(2_000).expect("settle"),
        )
        .expect("proposal")
    };
    let typed = zephium_agentic::SemanticActionText::try_new("Ada Lovelace".to_owned())
        .map_err(|_| "text")?;
    let started = Instant::now();
    let filled = probe.act(
        epoch,
        &form,
        proposal(
            zephium_agentic::SemanticActionIntent::Fill {
                target: passage("Name")?,
                value: typed,
            },
            zephium_agentic::SemanticEffectClass::LocalWrite,
            zephium_agentic::SemanticVerification::TargetValueMatchesInput,
        ),
    );
    check(
        filled.is_ok(),
        &format!(
            "typing into an ordinary field works ({filled:?}, {} ms)",
            started.elapsed().as_millis()
        ),
    )?;
    let (after, _) = probe.read(epoch, &form);
    check(
        after.is_ok_and(|r| r.passages.iter().any(|p| p.text == "state:Ada Lovelace")),
        "a controlled field's own state took the typed text",
    )?;
    let stored = crate::page_reader::latest_read(probe.tab).ok_or("stored read")?;
    let password = stored
        .observation()
        .frames()
        .first()
        .and_then(|frame| {
            frame
                .nodes()
                .iter()
                .find(|node| node.role() == zephium_agentic::SemanticRole::Password)
                .map(|node| node.reference())
        })
        .ok_or("password node")?;
    let secret =
        zephium_agentic::SemanticActionText::try_new("hunter2".to_owned()).map_err(|_| "text")?;
    let refused = probe.act(
        epoch,
        &form,
        proposal(
            zephium_agentic::SemanticActionIntent::Fill {
                target: password,
                value: secret,
            },
            zephium_agentic::SemanticEffectClass::LocalWrite,
            zephium_agentic::SemanticVerification::TargetValueMatchesInput,
        ),
    );
    check(
        matches!(
            refused,
            Err(crate::page_reader::PageActFailure::Refused
                | crate::page_reader::PageActFailure::Unsuitable)
        ),
        &format!("a password field is never typed into ({refused:?})"),
    )?;
    let ordered = probe.act(
        epoch,
        &form,
        proposal(
            zephium_agentic::SemanticActionIntent::Click {
                target: passage("Place order")?,
            },
            zephium_agentic::SemanticEffectClass::Purchase,
            zephium_agentic::SemanticVerification::PageChanged,
        ),
    );
    let (after, _) = probe.read(epoch, &form);
    check(
        ordered.is_ok() && after.is_ok_and(|r| r.passages.iter().any(|p| p.text == "ordered")),
        "a confirmed committing click runs once, as the page's own click",
    )?;
    let stale = probe.act(
        epoch_of_old(epoch),
        &form,
        proposal(
            zephium_agentic::SemanticActionIntent::Click {
                target: passage("Next story")?,
            },
            zephium_agentic::SemanticEffectClass::Read,
            zephium_agentic::SemanticVerification::PageChanged,
        ),
    );
    check(
        stale == Err(crate::page_reader::PageActFailure::PageChanged),
        "a step on another epoch is refused",
    )?;
    inputs.lock().unwrap_or_else(|p| p.into_inner()).clear();
    probe.press_key();
    let reported = || {
        if let Some(reader) = probe.reader.borrow().as_ref() {
            reader.poll();
        }
        inputs
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .contains(&probe.tab)
    };
    pump(reported, "for the person's key to be noticed")?;
    check(
        true,
        "the person's own input on a held tab is reported, so steps pause",
    )?;
    // A step admitted before a navigation never runs after it.
    let (read, _) = probe.read(epoch, &form);
    let read = read.map_err(|f| format!("form read: {f:?}"))?;
    let stored = crate::page_reader::latest_read(probe.tab).ok_or("stored read")?;
    let target = |text: &str| {
        read.passages
            .iter()
            .find(|p| p.text == text)
            .and_then(|p| stored.reference(p.id))
            .ok_or("passage")
    };
    let slow = Arc::new(Mutex::new(None));
    let second = Arc::new(Mutex::new(None));
    if let Some(reader) = probe.reader.borrow().as_ref() {
        let slot = slow.clone();
        reader.act(
            epoch,
            form.clone(),
            proposal(
                zephium_agentic::SemanticActionIntent::Click {
                    target: target("Check stock")?,
                },
                zephium_agentic::SemanticEffectClass::LocalWrite,
                zephium_agentic::SemanticVerification::PageChanged,
            ),
            Box::new(move |result| *slot.lock().unwrap_or_else(|p| p.into_inner()) = Some(result)),
        );
        let slot = second.clone();
        reader.act(
            epoch,
            form.clone(),
            proposal(
                zephium_agentic::SemanticActionIntent::Click {
                    target: target("Place order")?,
                },
                zephium_agentic::SemanticEffectClass::Purchase,
                zephium_agentic::SemanticVerification::PageChanged,
            ),
            Box::new(move |result| *slot.lock().unwrap_or_else(|p| p.into_inner()) = Some(result)),
        );
    }
    let _ = probe.navigation.begin(&next_url(fixture));
    probe
        .view
        .load_url(&next_url(fixture))
        .map_err(|e| e.to_string())?;
    pump(
        || {
            if let Some(reader) = probe.reader.borrow().as_ref() {
                reader.pump();
            }
            second.lock().unwrap_or_else(|p| p.into_inner()).is_some()
        },
        "for the queued step",
    )?;
    let second = second.lock().unwrap_or_else(|p| p.into_inner()).take();
    check(
        second == Some(Err(crate::page_reader::PageActFailure::PageChanged)),
        &format!("a navigation mid-plan ends the next step as a changed page ({second:?})"),
    )?;
    crate::host::page_reader::set_policy_for_probe(None);
    let _ = slow;

    println!("fixture: back-forward cache");
    let a = fixture.url("/cache-a");
    let b = fixture.url("/cache-b");
    let epoch_a = probe.load(&a)?;
    probe.wait_readable()?;
    check(
        probe
            .read(epoch_a, &a)
            .0
            .is_ok_and(|r| r.passages.iter().any(|p| p.text.contains("Alpha"))),
        "page A reads",
    )?;
    let epoch_b = probe.load(&b)?;
    probe.wait_readable()?;
    let before = probe.state.borrow().finished;
    probe.history(false);
    pump(
        || probe.state.borrow().title == "A restored",
        "for page A to come back from the cache",
    )?;
    pump(
        || probe.state.borrow().finished > before,
        "for the restore to finish",
    )?;
    let restored = probe.epoch().ok_or("restored epoch")?;
    check(
        restored != epoch_a && restored != epoch_b,
        "a restored page has its own epoch",
    )?;
    let (stale, _) = probe.read(epoch_b, &b);
    check(
        stale == Err(PageReadFailure::PageChanged),
        "a read for the page left is discarded",
    )?;
    pump(
        || crate::page_reader::readable(probe.tab),
        "for the restored page to pull again",
    )?;
    let (back, _) = probe.read(restored, &a);
    check(
        back.as_ref().is_ok_and(|r| {
            r.epoch == restored
                && r.passages.iter().any(|p| p.text.contains("Alpha"))
                && !r.passages.iter().any(|p| p.text.contains("Beta"))
        }),
        &format!("the restored page reads itself under its new epoch ({back:?})"),
    )?;
    let before = probe.state.borrow().finished;
    probe.history(true);
    pump(
        || probe.state.borrow().title == "B restored",
        "for page B to come back",
    )?;
    pump(
        || probe.state.borrow().finished > before,
        "for the forward restore to finish",
    )?;
    let forward = probe.epoch().ok_or("forward epoch")?;
    pump(
        || crate::page_reader::readable(probe.tab),
        "for page B to pull again",
    )?;
    check(
        probe.read(forward, &b).0.is_ok_and(|r| {
            r.passages.iter().any(|p| p.text.contains("Beta"))
                && !r.passages.iter().any(|p| p.text.contains("Alpha"))
        }),
        "a page restored before it was ever read reads itself",
    )?;

    println!("fixture: discarded tab restored");
    let epoch = probe.load(&url)?;
    probe.wait_readable()?;
    let _ = probe.read(epoch, &url);
    let saved = probe.interaction_state().ok_or("interaction state")?;
    let restored = Probe::new(mtm, true)?;
    let before = restored.state.borrow().finished;
    let _ = restored.navigation.begin(&url);
    restored.restore(&saved);
    pump(
        || restored.state.borrow().finished > before,
        "for the restored tab to load",
    )?;
    restored.wait_readable()?;
    let epoch = restored.epoch().ok_or("restored tab epoch")?;
    check(
        restored
            .read(epoch, &url)
            .0
            .is_ok_and(|r| r.passages.iter().any(|p| p.text == quote)),
        "a restored tab reads under its new epoch",
    )?;
    drop(restored);

    println!("fixture: warm spare adopted");
    let spare = Probe::new(mtm, true)?;
    let before = spare.state.borrow().finished;
    let _ = spare.navigation.begin("about:blank");
    spare
        .view
        .load_url("about:blank")
        .map_err(|e| e.to_string())?;
    pump(
        || spare.state.borrow().finished > before,
        "for the spare's blank page",
    )?;
    check(
        !crate::page_reader::readable(spare.tab),
        "a blank spare has no script",
    )?;
    let adopted = ItemId::generate();
    spare.tab_cell.set(adopted);
    let epoch = spare.load(&url)?;
    pump(
        || crate::page_reader::readable(adopted),
        "for the adopted tab to pull",
    )?;
    check(
        crate::page_reader::committed_epoch(adopted) == Some(epoch),
        "the adopted tab publishes its own epoch",
    )?;
    check(
        spare
            .read(epoch, &url)
            .0
            .is_ok_and(|r| r.passages.iter().any(|p| p.text == quote)),
        "and reads",
    )?;
    drop(spare);

    println!("fixture: sensitive site and AI off");
    probe.sensitive.set(true);
    let sensitive = fixture.url("/sensitive");
    let epoch = probe.load(&sensitive)?;
    settle(Duration::from_millis(300));
    check(
        !crate::page_reader::readable(probe.tab),
        "a sensitive site gets no script",
    )?;
    let (refused, _) = probe.read(epoch, &sensitive);
    check(
        refused == Err(PageReadFailure::NotReady),
        "and cannot be read",
    )?;
    probe.sensitive.set(false);
    let epoch = probe.load(&url)?;
    probe.wait_readable()?;
    let (_, _) = probe.read(epoch, &url);
    ENABLED.with(|enabled| enabled.set(false));
    if let Some(reader) = probe.reader.borrow().as_ref() {
        reader.set_enabled(false);
    }
    check(
        probe.user_scripts() == baseline_scripts,
        "AI off removes every reader script",
    )?;
    let (off, _) = probe.read(epoch, &url);
    check(off.is_err(), "AI off: nothing reads")?;
    let epoch = probe.load(&url)?;
    settle(Duration::from_millis(300));
    check(
        !crate::page_reader::readable(probe.tab),
        "AI off: new documents get no script",
    )?;
    let _ = epoch;
    ENABLED.with(|enabled| enabled.set(true));
    if let Some(reader) = probe.reader.borrow().as_ref() {
        reader.set_enabled(true);
    }
    let epoch = probe.load(&url)?;
    probe.wait_readable()?;
    check(
        probe.read(epoch, &url).0.is_ok(),
        "AI on again: the next document reads",
    )?;

    println!("fixture: tab close");
    drop(probe.reader.borrow_mut().take());
    check(
        probe.user_scripts() == baseline_scripts,
        "closing removes every reader script",
    )?;
    check(
        !crate::page_reader::readable(probe.tab),
        "and forgets the tab",
    )?;
    Ok(())
}

fn median(values: &mut [f64]) -> f64 {
    values.sort_by(|a, b| a.total_cmp(b));
    values.get(values.len() / 2).copied().unwrap_or(0.0)
}

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f64>() / values.len() as f64
    }
}

impl Probe {
    /// Collects garbage in the page's process (measurement only, through
    /// WebKit's testing SPI when present) and reports its footprint in KB.
    fn settled_footprint(&self) -> u64 {
        let page = super::native_webview(&self.view);
        // SAFETY: main-thread WebKit access; the SPI is probed first.
        unsafe {
            let configuration = page.configuration();
            let pool: Retained<objc2::runtime::NSObject> =
                objc2::msg_send![&*configuration, processPool];
            let supported: bool = objc2::msg_send![
                &*pool,
                respondsToSelector: objc2::sel!(_garbageCollectJavaScriptObjectsForTesting)
            ];
            if supported {
                let _: () = objc2::msg_send![&*pool, _garbageCollectJavaScriptObjectsForTesting];
            }
        }
        settle(Duration::from_millis(700));
        self.footprint().unwrap_or(0) / 1024
    }
}

fn ask_costs(probe: &Probe, url: &str, label: &str) -> Result<(), String> {
    let epoch = probe.load(url)?;
    probe.wait_readable()?;
    settle(Duration::from_millis(300));
    let base = probe.settled_footprint();
    let cpu = probe.cpu_ns();
    let (first, first_took) = probe.read(epoch, url);
    let first_cpu = probe.cpu_ns().zip(cpu).map_or(0.0, |(after, before)| {
        after.saturating_sub(before) as f64 / 1e6
    });
    let first = first.map_err(|f| format!("{label} first read: {f:?}"))?;
    let after_first = probe.settled_footprint();
    let mut reads = Vec::new();
    let mut cpus = Vec::new();
    for _ in 0..10 {
        let cpu = probe.cpu_ns();
        let (read, took) = probe.read(epoch, url);
        read.map_err(|f| format!("{label} read: {f:?}"))?;
        reads.push(took.as_secs_f64() * 1e3);
        cpus.push(probe.cpu_ns().zip(cpu).map_or(0.0, |(after, before)| {
            after.saturating_sub(before) as f64 / 1e6
        }));
    }
    let after_reads = probe.settled_footprint();
    let (_, selection) = probe.recipe(epoch, PageRecipe::selection());
    let (_, outline) = probe.recipe(epoch, PageRecipe::outline());
    let (_, mark) = probe.recipe(
        epoch,
        PageRecipe::mark("Paragraph 7 of section 150").ok_or("mark")?,
    );
    println!(
        "  {label}: first ask {} ms (page CPU {:.1} ms, {} passages); later reads median {:.1} ms (page CPU median {:.1} ms)",
        first_took.as_millis(),
        first_cpu,
        first.passages.len(),
        median(&mut reads),
        median(&mut cpus)
    );
    println!(
        "  {label}: page footprint {base} KB before asking, {after_first} KB after the first ask (+{} KB), {after_reads} KB after 10 more reads (+{} KB)",
        after_first.saturating_sub(base),
        after_reads.saturating_sub(base)
    );
    println!(
        "  {label}: selection {} ms, outline {} ms, mark {} ms",
        selection.as_millis(),
        outline.as_millis(),
        mark.as_millis()
    );
    Ok(())
}

fn measure(mtm: MainThreadMarker, fixture: &Fixture, article: Option<&str>) -> Result<(), String> {
    println!("measure: bootstrap cost per page load (A/B, alternating, 40 loads each)");
    let with = Probe::new(mtm, true)?;
    let without = Probe::new(mtm, false)?;
    for path in ["/plain", "/heavy"] {
        let url = fixture.url(path);
        for _ in 0..3 {
            with.load(&url)?;
            without.load(&url)?;
        }
        let mut samples: HashMap<bool, (Vec<f64>, Vec<f64>)> = HashMap::new();
        for _ in 0..40 {
            for (probe, enabled) in [(&with, true), (&without, false)] {
                let cpu = probe.cpu_ns();
                let started = Instant::now();
                probe.load(&url)?;
                let wall = started.elapsed().as_secs_f64() * 1e3;
                let cpu = probe.cpu_ns().zip(cpu).map_or(0.0, |(after, before)| {
                    after.saturating_sub(before) as f64 / 1e6
                });
                let entry = samples.entry(enabled).or_default();
                entry.0.push(wall);
                entry.1.push(cpu);
            }
        }
        for enabled in [true, false] {
            let (mut wall, mut cpu) = samples.remove(&enabled).unwrap_or_default();
            println!(
                "  {path:<7} reader={:<5} load median {:.2} ms; page-process CPU per load median {:.2} ms, mean {:.2} ms",
                enabled,
                median(&mut wall),
                median(&mut cpu.clone()),
                mean(&cpu)
            );
            cpu.clear();
        }
        settle(Duration::from_millis(300));
        println!(
            "  {path:<7} settled page footprint: reader {} KB, no reader {} KB",
            with.settled_footprint(),
            without.settled_footprint()
        );
    }
    drop(without);

    println!("measure: page memory in one process, reader on and off in alternating blocks");
    let heavy = fixture.url("/heavy");
    let mut blocks: HashMap<bool, Vec<u64>> = HashMap::new();
    for _ in 0..3 {
        for enabled in [true, false] {
            ENABLED.with(|on| on.set(enabled));
            if let Some(reader) = with.reader.borrow().as_ref() {
                reader.set_enabled(enabled);
            }
            for _ in 0..12 {
                with.load(&heavy)?;
            }
            blocks
                .entry(enabled)
                .or_default()
                .push(with.settled_footprint());
        }
    }
    ENABLED.with(|on| on.set(true));
    if let Some(reader) = with.reader.borrow().as_ref() {
        reader.set_enabled(true);
    }
    for enabled in [true, false] {
        println!(
            "  reader={enabled:<5} settled footprint after each 12-load block: {:?} KB",
            blocks.get(&enabled).cloned().unwrap_or_default()
        );
    }

    println!("measure: memory once the person leaves a page they asked about");
    let plain = fixture.url("/plain");
    with.load(&plain)?;
    let before = with.settled_footprint();
    let epoch = with.load(&heavy)?;
    with.wait_readable()?;
    for _ in 0..5 {
        let _ = with.read(epoch, &heavy);
    }
    let asked = with.settled_footprint();
    with.load(&plain)?;
    let left = with.settled_footprint();
    println!(
        "  on /plain {before} KB; on /heavy after 5 reads {asked} KB; back on /plain {left} KB ({:+} KB)",
        i64::try_from(left).unwrap_or(0) - i64::try_from(before).unwrap_or(0)
    );

    println!("measure: first ask, reads and memory");
    ask_costs(&with, &fixture.url("/plain"), "/plain")?;
    ask_costs(&with, &fixture.url("/heavy"), "/heavy")?;
    if let Some(article) = article {
        ask_costs(&with, article, "live article")?;
    }
    Ok(())
}

pub(crate) fn run(arguments: &[String]) -> Result<(), String> {
    let mtm = MainThreadMarker::new().ok_or("the probe must run on the main thread")?;
    let app = NSApplication::sharedApplication(mtm);
    let _ = app.setActivationPolicy(NSApplicationActivationPolicy::Prohibited);
    let fixture = Fixture::start()?;
    let measuring = arguments.iter().any(|a| a == "--measure");
    let wikipedia = arguments
        .iter()
        .position(|a| a == "--article")
        .and_then(|i| arguments.get(i + 1))
        .map(String::as_str);
    let outcome = objc2::rc::autoreleasepool(|_| {
        if measuring {
            measure(mtm, &fixture, wikipedia)
        } else {
            fixtures(mtm, &fixture)
        }
    });
    drop(fixture);
    outcome
}
