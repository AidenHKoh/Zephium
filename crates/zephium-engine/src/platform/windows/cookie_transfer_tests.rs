//! Synchronous COM doubles exercise the native adapter's write/cleanup boundary.

use super::*;
use webview2_com::Microsoft::Web::WebView2::Win32::*;
use windows_core::{Ref, BOOL, HRESULT};
use zephium_agentic::ContextCookieOrigin;

fn native_failure() -> windows_core::Error {
    windows_core::Error::from_hresult(HRESULT(0x80004005_u32 as i32))
}

fn output<T>(target: *mut T, value: T) -> windows_core::Result<()> {
    if target.is_null() {
        return Err(native_failure());
    }
    // SAFETY: each caller is a COM getter receiving the adapter's initialized
    // writable output of the exact binding type; null is rejected above.
    unsafe { target.write(value) };
    Ok(())
}

#[windows_core::implement(ICoreWebView2Cookie)]
struct TestCookie {
    name: String,
    value: String,
}

impl ICoreWebView2Cookie_Impl for TestCookie_Impl {
    fn Name(&self, value: *mut PWSTR) -> windows_core::Result<()> {
        output(value, webview2_com::pwstr_from_str(&self.name))
    }
    fn Value(&self, value: *mut PWSTR) -> windows_core::Result<()> {
        output(value, webview2_com::pwstr_from_str(&self.value))
    }
    fn Domain(&self, value: *mut PWSTR) -> windows_core::Result<()> {
        output(value, webview2_com::pwstr_from_str("example.test"))
    }
    fn Path(&self, value: *mut PWSTR) -> windows_core::Result<()> {
        output(value, webview2_com::pwstr_from_str("/"))
    }
    fn Expires(&self, value: *mut f64) -> windows_core::Result<()> {
        output(value, -1.0)
    }
    fn IsHttpOnly(&self, value: *mut BOOL) -> windows_core::Result<()> {
        output(value, true.into())
    }
    fn IsSecure(&self, value: *mut BOOL) -> windows_core::Result<()> {
        output(value, true.into())
    }
    fn IsSession(&self, value: *mut BOOL) -> windows_core::Result<()> {
        output(value, true.into())
    }
    fn SameSite(&self, value: *mut COREWEBVIEW2_COOKIE_SAME_SITE_KIND) -> windows_core::Result<()> {
        output(value, COREWEBVIEW2_COOKIE_SAME_SITE_KIND_LAX)
    }
    fn SetValue(&self, _: &PCWSTR) -> windows_core::Result<()> {
        Err(native_failure())
    }
    fn SetExpires(&self, _: f64) -> windows_core::Result<()> {
        Err(native_failure())
    }
    fn SetIsHttpOnly(&self, _: BOOL) -> windows_core::Result<()> {
        Err(native_failure())
    }
    fn SetSameSite(&self, _: COREWEBVIEW2_COOKIE_SAME_SITE_KIND) -> windows_core::Result<()> {
        Err(native_failure())
    }
    fn SetIsSecure(&self, _: BOOL) -> windows_core::Result<()> {
        Err(native_failure())
    }
}

fn cookie(name: &str, value: &str) -> ICoreWebView2Cookie {
    TestCookie {
        name: name.into(),
        value: value.into(),
    }
    .into()
}

#[windows_core::implement(ICoreWebView2CookieList)]
struct TestCookieList(Vec<ICoreWebView2Cookie>);
impl ICoreWebView2CookieList_Impl for TestCookieList_Impl {
    fn Count(&self, value: *mut u32) -> windows_core::Result<()> {
        output(value, self.0.len() as u32)
    }
    fn GetValueAtIndex(&self, index: u32) -> windows_core::Result<ICoreWebView2Cookie> {
        self.0
            .get(index as usize)
            .cloned()
            .ok_or_else(native_failure)
    }
}

#[derive(Default)]
struct Store {
    cookies: RefCell<Vec<ICoreWebView2Cookie>>,
    writes: Cell<usize>,
    deletes: Cell<usize>,
    broad_clears: Cell<usize>,
    fail_write: Cell<Option<usize>>,
    fail_delete: Cell<bool>,
}

