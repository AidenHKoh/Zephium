//! Opt-in, release-excluded public-form final-state qualification.
//! A timed-out observer never invents native callback drain or a network proof.
use super::*;
use crate::platform::work_document_navigation::{WorkDocumentNavigation, WorkDocumentStamp};
use objc2::runtime::AnyObject;
use objc2_foundation::{MainThreadMarker, NSError, NSString};
use objc2_web_kit::WKContentWorld;
use std::sync::Mutex;
use wry::WebViewExtMacOS as _;

const SOURCE: &str = include_str!("work_form_oracle.js");
const LIMIT: Duration = Duration::from_secs(2);

#[derive(Default)]
pub(super) enum State {
    #[default]
    Unstarted,
    Waiting(Box<Pending>),
    Finished,
}
#[derive(Default)]
struct Completion {
    returned: bool,
    timed_out: bool,
    checks: Option<Vec<Option<bool>>>,
}
impl Completion {
    fn expire(&mut self) {
        self.timed_out = true;
        self.checks = None;
    }
    fn settle(&mut self, checks: Option<Vec<Option<bool>>>) -> bool {
        if self.returned {
            return false;
        }
        self.returned = true;
        if !self.timed_out {
            self.checks = checks;
        }
        true
    }
}
pub(super) struct Pending {
    case: &'static str,
    url: &'static str,
    gate: WorkDocumentNavigation,
    stamp: WorkDocumentStamp,
    invocation: u64,
    started: Instant,
    completion: Arc<Mutex<Completion>>,
    watchdog: Option<crate::platform::imp::ContentPolicyTimeout>,
    reported: bool,
}

