//! Owner-only on Windows: every directory and file gets this user as owner and a protected
//! DACL with one ACE for this user (`sw_winacl`), and a file whose descriptor is any wider
//! (another trustee, an inherited DACL, another owner) is refused.

use std::fs::{self, File, Metadata, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use sw_winacl::Me;
use sw_winacl::policy::Object;

/// How the keystore's files are protected, for the daemon's startup notice.
pub const PROTECTION: &str = "an owner-only, non-inherited ACL on each file and directory";

/// `%LOCALAPPDATA%\synchronized`: machine-local state. Not `%APPDATA%`, which roams to other
/// machines with a domain profile, and Device keys must stay on the Device.
pub(super) fn default_root() -> io::Result<PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA").ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "LOCALAPPDATA is not set; pass --home",
        )
    })?;
    Ok(PathBuf::from(local).join("synchronized"))
}

pub(super) fn create_private_dir(dir: &Path) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    Me::current()?.secure_path(dir, Object::Directory)
}

/// Opens `path` with `options` and makes it owner-only, whether or not it existed.
pub fn open_private(options: &mut OpenOptions, path: &Path) -> io::Result<File> {
    let file = options.open(path)?;
    Me::current()?.secure_path(path, Object::File)?;
    Ok(file)
}

pub(super) fn check_private(path: &Path, _meta: &Metadata) -> io::Result<()> {
    Me::current()?.check_path(path)
}
