//! The worker owns the entire copied workspace, including acknowledged cleanup.
//! Granted folders are read-only inputs. Cancellation is checked before each
//! child admission; no worker publishes changes back to a granted folder.
use super::worker_slots::{Permit, Slots};
use super::{ApprovedChange, Failure, GitOperation, Output, ReadOperation, Snapshot};
use crate::work_files::WorkFileGrant;
use std::{
    future::Future,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, OnceLock,
    },
    time::{Duration, Instant},
};
const FILESYSTEM_LIMIT: Duration = Duration::from_secs(10);
const REVIEW_LIMIT: Duration = Duration::from_secs(90);
const CLEANUP_GRACE: Duration = Duration::from_millis(300);
fn slots() -> &'static Arc<Slots> {
    static SLOTS: OnceLock<Arc<Slots>> = OnceLock::new();
    SLOTS.get_or_init(|| Arc::new(Slots::new(4)))
}
fn launch_owned(
    permit: Permit,
    work: impl FnOnce() -> Result<Output, Failure> + Send + 'static,
) -> tokio::task::JoinHandle<Result<Output, Failure>> {
    super::worker_slots::launch(permit, work, |result| {
        !matches!(
            result,
            Err(Failure::CleanupPending | Failure::StopUnconfirmed)
        )
    })
}

struct Control {
    cancelled: AtomicBool,
    started: Instant,
    filesystem_deadline: AtomicU64,
    filesystem_limit: Duration,
}
impl Control {
    fn new(limit: Duration) -> Self {
        Self {
            cancelled: AtomicBool::new(false),
            started: Instant::now(),
            filesystem_deadline: AtomicU64::new(limit.as_millis() as u64),
            filesystem_limit: limit,
        }
    }
    fn filesystem(&self) {
        self.filesystem_deadline.store(
            self.started.elapsed().as_millis() as u64 + self.filesystem_limit.as_millis() as u64,
            Ordering::Release,
        );
    }
    fn native(&self) {
        self.filesystem_deadline.store(0, Ordering::Release);
    }
    fn check(&self) -> Result<(), Failure> {
        if self.cancelled.load(Ordering::Acquire) {
            return Err(Failure::Stopped);
        }
        let deadline = self.filesystem_deadline.load(Ordering::Acquire);
        if deadline != 0 && self.started.elapsed().as_millis() >= deadline as u128 {
            return Err(Failure::Limit);
        }
        Ok(())
    }
}
struct CancelOnDrop(Arc<Control>);
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.cancelled.store(true, Ordering::Release);
    }
}