fn admission_current(
    session: Option<&zephium_agentic::WorkBrowserSession>,
    healthy: bool,
    ready: bool,
    isolated_public: bool,
    work_website_data: bool,
) -> bool {
    healthy
        && ready
        && isolated_public
        && !work_website_data
        && session.is_some_and(zephium_agentic::WorkBrowserSession::is_current)
}
fn target(case: &str) -> Option<(&'static str, &'static str)> {
    Some(match case {
        "form-httpbin-draft" => ("form-httpbin-draft", "https://httpbin.org/forms/post"),
        "form-selenium-controls" => (
            "form-selenium-controls",
            "https://www.selenium.dev/selenium/web/web-form.html",
        ),
        "form-selenium-date" => (
            "form-selenium-date",
            "https://www.selenium.dev/selenium/web/web-form.html",
        ),
        "form-expand-inputs" => (
            "form-expand-inputs",
            "https://practice.expandtesting.com/inputs",
        ),
        "form-heroku-numeric" => (
            "form-heroku-numeric",
            "https://the-internet.herokuapp.com/inputs",
        ),
        "form-heroku-dropdown" => (
            "form-heroku-dropdown",
            "https://the-internet.herokuapp.com/dropdown",
        ),
        "form-heroku-checkboxes" => (
            "form-heroku-checkboxes",
            "https://the-internet.herokuapp.com/checkboxes",
        ),
        "form-heroku-editor" => (
            "form-heroku-editor",
            "https://the-internet.herokuapp.com/tinymce",
        ),
        "form-qa-select" => ("form-qa-select", "https://demoqa.com/select-menu"),
        "form-qa-date" => ("form-qa-date", "https://demoqa.com/date-picker"),
        _ => return None,
    })
}
fn decode(case: &str, bytes: &str) -> Option<Vec<Option<bool>>> {
    if bytes.len() > 256 {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(bytes).ok()?;
    let object = value.as_object()?;
    if object.len() != 2 || object.get("version")?.as_u64()? != 1 {
        return None;
    }
    let checks = object.get("checks")?.as_array()?;
    let expected = match case {
        "form-httpbin-draft" => 11,
        "form-selenium-controls" | "form-expand-inputs" => 3,
        "form-selenium-date" | "form-heroku-numeric" | "form-heroku-checkboxes" => 2,
        _ => 1,
    };
    if checks.len() != expected {
        return None;
    }
    checks
        .iter()
        .map(|value| {
            if value.is_null() {
                Some(None)
            } else {
                value.as_bool().map(Some)
            }
        })
        .collect()
}
#[allow(clippy::print_stderr)]
fn report(
    resource: &WorkNativeResource,
    pending: &Pending,
    checks: Option<&[Option<bool>]>,
    reason: &'static str,
) {
    let identity = resource.guard.resource().identity();
    let evidence = serde_json::json!({
        "version":1, "case":pending.case,
        "profile":format!("{:?}", identity.profile()),
        "work":format!("{:?}", identity.work()),
        "context":format!("{:?}", identity.context()),
        "document":format!("{:?}", pending.stamp), "invocation":pending.invocation,
        "checks":checks, "reason":reason,
        "observer_ms":pending.started.elapsed().as_millis(),
        "accepted_action_history":"not_observed", "network_no_submit":"unknown",
        "content_exported":false
    });
    eprintln!("form-oracle: {evidence}");
}

pub(super) fn before_retire(resource: &mut WorkNativeResource) -> bool {
    let mut state = std::mem::take(&mut resource.form_oracle);
    let ready = state.progress(resource);
    resource.form_oracle = state;
    ready
}
impl State {
    fn progress(&mut self, resource: &mut WorkNativeResource) -> bool {
        match self {
            Self::Finished => true,
            Self::Unstarted => {
                let selection = std::env::var("ZEPHIUM_EVAL_FORM_ORACLE")
                    .ok()
                    .filter(|_| std::env::var_os("ZEPHIUM_EVAL_PUBLIC_EXPORT").is_some())
                    .and_then(|case| target(&case));
                let Some((case, url)) = selection else {
                    *self = Self::Finished;
                    return true;
                };
                // No sampling of cancelled, uncertain, session-backed or different pages.
                if !admission_current(
                    resource.guard.anonymous_session(),
                    resource.guard.is_healthy(),
                    resource.ready(),
                    resource.guard.isolated_public(),
                    resource.guard.work_website_data(),
                ) || resource.last_invocation == 0
                {
                    *self = Self::Finished;
                    return true;
                }
                if resource.observation.is_some()
                    || resource.action.is_some()
                    || resource.navigation.is_some()
                    || resource.history_back_pending()
                {
                    // A diagnostic cannot delay existing cleanup to manufacture
                    // an idle sampling point. Missing final evidence stays unknown.
                    *self = Self::Finished;
                    return true;
                }
                let Some(view) = resource.view.as_ref() else {
                    *self = Self::Finished;
                    return true;
                };
                if crate::platform::imp::current_url(view.view()).as_deref() != Some(url) {
                    *self = Self::Finished;
                    return true;
                }
                let Some(gate) = view.work_navigation().cloned() else {
                    *self = Self::Finished;
                    return true;
                };
                let Some(stamp) = gate.document_stamp() else {
                    *self = Self::Finished;
                    return true;
                };
                let Some(mtm) = MainThreadMarker::new() else {
                    *self = Self::Finished;
                    return true;
                };
                let completion = Arc::new(Mutex::new(Completion::default()));
                let expired = completion.clone();
                let wake = resource.guard.clone();
                let watchdog =
                    crate::platform::imp::schedule_content_policy_timeout(LIMIT, move || {
                        let mut state = expired
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        state.expire();
                        drop(state);
                        notify_work_resource(wake);
                    });
                if watchdog.is_none() {
                    *self = Self::Finished;
                    return true;
                }
                let returned = completion.clone();
                let wake = resource.guard.clone();
                let callback: block2::RcBlock<dyn Fn(*mut AnyObject, *mut NSError)> =
                    block2::RcBlock::new(move |value: *mut AnyObject, error: *mut NSError| {
                        // SAFETY: WebKit supplies live callback objects on the main thread.
                        let checks = if error.is_null() {
                            unsafe { value.as_ref() }
                                .and_then(AnyObject::downcast_ref::<NSString>)
                                .filter(|text| text.length() <= 256)
                                .and_then(|text| decode(case, &text.to_string()))
                        } else {
                            None
                        };
                        let mut state = returned
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        if !state.settle(checks) {
                            return;
                        }
                        drop(state);
                        let queued = wake.clone();
                        wake.dispatch_notification(move || notify_work_resource(queued));
                    });
                let source = NSString::from_str(
                    &SOURCE.replace("__FORM_ORACLE_CASE__", &format!("\"{case}\"")),
                );
                // SAFETY: named isolated world, fixed allowlisted read-only source, exact
                // owned live main-frame view; the callback owns only bounded evidence.
                let dispatched = objc2::exception::catch(std::panic::AssertUnwindSafe(|| unsafe {
                    let world = WKContentWorld::worldWithName(
                        &NSString::from_str("zephium-public-form-oracle-v1"),
                        mtm,
                    );
                    view.view()
                        .webview()
                        .evaluateJavaScript_inFrame_inContentWorld_completionHandler(
                            &source,
                            None,
                            &world,
                            Some(&callback),
                        );
                }))
                .is_ok();
                if !dispatched {
                    // An exceptional native dispatch cannot prove callback absence.
                    completion
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .timed_out = true;
                }
                *self = Self::Waiting(Box::new(Pending {
                    case,
                    url,
                    gate,
                    stamp,
                    invocation: resource.last_invocation,
                    started: Instant::now(),
                    completion,
                    watchdog,
                    reported: false,
                }));
                false
            }
            Self::Waiting(pending) => {
                let completion = pending
                    .completion
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if !completion.returned && !completion.timed_out {
                    return false;
                }
                let expired = completion.timed_out || pending.started.elapsed() > LIMIT;
                let current = admission_current(
                    resource.guard.anonymous_session(),
                    resource.guard.is_healthy(),
                    resource.ready(),
                    resource.guard.isolated_public(),
                    resource.guard.work_website_data(),
                ) && pending.gate.document_stamp() == Some(pending.stamp)
                    && resource.last_invocation == pending.invocation
                    && resource.view.as_ref().is_some_and(|view| {
                        crate::platform::imp::current_url(view.view()).as_deref()
                            == Some(pending.url)
                    });
                if !pending.reported {
                    let reason = if expired {
                        "timed_out"
                    } else if !current {
                        "stale_document"
                    } else if completion.checks.is_none() {
                        "unavailable"
                    } else {
                        "observed"
                    };
                    report(
                        resource,
                        pending,
                        if current && !expired {
                            completion.checks.as_deref()
                        } else {
                            None
                        },
                        reason,
                    );
                    pending.reported = true;
                }
                if !completion.returned {
                    return false;
                }
                drop(completion);
                pending.watchdog = None;
                *self = Self::Finished;
                true
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn revoked_or_expired_session_cannot_sample_a_healthy_ready_page() {
        let session = zephium_agentic::WorkBrowserSession::new(
            zephium_core::ids::ProfileId::generate(),
            zephium_agentic::WorkId::generate(),
            Instant::now() + Duration::from_secs(10),
        );
        assert!(admission_current(Some(&session), true, true, true, false));
        assert!(
            !admission_current(Some(&session), true, true, false, false),
            "profile/account store is excluded"
        );
        assert!(
            !admission_current(Some(&session), true, true, true, true),
            "persistent Work store is excluded"
        );
        session.close();
        assert!(!admission_current(Some(&session), true, true, true, false));
        let expired = zephium_agentic::WorkBrowserSession::new(
            zephium_core::ids::ProfileId::generate(),
            zephium_agentic::WorkId::generate(),
            Instant::now(),
        );
        assert!(!admission_current(Some(&expired), true, true, true, false));
        assert!(!admission_current(None, true, true, true, false));
    }
    #[test]
    fn timeout_keeps_callback_debt_and_late_or_duplicate_results_cannot_pass() {
        let mut timed_out = Completion::default();
        timed_out.expire();
        assert!(!timed_out.returned, "a timeout is not callback return");
        assert!(timed_out.settle(Some(vec![Some(true)])));
        assert!(timed_out.returned);
        assert!(timed_out.checks.is_none(), "late content stays unavailable");
        let mut completed = Completion::default();
        assert!(completed.settle(Some(vec![Some(false)])));
        assert!(!completed.settle(Some(vec![Some(true)])));
        assert_eq!(completed.checks, Some(vec![Some(false)]));
    }
    #[test]
    fn form_payload_is_closed_boolean_evidence_only() {
        assert_eq!(
            decode(
                "form-expand-inputs",
                r#"{"version":1,"checks":[true,false,null]}"#
            ),
            Some(vec![Some(true), Some(false), None])
        );
        for invalid in [
            r#"{"version":1,"checks":["private"]}"#,
            r#"{"version":1,"checks":[],"value":"secret"}"#,
            r#"{"version":2,"checks":[true]}"#,
        ] {
            assert!(decode("form-expand-inputs", invalid).is_none());
        }
        assert!(target("https://example.com").is_none());
        assert!(target("form-heroku-numeric").is_some());
    }
}
