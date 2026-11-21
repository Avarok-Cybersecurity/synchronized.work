//! The `~/.synchronized/` layout (design §9.4) and the only code that touches it. Directories
//! are 0700 and files 0600; a write goes to a temporary file that is renamed over the target,
//! so a crash never leaves half a key or half an allowlist behind.

use std::fs::{self, DirBuilder, OpenOptions};
use std::io::{self, Write as _};
use std::os::unix::fs::{DirBuilderExt as _, OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Home {
    root: PathBuf,
}

impl Home {
    /// `explicit` (the `--home` flag) or `$HOME/.synchronized`, the documented location.
    pub fn locate(explicit: Option<PathBuf>) -> io::Result<Self> {
        let root = match explicit {
            Some(dir) => dir,
            None => {
                let home = std::env::var_os("HOME").ok_or_else(|| {
                    io::Error::new(io::ErrorKind::NotFound, "HOME is not set; pass --home")
                })?;
                PathBuf::from(home).join(".synchronized")
            }
        };
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn config(&self) -> PathBuf {
        self.root.join("config.toml")
    }
    pub fn device_keys(&self) -> PathBuf {
        self.root.join("keys").join("device.cbor")
    }
    /// The sign-in assertion work.avarok.net issued (contract §1 step 5).
    pub fn assertion(&self) -> PathBuf {
        self.root.join("keys").join("assertion.bin")
    }
    /// The issuer's `/.well-known/citadel` key and `issued_at`, pinned on first use.
    pub fn issuer_pin(&self) -> PathBuf {
        self.root.join("keys").join("issuer-pin.cbor")
    }
    pub fn trusted_controllers(&self) -> PathBuf {
        self.root.join("trusted-controllers.cbor")
    }
    pub fn replay_log(&self) -> PathBuf {
        self.root.join("replay.cbor")
    }
    /// This Device's `ENROL` by assertion, as sent to the node: written once it is sent, and
    /// carried in every scratchpad note and ack as the proof of this Device's keys.
    pub fn enrol_proof(&self) -> PathBuf {
        self.root.join("keys").join("enrol-proof.cbor")
    }
    /// The developer scratchpad on this Device (`crate::pad::state`), 0600.
    pub fn pad_state(&self) -> PathBuf {
        self.root.join("pad.json")
    }
    pub fn inbox(&self) -> PathBuf {
        self.root.join("inbox")
    }
    pub fn citadel_backend(&self) -> PathBuf {
        self.root.join("citadel")
    }
    pub fn log_file(&self) -> PathBuf {
        self.root.join("log").join("sw-mcp.log")
    }
    /// The daemon's Unix socket (0600): `serve`, `hook`, `gate`, `trust` and `revoke` talk to it.
    pub fn socket(&self) -> PathBuf {
        self.root.join("daemon.sock")
    }

    /// Creates every directory of the layout, mode 0700.
    pub fn ensure_layout(&self) -> io::Result<()> {
        for dir in [
            self.root.clone(),
            self.root.join("keys"),
            self.inbox(),
            self.citadel_backend(),
            self.root.join("log"),
        ] {
            DirBuilder::new().recursive(true).mode(0o700).create(&dir)?;
            fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;
        }
        Ok(())
    }
}

/// Writes `bytes` to `path` with mode 0600, atomically.
pub fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let tmp = path.with_extension("tmp");
    {
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600))?;
    fs::rename(&tmp, path)
}

/// The file's bytes, or `None` if it does not exist. Refuses a file anyone but the owner can
/// read or write: a key or an allowlist with loose permissions is not trusted.
pub fn read_private(path: &Path) -> io::Result<Option<Vec<u8>>> {
    let meta = match fs::metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    if meta.permissions().mode() & 0o077 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "{} is readable by others; expected mode 0600",
                path.display()
            ),
        ));
    }
    fs::read(path).map(Some)
}

pub fn remove_if_present(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}
