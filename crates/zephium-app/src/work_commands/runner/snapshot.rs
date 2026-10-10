//! The child never receives a source grant. It sees only bounded checked copies.
use super::Failure;
use crate::work_files::{protected_path, snapshot_bytes, WorkFileGrant};
use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
};
use tempfile::TempDir;

const MAX_FILES: usize = 2_000;
const MAX_FILE: u64 = 1024 * 1024;
pub(super) const MAX_TOTAL: usize = 32 * 1024 * 1024;
const MAX_DEPTH: usize = 32;

pub(super) struct Snapshot {
    // This owned root contains only broker-authored names. Source paths and Git
    // metadata are never mounted, linked or copied without file-policy checks.
    _directory: TempDir,
    pub root: PathBuf,
    pub work: PathBuf,
    pub home: PathBuf,
    pub temporary: PathBuf,
    pub empty: PathBuf,
    pub files: BTreeMap<PathBuf, Vec<u8>>,
}
impl Snapshot {
    pub fn capture(
        grant: &WorkFileGrant,
        requested: &Path,
        selected: &[PathBuf],
    ) -> Result<Self, Failure> {
        let root = grant
            .resolve(requested.to_str().ok_or(Failure::Input)?, false)
            .map_err(|_| Failure::Input)?;
        if root != requested {
            return Err(Failure::Input);
        }
        let files = collect(&root, selected)?;
        Self::from_files(files)
    }

    fn from_files(files: BTreeMap<PathBuf, Vec<u8>>) -> Result<Self, Failure> {
        let mut builder = tempfile::Builder::new();
        builder.prefix("zephium-work-copy-");
        #[cfg(target_os = "macos")]
        let directory = builder.tempdir_in("/private/tmp");
        #[cfg(not(target_os = "macos"))]
        let directory = builder.tempdir();
        let directory = directory.map_err(|_| Failure::Storage)?;
        let private = directory
            .path()
            .canonicalize()
            .map_err(|_| Failure::Storage)?;
        #[cfg(windows)]
        super::windows::protect_root(&private)?;
        let result = Self {
            _directory: directory,
            work: private.join("work"),
            home: private.join("home"),
            temporary: private.join("tmp"),
            empty: private.join("empty"),
            root: private,
            files,
        };
        for directory in [&result.work, &result.home, &result.temporary, &result.empty] {
            fs::create_dir(directory).map_err(|_| Failure::Storage)?;
        }
        fs::create_dir(result.work.join(".git")).map_err(|_| Failure::Storage)?;
        fs::create_dir(result.work.join(".git/info")).map_err(|_| Failure::Storage)?;
        // Git's highest-priority attributes are broker-owned: copied attribute
        // files are data and cannot transform, hide, or disable checks of edits.
        fs::write(result.work.join(".git/info/attributes"),
            b"* -filter -ident -text -eol -working-tree-encoding diff whitespace=blank-at-eol,blank-at-eof,space-before-tab\n")
            .map_err(|_| Failure::Storage)?;
        for (path, bytes) in &result.files {
            // Create-only catches case/normalization aliases that collide on
            // the destination filesystem instead of silently merging files.
            use std::io::Write;
            let destination = result.work.join(path);
            fs::create_dir_all(destination.parent().ok_or(Failure::Input)?)
                .map_err(|_| Failure::Storage)?;
            let mut options = fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            options
                .open(destination)
                .map_err(|_| Failure::Input)?
                .write_all(bytes)
                .map_err(|_| Failure::Storage)?;
        }
        Ok(result)
    }

    #[cfg(test)]
    pub(super) fn fixture(files: &[(&str, &[u8])]) -> Self {
        Self::from_files(
            files
                .iter()
                .map(|(name, bytes)| (PathBuf::from(name), bytes.to_vec()))
                .collect(),
        )
        .unwrap()
    }

    pub fn write(&self, relative: &Path, bytes: &[u8]) -> Result<(), Failure> {
        if relative.is_absolute()
            || protected_path(relative)
            || !relative
                .components()
                .all(|part| matches!(part, Component::Normal(_)))
            || bytes.len() > MAX_FILE as usize
        {
            return Err(Failure::Input);
        }
        let path = self.work.join(relative);
        fs::create_dir_all(path.parent().ok_or(Failure::Input)?).map_err(|_| Failure::Storage)?;
        fs::write(path, bytes).map_err(|_| Failure::Storage)
    }
}

fn collect(root: &Path, selected: &[PathBuf]) -> Result<BTreeMap<PathBuf, Vec<u8>>, Failure> {
    if selected.len() > MAX_FILES {
        return Err(Failure::Limit);
    }
    let mut files = BTreeMap::new();
    let mut total = 0usize;
    for relative in selected {
        if relative.is_absolute()
            || protected_path(relative)
            || relative.as_os_str().is_empty()
            || !relative
                .components()
                .all(|part| matches!(part, Component::Normal(_)))
        {
            return Err(Failure::Input);
        }
        if relative.components().count() > MAX_DEPTH {
            return Err(Failure::Limit);
        }
        let bytes = snapshot_bytes(&root.join(relative), MAX_FILE).map_err(|_| Failure::Input)?;
        total = total.checked_add(bytes.len()).ok_or(Failure::Limit)?;
        if total > MAX_TOTAL {
            return Err(Failure::Limit);
        }
        if files.insert(relative.to_owned(), bytes).is_some() {
            return Err(Failure::Input);
        }
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn protected_files_aliases_and_links_are_not_staged() {
        let scratch = tempfile::tempdir().unwrap();
        let root = scratch.path().canonicalize().unwrap();
        fs::create_dir(root.join(".git")).unwrap();
        fs::write(root.join(".git/config"), "inert hidden config").unwrap();
        for path in [
            ".env",
            "credentialſ.json",
            "secrets",
            "private.key",
            ".gitconfig",
            ".git-credentials",
        ] {
            fs::write(root.join(path), "inert secret").unwrap();
        }
        fs::write(root.join("ordinary.rs"), "fn main() {}\n").unwrap();
        fs::hard_link(root.join(".env"), root.join("innocent.txt")).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(root.join("ordinary.rs"), root.join("alias.rs")).unwrap();
        let copied = collect(&root, &["ordinary.rs".into()]).unwrap();
        assert_eq!(copied.len(), 1);
        assert_eq!(copied[Path::new("ordinary.rs")], b"fn main() {}\n");
        for relative in [
            ".git/config",
            ".env",
            "credentialſ.json",
            "secrets",
            "private.key",
            ".gitconfig",
            ".git-credentials",
            "innocent.txt",
            "../outside",
        ] {
            assert!(
                matches!(collect(&root, &[relative.into()]), Err(Failure::Input)),
                "{relative}"
            );
        }
        #[cfg(unix)]
        assert!(matches!(
            collect(&root, &["alias.rs".into()]),
            Err(Failure::Input)
        ));
    }
}