pub(super) async fn run<C, CF>(
    grant: &WorkFileGrant,
    root: &Path,
    changes: &[ApprovedChange],
    operation: ReadOperation,
    cancelled: C,
) -> Result<Output, Failure>
where
    C: Fn() -> CF,
    CF: Future<Output = bool>,
{
    super::ensure_available()?;
    if changes.is_empty() || changes.len() > 2_000 {
        return Err(Failure::Input);
    }
    if super::cancellation_requested(&cancelled).await {
        return Err(Failure::Stopped);
    }
    let permit = slots().reserve().map_err(|_| Failure::ReviewBusy)?;
    let control = Arc::new(Control::new(FILESYSTEM_LIMIT));
    let _cancel = CancelOnDrop(control.clone());
    let (grant, root, changes) = (grant.clone(), root.to_owned(), changes.to_vec());
    let worker_control = control.clone();
    let runtime = tokio::runtime::Handle::current();
    let worker = launch_owned(permit, move || {
        runtime.block_on(owned(grant, root, changes, operation, &worker_control))
    });
    await_worker(worker, &control, &cancelled, REVIEW_LIMIT, CLEANUP_GRACE).await
}
async fn owned(
    grant: WorkFileGrant,
    root: PathBuf,
    changes: Vec<ApprovedChange>,
    operation: ReadOperation,
    control: &Control,
) -> Result<Output, Failure> {
    control.check()?;
    let selected: Vec<_> = changes.iter().map(|change| change.path.clone()).collect();
    let snapshot = Snapshot::capture_checked(&grant, &root, &selected, &|| control.check())?;
    let result = inspect(&snapshot, &root, &changes, operation, control).await;
    control.filesystem();
    // Explicit close is the acknowledgement. Best-effort Drop is never used
    // as evidence that cancellation finished removing the private copy.
    let closed = snapshot.close();
    match (result, closed) {
        (Err(Failure::StopUnconfirmed), _) => Err(Failure::StopUnconfirmed),
        (_, Err(_)) => Err(Failure::CleanupPending),
        (result, Ok(())) => result,
    }
}
async fn inspect(
    snapshot: &Snapshot,
    root: &Path,
    changes: &[ApprovedChange],
    operation: ReadOperation,
    control: &Control,
) -> Result<Output, Failure> {
    super::prepare_baseline_checked(snapshot, changes, &|| control.check())?;
    for operation in [
        GitOperation::Initialize,
        GitOperation::CaptureIndex,
        GitOperation::CommitBaseline,
    ] {
        control.check()?;
        control.native();
        if !super::execute(snapshot, operation, &|| async {
            control.cancelled.load(Ordering::Acquire)
        })
        .await?
        .success
        {
            return Err(Failure::Failed);
        }
    }
    control.filesystem();
    for change in changes {
        control.check()?;
        snapshot.write(&change.path, &change.after)?;
    }
    control.check()?;
    control.native();
    if !super::execute(snapshot, GitOperation::IntentToAdd, &|| async {
        control.cancelled.load(Ordering::Acquire)
    })
    .await?
    .success
    {
        return Err(Failure::Failed);
    }
    control.check()?;
    let output = super::execute(snapshot, GitOperation::Read(operation), &|| async {
        control.cancelled.load(Ordering::Acquire)
    })
    .await?;
    control.filesystem();
    for change in changes {
        control.check()?;
        if crate::work_files::snapshot_bytes(&root.join(&change.path), 1024 * 1024)
            .ok()
            .as_ref()
            != Some(&change.after)
        {
            return Err(Failure::Changed);
        }
    }
    control.check()?;
    Ok(output)
}
async fn await_worker<C, CF>(
    mut worker: tokio::task::JoinHandle<Result<Output, Failure>>,
    control: &Control,
    cancelled: &C,
    limit: Duration,
    grace: Duration,
) -> Result<Output, Failure>
where
    C: Fn() -> CF,
    CF: Future<Output = bool>,
{
    let deadline = tokio::time::sleep(limit);
    tokio::pin!(deadline);
    let mut tick = tokio::time::interval(Duration::from_millis(25));
    let reason = loop {
        tokio::select! {
            result=&mut worker=>return result.unwrap_or(Err(Failure::CleanupPending)),
            _=&mut deadline=>break Failure::Limit,
            _=tick.tick()=>{
                if super::cancellation_requested(cancelled).await{break Failure::Stopped}
                if let Err(error)=control.check(){break error}
            }
        }
    };
    control.cancelled.store(true, Ordering::Release);
    match tokio::time::timeout(grace, &mut worker).await {
        Ok(Ok(Ok(_))) => Err(reason),
        Ok(Ok(Err(Failure::Stopped))) => Err(reason),
        Ok(Ok(Err(error))) => Err(error),
        Ok(Err(_)) | Err(_) => Err(Failure::CleanupPending),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    fn output() -> Output {
        Output {
            text: String::new(),
            success: true,
            exit: 0,
        }
    }
    #[tokio::test]
    async fn cancellation_waits_for_worker_cleanup_before_reporting_stopped() {
        let control = Arc::new(Control::new(Duration::from_secs(2)));
        let current = control.clone();
        let cleaned = Arc::new(AtomicBool::new(false));
        let ack = cleaned.clone();
        let worker = tokio::task::spawn_blocking(move || {
            let snapshot = Snapshot::fixture(&[("inert.txt", b"inert")]);
            while !current.cancelled.load(Ordering::Acquire) {
                std::thread::sleep(Duration::from_millis(2));
            }
            snapshot.close()?;
            ack.store(true, Ordering::Release);
            Err(Failure::Stopped)
        });
        let result = await_worker(
            worker,
            &control,
            &|| async { true },
            Duration::from_secs(2),
            CLEANUP_GRACE,
        )
        .await;
        assert!(matches!(result, Err(Failure::Stopped)));
        assert!(cleaned.load(Ordering::Acquire));
    }
    #[tokio::test]
    async fn stalled_prelaunch_filesystem_work_is_pending_then_cancels_without_a_child() {
        let control = Arc::new(Control::new(Duration::from_millis(20)));
        let current = control.clone();
        let launches = Arc::new(AtomicUsize::new(0));
        let launched = launches.clone();
        let cleaned = Arc::new(AtomicBool::new(false));
        let ack = cleaned.clone();
        let slots = Arc::new(Slots::new(1));
        let worker = launch_owned(slots.reserve().unwrap(), move || {
            let snapshot = Snapshot::fixture(&[("inert.txt", b"inert")]);
            // An injected blocking read: no source path outside this owned fixture.
            std::thread::sleep(Duration::from_millis(180));
            let result = current.check().map(|_| {
                launched.fetch_add(1, Ordering::SeqCst);
                output()
            });
            snapshot.close()?;
            ack.store(true, Ordering::Release);
            result
        });
        let began = Instant::now();
        let result = await_worker(
            worker,
            &control,
            &|| async { false },
            Duration::from_secs(2),
            Duration::from_millis(20),
        )
        .await;
        assert!(matches!(result, Err(Failure::CleanupPending)));
        assert!(began.elapsed() < Duration::from_millis(150));
        assert!(!cleaned.load(Ordering::Acquire));
        assert!(slots.reserve().is_err());
        tokio::time::timeout(Duration::from_secs(2), async {
            while !cleaned.load(Ordering::Acquire) || slots.outstanding() != 0 {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(launches.load(Ordering::Acquire), 0);
        assert!(slots.reserve().is_ok());
    }
    #[tokio::test]
    async fn deadline_keeps_limit_reason_after_worker_acknowledges_stop() {
        let control = Arc::new(Control::new(Duration::from_millis(20)));
        let current = control.clone();
        let worker = tokio::task::spawn_blocking(move || {
            let snapshot = Snapshot::fixture(&[("inert.txt", b"inert")]);
            while !current.cancelled.load(Ordering::Acquire) {
                std::thread::sleep(Duration::from_millis(2));
            }
            snapshot.close()?;
            Err(Failure::Stopped)
        });
        assert!(matches!(
            await_worker(
                worker,
                &control,
                &|| async { false },
                Duration::from_secs(2),
                CLEANUP_GRACE
            )
            .await,
            Err(Failure::Limit)
        ));
    }
    #[tokio::test]
    async fn unconfirmed_cleanup_keeps_capacity_reserved_after_worker_returns() {
        let slots = Arc::new(Slots::new(1));
        let worker = launch_owned(slots.reserve().unwrap(), || Err(Failure::CleanupPending));
        assert!(matches!(
            worker.await.unwrap(),
            Err(Failure::CleanupPending)
        ));
        assert!(slots.reserve().is_err());
    }
    #[tokio::test]
    async fn failed_cleanup_acknowledgement_is_never_reported_as_stopped() {
        let control = Arc::new(Control::new(Duration::from_secs(2)));
        let worker = tokio::task::spawn_blocking(|| Err(Failure::CleanupPending));
        assert!(matches!(
            await_worker(
                worker,
                &control,
                &|| async { true },
                Duration::from_secs(2),
                CLEANUP_GRACE
            )
            .await,
            Err(Failure::CleanupPending)
        ));
    }
}
