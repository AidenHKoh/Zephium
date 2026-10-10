//! Selected stdlib unittest modules in a read-only text snapshot. No shell,
//! dependency installation, arbitrary interpreter options or project startup hook.
use super::{Failure, Output, Snapshot};
use crate::work_files::{protected_path, read_text_limited, WorkFileGrant};
use std::{
    collections::BTreeSet,
    future::Future,
    path::{Component, Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

pub(super) const BOOTSTRAP: &str = include_str!("python_bootstrap.py");
#[derive(Clone)]
pub(crate) struct UnittestRequest {
    modules: Vec<String>,
    selected: Vec<PathBuf>,
}
impl UnittestRequest {
    pub fn new(tests: &[String], supporting: &[String]) -> Result<Self, Failure> {
        if tests.is_empty() || tests.len() > 64 || tests.len() + supporting.len() > 2_000 {
            return Err(Failure::Input);
        }
        let mut modules = Vec::new();
        let mut selected = BTreeSet::new();
        for test in tests {
            let path = relative(test)?;
            let stem = test.strip_suffix(".py").ok_or(Failure::Input)?;
            if !stem.split('/').all(identifier) {
                return Err(Failure::Input);
            }
            let module = stem
                .strip_suffix("/__init__")
                .unwrap_or(stem)
                .replace('/', ".");
            if modules.contains(&module) {
                return Err(Failure::Input);
            }
            modules.push(module);
            selected.insert(path);
        }
        for path in supporting {
            selected.insert(relative(path)?);
        }
        Ok(Self {
            modules,
            selected: selected.into_iter().collect(),
        })
    }
    pub fn count(&self) -> usize {
        self.modules.len()
    }
}
fn identifier(part: &str) -> bool {
    let mut bytes = part.bytes();
    bytes
        .next()
        .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
        && bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_')
}
fn relative(value: &str) -> Result<PathBuf, Failure> {
    let path = Path::new(value);
    if value.is_empty()
        || value.len() > 1024
        || value.contains('\\')
        || path.is_absolute()
        || protected_path(path)
        || path.components().count() > 32
        || !path
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
    {
        return Err(Failure::Input);
    }
    Ok(path.to_owned())
}
pub(crate) fn ensure_available() -> Result<(), Failure> {
    #[cfg(target_os = "macos")]
    if super::macos::qualified() {
        return Ok(());
    }
    // Windows requires packaged runtime hashes and native LPAC/Job qualification.
    Err(Failure::PythonUnavailable)
}
struct CancelOnDrop(Arc<AtomicBool>);
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}
async fn worker<T, C, CF>(
    mut task: tokio::task::JoinHandle<Result<T, Failure>>,
    flag: &Arc<AtomicBool>,
    cancelled: &C,
) -> Result<T, Failure>
where
    C: Fn() -> CF,
    CF: Future<Output = bool>,
{
    let deadline = tokio::time::sleep(Duration::from_secs(10));
    tokio::pin!(deadline);
    let mut tick = tokio::time::interval(Duration::from_millis(25));
    loop {
        tokio::select! {
            result=&mut task=>return result.map_err(|_|Failure::Failed)?,
            _=&mut deadline=>{flag.store(true,Ordering::Release);return Err(Failure::Limit);}
            _=tick.tick()=>if super::cancellation_requested(cancelled).await {flag.store(true,Ordering::Release);return Err(Failure::Stopped);}
        }
    }
}
pub(crate) async fn run<C, CF>(
    grant: &WorkFileGrant,
    root: &Path,
    request: &UnittestRequest,
    cancelled: C,
) -> Result<Output, Failure>
where
    C: Fn() -> CF,
    CF: Future<Output = bool>,
{
    ensure_available()?;
    if super::cancellation_requested(&cancelled).await {
        return Err(Failure::Stopped);
    }
    let flag = Arc::new(AtomicBool::new(false));
    let _cancel = CancelOnDrop(flag.clone());
    let (grant, root, request) = (grant.clone(), root.to_owned(), request.clone());
    let (input_grant, input_root, input_request, working_flag) =
        (grant.clone(), root.clone(), request.clone(), flag.clone());
    let task = tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "macos")]
        let runtime = super::macos_python::Runtime::qualified()?;
        let resolved = input_grant
            .resolve(input_root.to_str().ok_or(Failure::Input)?, false)
            .map_err(|_| Failure::Input)?;
        if resolved != input_root {
            return Err(Failure::Input);
        }
        let mut files = std::collections::BTreeMap::new();
        let mut total = 0usize;
        for path in &input_request.selected {
            if working_flag.load(Ordering::Acquire) {
                return Err(Failure::Stopped);
            }
            let (_, _, text) =
                read_text_limited(&input_root.join(path), 1024 * 1024).map_err(|error| {
                    if error == crate::work_files::WorkFileError::Sensitive {
                        Failure::Sensitive
                    } else {
                        Failure::Input
                    }
                })?;
            let text = text.ok_or(Failure::Input)?;
            total = total.checked_add(text.len()).ok_or(Failure::Limit)?;
            if total > 32 * 1024 * 1024 {
                return Err(Failure::Limit);
            }
            files.insert(path.clone(), text.into_bytes());
        }
        if working_flag.load(Ordering::Acquire) {
            return Err(Failure::Stopped);
        }
        let snapshot = Snapshot::read_only(files)?;
        #[cfg(target_os = "macos")]
        return Ok((snapshot, runtime));
        #[cfg(not(target_os = "macos"))]
        Ok((snapshot, ()))
    });
    let (snapshot, runtime) = worker(task, &flag, &cancelled).await?;
    #[cfg(target_os = "macos")]
    let output =
        super::macos_python::run(&snapshot, &runtime, &request.modules, &cancelled).await?;
    #[cfg(windows)]
    let output = super::windows::python(&snapshot, &request.modules, &cancelled).await?;
    #[cfg(not(any(target_os = "macos", windows)))]
    let output = {
        let _ = (&runtime, &snapshot);
        return Err(Failure::PythonUnavailable);
    };
    let working_flag = flag.clone();
    let task = tokio::task::spawn_blocking(move || {
        let _ = runtime;
        for (path, bytes) in &snapshot.files {
            if working_flag.load(Ordering::Acquire) {
                return Err(Failure::Stopped);
            }
            if crate::work_files::snapshot_bytes(&root.join(path), 1024 * 1024)
                .ok()
                .as_ref()
                != Some(bytes)
            {
                return Err(Failure::Changed);
            }
        }
        Ok(())
    });
    worker(task, &flag, &cancelled).await?;
    Ok(output)
}

#[cfg(test)]
mod tests;
