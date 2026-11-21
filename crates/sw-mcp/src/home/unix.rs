//! Owner-only on Unix: directories 0700, files 0600, and a file with any group or other bit
//! set is refused.

use std::fs::{self, DirBuilder, File, Metadata, OpenOptions};
use std::io;
use std::os::unix::fs::{DirBuilderExt as _, OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};

/// How the keystore's files are protected, for the daemon's startup notice.
pub const PROTECTION: &str = "file permissions (0600 files in 0700 directories)";

pub(super) fn default_root() -> io::Result<PathBuf> {
    let home = std::env::var_os("HOME")
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is not set; pass --home"))?;
    Ok(PathBuf::from(home).join(".synchronized"))
}

pub(super) fn create_private_dir(dir: &Path) -> io::Result<()> {
    DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
}

/// Opens `path` with `options`, creating it 0600, and leaves it 0600 even if it existed.
pub fn open_private(options: &mut OpenOptions, path: &Path) -> io::Result<File> {
    let file = options.mode(0o600).open(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(file)
}

pub(super) fn check_private(path: &Path, meta: &Metadata) -> io::Result<()> {
    if meta.permissions().mode() & 0o077 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "{} is readable by others; expected mode 0600",
                path.display()
            ),
        ));
    }
    Ok(())
}