#[windows_core::implement(ICoreWebView2CookieManager)]
struct TestManager(Rc<Store>);
impl ICoreWebView2CookieManager_Impl for TestManager_Impl {
    fn CreateCookie(
        &self,
        _: &PCWSTR,
        _: &PCWSTR,
        _: &PCWSTR,
        _: &PCWSTR,
    ) -> windows_core::Result<ICoreWebView2Cookie> {
        Err(native_failure())
    }
    fn CopyCookie(
        &self,
        cookie: Ref<'_, ICoreWebView2Cookie>,
    ) -> windows_core::Result<ICoreWebView2Cookie> {
        Ok(cookie.ok()?.clone())
    }
    fn GetCookies(
        &self,
        _: &PCWSTR,
        handler: Ref<'_, ICoreWebView2GetCookiesCompletedHandler>,
    ) -> windows_core::Result<()> {
        let list: ICoreWebView2CookieList = TestCookieList(self.0.cookies.borrow().clone()).into();
        // SAFETY: the callback is the adapter's live COM owner and the list is
        // retained through synchronous Invoke. This also exercises reentrancy.
        unsafe { handler.ok()?.Invoke(HRESULT(0), &list) }
    }
    fn AddOrUpdateCookie(&self, cookie: Ref<'_, ICoreWebView2Cookie>) -> windows_core::Result<()> {
        let cookie = cookie.ok()?;
        let identity = cookie_identity(cookie).map_err(|_| native_failure())?;
        self.0
            .cookies
            .borrow_mut()
            .retain(|current| cookie_identity(current).is_ok_and(|current| current != identity));
        self.0.cookies.borrow_mut().push(cookie.clone());
        let number = self.0.writes.get() + 1;
        self.0.writes.set(number);
        // A failed COM call need not prove that no native mutation occurred.
        if self.0.fail_write.get() == Some(number) {
            Err(native_failure())
        } else {
            Ok(())
        }
    }
    fn DeleteCookie(&self, cookie: Ref<'_, ICoreWebView2Cookie>) -> windows_core::Result<()> {
        self.0.deletes.set(self.0.deletes.get() + 1);
        if self.0.fail_delete.get() {
            return Err(native_failure());
        }
        let identity = cookie_identity(cookie.ok()?).map_err(|_| native_failure())?;
        self.0
            .cookies
            .borrow_mut()
            .retain(|current| cookie_identity(current).is_ok_and(|current| current != identity));
        Ok(())
    }
    fn DeleteCookies(&self, _: &PCWSTR, _: &PCWSTR) -> windows_core::Result<()> {
        Err(native_failure())
    }
    fn DeleteCookiesWithDomainAndPath(
        &self,
        _: &PCWSTR,
        _: &PCWSTR,
        _: &PCWSTR,
    ) -> windows_core::Result<()> {
        Err(native_failure())
    }
    fn DeleteAllCookies(&self) -> windows_core::Result<()> {
        self.0.broad_clears.set(self.0.broad_clears.get() + 1);
        self.0.cookies.borrow_mut().clear();
        Ok(())
    }
}

fn sync(source: Vec<ICoreWebView2Cookie>, destination: Rc<Store>) -> WindowsAgentCookieTerminal {
    let source = Rc::new(Store {
        cookies: RefCell::new(source),
        ..Store::default()
    });
    let completion = Rc::new(Cell::new(None));
    let completed = completion.clone();
    let panicked = Rc::new(Cell::new(false));
    let panic_flag = panicked.clone();
    let owner = WindowsAgentCookieTransfer::start_work_sync(
        TestManager(source).into(),
        TestManager(destination).into(),
        ContextCookieScope::try_new(vec![
            ContextCookieOrigin::parse("https://example.test/").unwrap(),
            ContextCookieOrigin::parse("https://app.example.test/").unwrap(),
        ])
        .unwrap(),
        Instant::now() + Duration::from_secs(30),
        move |terminal| completed.set(Some(terminal)),
        move || panic_flag.set(true),
    )
    .unwrap();
    assert!(owner.is_terminal());
    assert!(!panicked.get());
    completion.get().expect("synchronous adapter terminal")
}

