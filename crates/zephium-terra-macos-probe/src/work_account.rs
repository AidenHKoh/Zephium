//! Explicit read-only qualification in a fresh probe profile. Account contents
//! remain in the ephemeral Work store; only closed counters leave this adapter.
use std::{
    io::{Read, Write},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex, OnceLock,
    },
    time::Duration,
};
use zephium_core::{
    ids::ProfileId,
    work::{runtime::*, search::*, *},
};
use zephium_ipc::work::*;
use zephium_work_composition::MacosWorkComposition;

pub(super) struct Account {
    origin: &'static str,
    entry: String,
    input: Mutex<mpsc::Receiver<Input>>,
    stopped: Arc<AtomicBool>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Input {
    Check,
    Stop,
}
static ACCOUNT: OnceLock<Account> = OnceLock::new();
pub(super) fn active() -> Option<&'static Account> {
    ACCOUNT.get()
}
fn origin(url: &str) -> Option<&'static str> {
    [
        "https://app.slack.com/",
        "https://app.todoist.com/",
        "https://linear.app/",
    ]
    .into_iter()
    .find(|origin| url.starts_with(origin))
    .filter(|_| {
        url.len() <= 1024
            && url.is_ascii()
            && !url
                .bytes()
                .any(|b| b.is_ascii_control() || b == b' ' || b == b'\\' || b == b'?' || b == b'#')
    })
}
fn command(bytes: &[u8]) -> Option<Input> {
    match bytes {
        b"check" => Some(Input::Check),
        b"stop" => Some(Input::Stop),
        _ => None,
    }
}
fn route_command(bytes: &[u8], tx: &mpsc::SyncSender<Input>, stopped: &AtomicBool) {
    match command(bytes) {
        Some(Input::Stop) => stopped.store(true, Ordering::Release),
        Some(Input::Check) => {
            let _ = tx.try_send(Input::Check);
        }
        None => {}
    }
}
async fn unless_stopped<T>(
    stopped: &AtomicBool,
    task: impl std::future::Future<Output = T>,
) -> Option<T> {
    tokio::select! {
        result = task => Some(result),
        _ = async { while !stopped.load(Ordering::Acquire) { tokio::time::sleep(Duration::from_millis(25)).await; } } => None,
    }
}
pub(super) fn configure(url: &str) -> Result<String, &'static str> {
    let origin = origin(url).ok_or("account_site")?;
    // No inherited profile, seeded cookies, debug/export or model override.
    if std::env::vars_os().any(|(key, _)| key.to_string_lossy().starts_with("ZEPHIUM_")) {
        return Err("account_environment");
    }
    let host = origin.trim_start_matches("https://").trim_end_matches('/');
    let entry =
        zephium_app::work_sites::entry_question_for(&[zephium_app::work_sites::service_name(host)])
            .0;
    let (tx, rx) = mpsc::sync_channel(4);
    let stopped = Arc::new(AtomicBool::new(false));
    let input_stop = stopped.clone();
    std::thread::Builder::new()
        .name("account-probe-input".into())
        .spawn(move || {
            let mut line = [0u8; 8];
            let mut length = 0usize;
            let mut byte = [0u8; 1];
            while std::io::stdin().read_exact(&mut byte).is_ok() {
                if byte[0] == b'\n' {
                    if length <= line.len() {
                        route_command(&line[..length], &tx, &input_stop);
                    }
                    line.fill(0);
                    length = 0;
                } else {
                    if length < line.len() {
                        line[length] = byte[0];
                    }
                    length = length.saturating_add(1);
                }
            }
        })
        .map_err(|_| "account_input")?;
    ACCOUNT
        .set(Account {
            origin,
            entry,
            input: Mutex::new(rx),
            stopped,
        })
        .map_err(|_| "account_duplicate")?;
    Ok(format!("Open my test account at {url} with the Browser helper and read only the currently visible channel, inbox or issue list. If sign-in or MFA is needed, hand the live page to me and resume after native verification. Summarize at most three visible items with sources, or report honestly what blocks access. Do not search, substitute documentation, use Computer, type into account content, send, create, edit or delete anything.\nExplain the previous result from recorded state and evidence. If access was blocked, say exactly what is needed; do not retry or open a new page just to explain."))
}
impl Account {
    pub(super) fn stopped(&self) -> bool {
        self.stopped.load(Ordering::Acquire)
    }
    pub(super) fn permits(
        &self,
        request: &zephium_app::work_agent::WorkAgentBrowseRequest,
    ) -> bool {
        request.private_run
            && request.session == zephium_app::work_sites::SiteSession::Private
            && matches!(&request.step, WorkStepKindV1::Read { url, .. } if origin(url) == Some(self.origin))
    }
    fn answer(&self, kind: &WorkStepKindV1) -> Option<String> {
        match kind {
            WorkStepKindV1::Ask {
                prompt,
                options,
                answer: None,
                purpose,
                ..
            } => {
                let answer = if *purpose == Some(WorkAskPurposeV1::Entry)
                    && *prompt == self.entry
                    && options.iter().any(|o| o == "Allow")
                {
                    // The CLI invocation authorizes this one named test site.
                    "Allow"
                } else if *purpose == Some(WorkAskPurposeV1::Question) {
                    "Read only the specified account page. Do not make changes."
                } else {
                    "Not now"
                };
                Some(answer.into())
            }
            _ => None,
        }
    }
    pub(super) async fn person(
        &self,
        handle: &zephium_app::Handle,
        composition: &MacosWorkComposition,
        profile: ProfileId,
        work: WorkId,
    ) {
        let mut presented = Vec::new();
        let mut previous = None;
        loop {
            tokio::time::sleep(Duration::from_millis(100)).await;
            let input = self.input.lock().ok().and_then(|rx| rx.try_recv().ok());
            if let Ok(pages) = composition.human_pages(profile, work) {
                if let Some(page) = pages.iter().find(|page| {
                    matches!(
                        page.phase,
                        WorkHumanPhaseV1::WaitingForHuman
                            | WorkHumanPhaseV1::Presenting
                            | WorkHumanPhaseV1::Presented
                    )
                }) {
                    let state = (page.id, page.phase, page.reason);
                    if previous != Some(state) {
                        let _ = writeln!(
                            std::io::stdout().lock(),
                            "account-probe: handoff={:?} phase={:?}; content=redacted",
                            page.reason,
                            page.phase
                        );
                        previous = Some(state);
                    }
                    if page.phase == WorkHumanPhaseV1::WaitingForHuman
                        && !presented.contains(&page.id)
                    {
                        if composition
                            .present_human_page(
                                profile,
                                work,
                                page.id,
                                WorkHumanRegionV1 {
                                    x: 0,
                                    y: 0,
                                    width: 760,
                                    height: 640,
                                },
                            )
                            .is_ok()
                        {
                            presented.push(page.id);
                        }
                    }
                    if input == Some(Input::Check) {
                        if let Some(result) = unless_stopped(
                            &self.stopped,
                            composition.check_human_page(profile, work, page.id),
                        )
                        .await
                        {
                            let _ = writeln!(
                                std::io::stdout().lock(),
                                "account-probe: native_check={result:?}; content=redacted"
                            );
                        }
                    }
                }
            }
            let Ok(request) = handle.work_projection(profile, work) else {
                continue;
            };
            let Ok(response) =
                tokio::time::timeout(Duration::from_millis(200), request.response(profile)).await
            else {
                continue;
            };
            let WorkReplyV1::Projection { projection } = response.reply else {
                continue;
            };
            let Some(execution) = projection
                .executions
                .iter()
                .rev()
                .find(|e| !e.status.terminal())
            else {
                continue;
            };
            let intent = if self.stopped() {
                Some(WorkRuntimeIntent::Cancel {
                    execution: execution.id,
                    intervention: None,
                })
            } else {
                execution.steps.iter().find_map(|step| {
                    if step.status != WorkStepStatus::Running {
                        return None;
                    }
                    if let Some(answer) = self.answer(&step.kind) {
                        return Some(WorkRuntimeIntent::AnswerStep {
                            execution: execution.id,
                            step: step.id,
                            answer,
                        });
                    }
                    let waiting = match &step.kind {
                        WorkStepKindV1::Confirm { confirm } => confirm.decision.is_none(),
                        kind => kind.proposes_write() && kind.file_decision().is_none(),
                    };
                    waiting.then_some(WorkRuntimeIntent::ApproveStep {
                        execution: execution.id,
                        step: step.id,
                        approve: false,
                        for_run: false,
                    })
                })
            };
            if let Some(intent) = intent {
                if let Ok(request) = handle.work_command(
                    profile,
                    WorkCommandV1 {
                        version: 1,
                        work,
                        expected_revision: projection.work.revision,
                        command: WorkCommandId::generate(),
                        intent,
                    },
                ) {
                    let _ =
                        tokio::time::timeout(Duration::from_millis(200), request.response(profile))
                            .await;
                }
            }
        }
    }
}
pub(super) struct NoSearch;
impl WorkPublicSearchProvider for NoSearch {
    fn search<'a>(
        &'a self,
        _: &'a WorkPublicSearchScope,
        _: &'a [zephium_core::work::context::WorkContextBody],
        _: WorkExecutionLimits,
    ) -> WorkPublicSearchFuture<'a> {
        Box::pin(async { Err(WorkPublicSearchError::NotDispatched(WorkError::Unavailable)) })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn account_origins_and_input_are_closed() {
        assert_eq!(
            origin("https://app.slack.com/client/T0/C0"),
            Some("https://app.slack.com/")
        );
        assert!(origin("https://app.todoist.com/app/inbox").is_some());
        assert!(origin("https://linear.app/test/team").is_some());
        for bad in [
            "https://app.slack.com.evil.test/",
            "https://app.slack.com@evil.test/",
            "https://app.slack.com:443/",
            "http://app.slack.com/",
            "https://app.slack.com/\\evil",
            "https://app.slack.com/?token=test",
            "https://app.slack.com/#token=test",
        ] {
            assert!(origin(bad).is_none());
        }
        assert!(command(b"check").is_some());
        assert!(command(b"stop").is_some());
        for bad in [b"approve".as_slice(), b"continue", b"allow", b"password"] {
            assert!(command(bad).is_none());
        }
    }
    #[test]
    fn only_the_exact_native_entry_question_can_be_allowed() {
        let (_, rx) = mpsc::sync_channel(1);
        let account = Account {
            origin: "https://app.slack.com/",
            entry: "Work in your Slack?".into(),
            input: Mutex::new(rx),
            stopped: Arc::new(AtomicBool::new(false)),
        };
        let ask = |prompt: &str, purpose| WorkStepKindV1::Ask {
            prompt: prompt.into(),
            options: vec!["Allow".into(), "Not now".into()],
            answer: None,
            purpose: Some(purpose),
        };
        assert_eq!(
            account
                .answer(&ask("Work in your Slack?", WorkAskPurposeV1::Entry))
                .as_deref(),
            Some("Allow")
        );
        assert_eq!(
            account
                .answer(&ask("Work in your Gmail?", WorkAskPurposeV1::Entry))
                .as_deref(),
            Some("Not now")
        );
        assert_eq!(
            account
                .answer(&ask("Work in your Slack?", WorkAskPurposeV1::Confirm))
                .as_deref(),
            Some("Not now")
        );
    }
    #[tokio::test]
    async fn stop_bypasses_full_input_queue_and_a_pending_native_check() {
        let (tx, rx) = mpsc::sync_channel(4);
        let stop = AtomicBool::new(false);
        for _ in 0..8 {
            route_command(b"check", &tx, &stop);
        }
        route_command(b"stop", &tx, &stop);
        assert!(stop.load(Ordering::Acquire));
        assert_eq!(rx.try_iter().count(), 4);
        let result = tokio::time::timeout(
            Duration::from_millis(100),
            unless_stopped(&stop, std::future::pending::<()>()),
        )
        .await;
        assert!(matches!(result, Ok(None)));
    }
}
