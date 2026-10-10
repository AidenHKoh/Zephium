//! Recovery is explicit and approved. Journal discovery never reads OS Trash.
use super::*;
use crate::work_files::trash::{self, PreparedTrash};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
struct CancelOnDrop(Arc<AtomicBool>);
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

fn profile(host: &dyn ComputerHost) -> Result<String, ToolReply> {
    host.recovery_profile()
        .ok_or_else(|| ToolReply::fault("Recovery storage is not available for this run"))
}
impl ComputerTools {
    pub(super) async fn trash(
        &self,
        host: &dyn ComputerHost,
        args: &Value,
    ) -> Result<ToolReply, ToolReply> {
        let profile = profile(host)?;
        let path = self.absolute(text(args, "path")?);
        let file = self
            .files
            .resolve(&path, false)
            .map_err(|error| file_fault(error, &path))?;
        let seen = self
            .seen(&file)
            .ok_or_else(|| ToolReply::fault("Read the file before proposing Trash"))?;
        let prepared = PreparedTrash::prepare(&self.files, &path, &seen)
            .map_err(|error| ToolReply::fault(error.note()))?;
        let id = prepared.id.clone();
        let bytes = prepared.bytes();
        let fact = prepared.fact();
        let digest = fact.before_digest.clone().unwrap_or_default();
        let step = host
            .begin(
                WorkStepKindV1::DeleteFile {
                    path: path.clone(),
                    decision: None,
                },
                Some(fact),
            )
            .await
            .map_err(stopped)?;
        self.recovery_decision(host, step).await?;
        if crate::work_commands::runner::cancellation_requested(&|| host.cancelled()).await {
            return self.recovery_stopped(host, step).await;
        }
        let step_key = step.to_string();
        let cancelled = Arc::new(AtomicBool::new(false));
        let _cancel_on_drop = CancelOnDrop(cancelled.clone());
        let worker_cancel = cancelled.clone();
        let mut worker = tokio::task::spawn_blocking(move || {
            prepared.apply(&profile, &step_key, &worker_cancel)
        });
        let mut tick = tokio::time::interval(Duration::from_millis(25));
        let outcome = loop {
            tokio::select! {
                outcome = &mut worker => break outcome,
                _ = tick.tick() => if crate::work_commands::runner::cancellation_requested(&|| host.cancelled()).await { cancelled.store(true, Ordering::Release); }
            }
        };
        match outcome {
            Ok(Err(trash::Failure::Stopped)) => self.recovery_stopped(host, step).await,
            Ok(Ok(())) => {
                if let Ok(mut reads) = self.reads.lock() {
                    reads.remove(&file);
                }
                let evidence = WorkFileEvidenceV1 {
                    path,
                    name: file
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                    kind: WorkFileKindV1::Deleted,
                    bytes,
                    digest: digest.clone(),
                    text: format!("Moved to Trash. Recovery receipt: {id}"),
                    truncated: false,
                    before_digest: Some(digest),
                    after_digest: None,
                    lines: None,
                };
                self.settle(host, step, Ok(evidence), |_| {
                    format!("Moved to Trash; recovery {id}")
                })
                .await?;
                Ok(ToolReply::ok(format!(
                    "Moved {} to Trash. Recovery receipt: {id}",
                    self.shown(&file)
                )))
            }
            failure => {
                let note = match failure {
                    Ok(Err(error)) => error.note(),
                    _ => "The Trash result is unknown; check private recovery before retrying",
                };
                let note = format!("{note}. Check recoveries for reference {id} before retrying.");
                host.settle(
                    step,
                    WorkStepStatus::Failed,
                    Settled::Nothing,
                    Some(note.clone()),
                )
                .await
                .map_err(stopped)?;
                Err(ToolReply::fault(note))
            }
        }
    }
    pub(super) async fn recoveries(&self, host: &dyn ComputerHost) -> Result<ToolReply, ToolReply> {
        let rows = trash::recoveries(&self.files, &profile(host)?)
            .map_err(|error| ToolReply::fault(error.note()))?;
        let mut text = String::new();
        let total = rows.len();
        let mut shown = 0;
        for row in rows {
            if text.len() + row.path.as_os_str().len() + 160 > 16 * 1024 {
                break;
            }
            shown += 1;
            text.push_str(&format!(
                "{} — {} — {}\n",
                row.id,
                self.shown(&row.path),
                if row.confirmed {
                    "Trash confirmed"
                } else {
                    "interrupted; saved copy available"
                }
            ));
        }
        if shown < total {
            text.push_str(&format!(
                "{} more receipts; narrow the granted folders to inspect them.\n",
                total - shown
            ));
        }
        if text.is_empty() {
            text.push_str("No retained recovery receipts in the current folders.");
        }
        Ok(ToolReply::ok(text))
    }
    pub(super) async fn restore(
        &self,
        host: &dyn ComputerHost,
        args: &Value,
    ) -> Result<ToolReply, ToolReply> {
        let profile = profile(host)?;
        let id = text(args, "receipt")?;
        let (kind, change, file, saved) = trash::restore_proposal(&self.files, &profile, id)
            .map_err(|error| ToolReply::fault(error.note()))?;
        let step = host
            .begin(kind, Some(change.fact()))
            .await
            .map_err(stopped)?;
        self.recovery_decision(host, step).await?;
        if crate::work_commands::runner::cancellation_requested(&|| host.cancelled()).await {
            return self.recovery_stopped(host, step).await;
        }
        let outcome = self
            .files
            .apply(change)
            .map_err(|error| file_fault(error, &self.shown(&file)));
        let (record, _) = self
            .settle(host, step, outcome, |_| {
                format!("Restored saved copy; recovery {id}")
            })
            .await?;
        if let Ok(mut reads) = self.reads.lock() {
            reads.insert(file.clone(), record.digest);
        }
        if let Ok(mut changed) = self.changed.lock() {
            changed.push((
                file.clone(),
                Changed {
                    before: None,
                    after: saved,
                },
            ));
        }
        let marked = trash::restored(&profile, id).is_ok();
        Ok(ToolReply::ok(format!(
            "Restored the saved text to {}.{}",
            self.shown(&file),
            if marked {
                ""
            } else {
                " The recovery receipt could not be updated; the existing file will not be overwritten."
            }
        )))
    }
    async fn recovery_decision(
        &self,
        host: &dyn ComputerHost,
        step: WorkStepId,
    ) -> Result<(), ToolReply> {
        match host.decision(step).await.map_err(stopped)? {
            Decision::Approved => Ok(()),
            Decision::Declined => {
                host.settle(
                    step,
                    WorkStepStatus::Failed,
                    Settled::Nothing,
                    Some("Declined by the person".into()),
                )
                .await
                .map_err(stopped)?;
                Err(ToolReply::fault(
                    "The person declined this change. Do not repeat it unchanged.",
                ))
            }
            Decision::Stopped => {
                let _ = self.recovery_stopped(host, step).await;
                Err(ToolReply::fault("The run stopped"))
            }
        }
    }
    async fn recovery_stopped(
        &self,
        host: &dyn ComputerHost,
        step: WorkStepId,
    ) -> Result<ToolReply, ToolReply> {
        let _ = host
            .settle(
                step,
                WorkStepStatus::Cancelled,
                Settled::Nothing,
                Some("Stopped before the file change".into()),
            )
            .await;
        Err(ToolReply::fault("The run stopped"))
    }
}