fn holds(store: &Store, expected: &[&ICoreWebView2Cookie]) {
    let cookies = store.cookies.borrow();
    assert_eq!(cookies.len(), expected.len());
    // The same exact objects prove each value and its attributes.
    for cookie in expected {
        assert!(cookies.iter().any(|held| held.as_raw() == cookie.as_raw()));
    }
    assert_eq!(store.broad_clears.get(), 0);
}

#[test]
fn work_sync_updates_changed_identities_and_keeps_destination_only_ones() {
    let kept = cookie("work-only", "signed-in-in-work");
    let destination = Rc::new(Store {
        cookies: RefCell::new(vec![kept.clone(), cookie("session", "old")]),
        ..Store::default()
    });
    let session = cookie("session", "new");
    let unrelated = cookie("unrelated", "tab");
    let terminal = sync(
        vec![session.clone(), unrelated.clone()],
        destination.clone(),
    );
    assert!(matches!(
        terminal.outcome(),
        ContextCookieTransferOutcome::Applied(_)
    ));
    assert_eq!(terminal.cleanup(), WindowsAgentCookieCleanup::NotRequired);
    assert_eq!(destination.writes.get(), 2);
    assert_eq!(destination.deletes.get(), 0);
    holds(&destination, &[&kept, &session, &unrelated]);
}

#[test]
fn work_sync_leaves_an_unchanged_cookie_unwritten() {
    let held = cookie("session", "same");
    let destination = Rc::new(Store {
        cookies: RefCell::new(vec![held.clone()]),
        ..Store::default()
    });
    let terminal = sync(vec![cookie("session", "same")], destination.clone());
    assert!(matches!(
        terminal.outcome(),
        ContextCookieTransferOutcome::Applied(_)
    ));
    assert_eq!(destination.writes.get(), 0);
    holds(&destination, &[&held]);
}

#[test]
fn work_sync_partial_failure_deletes_new_and_restores_overwritten_identities() {
    let kept = cookie("work", "work-human-session");
    let prior = cookie("session", "old");
    let destination = Rc::new(Store {
        cookies: RefCell::new(vec![kept.clone(), prior.clone()]),
        fail_write: Cell::new(Some(2)),
        ..Store::default()
    });
    let terminal = sync(
        vec![cookie("first", "one"), cookie("session", "new")],
        destination.clone(),
    );
    assert!(matches!(
        terminal.outcome(),
        ContextCookieTransferOutcome::Partial {
            failure: ContextCookieTransferFailure::ApplicationFailed,
            ..
        }
    ));
    assert_eq!(terminal.cleanup(), WindowsAgentCookieCleanup::Proven);
    assert_eq!(destination.deletes.get(), 1);
    holds(&destination, &[&kept, &prior]);
}

#[test]
fn work_sync_first_failed_native_write_is_also_journaled() {
    let kept = cookie("work", "work-human-session");
    let destination = Rc::new(Store {
        cookies: RefCell::new(vec![kept.clone()]),
        fail_write: Cell::new(Some(1)),
        ..Store::default()
    });
    let terminal = sync(vec![cookie("first", "one")], destination.clone());
    assert_eq!(
        terminal.outcome(),
        ContextCookieTransferOutcome::Refused(ContextCookieTransferFailure::ApplicationFailed)
    );
    assert_eq!(terminal.cleanup(), WindowsAgentCookieCleanup::Proven);
    assert_eq!(destination.deletes.get(), 1);
    holds(&destination, &[&kept]);
}

#[test]
fn work_sync_failed_rollback_requires_quarantine_without_broad_clear() {
    let kept = cookie("work", "work-human-session");
    let destination = Rc::new(Store {
        cookies: RefCell::new(vec![kept.clone()]),
        fail_write: Cell::new(Some(1)),
        fail_delete: Cell::new(true),
        ..Store::default()
    });
    let terminal = sync(vec![cookie("first", "one")], destination.clone());
    assert_eq!(terminal.cleanup(), WindowsAgentCookieCleanup::Unproven);
    assert_eq!(destination.broad_clears.get(), 0);
    assert_eq!(destination.cookies.borrow().len(), 2);
    assert_eq!(destination.cookies.borrow()[0].as_raw(), kept.as_raw());
}
