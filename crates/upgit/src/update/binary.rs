use std::fs;
use std::path::{Path, PathBuf};

use super::UpdateError;

/// In-progress replacement of the running executable.
///
/// The previous file lives at `*.old` until [`BinarySwap::commit`] or
/// [`BinarySwap::restore`].
pub struct BinarySwap {
    current: PathBuf,
    backup: PathBuf,
}

impl BinarySwap {
    pub fn install(current: &Path, new_bin: &Path) -> Result<Self, UpdateError> {
        let backup = sibling(current, ".old");
        let _ = fs::remove_file(&backup);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::copy(current, &backup)?;
            let tmp = sibling(current, ".new");
            fs::copy(new_bin, &tmp)?;
            fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755))?;
            fs::rename(&tmp, current)?;
        }
        #[cfg(windows)]
        {
            fs::rename(current, &backup)?;
            if let Err(e) = fs::copy(new_bin, current) {
                let _ = fs::rename(&backup, current);
                return Err(e.into());
            }
        }
        #[cfg(not(any(unix, windows)))]
        {
            fs::copy(current, &backup)?;
            fs::copy(new_bin, current)?;
        }
        Ok(Self {
            current: current.to_path_buf(),
            backup,
        })
    }

    pub fn restore(self) -> Result<(), UpdateError> {
        #[cfg(windows)]
        {
            let _ = fs::remove_file(&self.current);
        }
        fs::rename(&self.backup, &self.current)?;
        Ok(())
    }

    pub fn commit(self) -> Result<(), UpdateError> {
        let _ = fs::remove_file(&self.backup);
        Ok(())
    }
}

fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_else(|| std::ffi::OsString::from("upgit"));
    name.push(suffix);
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.join(name),
        _ => PathBuf::from(name),
    }
}
