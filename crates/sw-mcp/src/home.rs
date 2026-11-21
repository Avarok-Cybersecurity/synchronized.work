//! The `~/.synchronized/` layout (design §9.4) and the only code that touches it. Everything
//! in it is the owner's alone: on Unix, directories are 0700 and files 0600; on Windows, each
//! has this user as owner and an owner-only, protected (non-inherited) DACL. A write goes to a
//! temporary file that is renamed over the target, so a crash never leaves half a key or half
//! an allowlist behind.

use std::fs::{self, OpenOptions};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use crate::ipc::Endpoint;

#[cfg_attr(unix, path = "home/unix.rs")]
#[cfg_attr(windows, path = "home/windows.rs")]
mod platform;

pub use platform::{PROTECTION, open_private};

#[derive(Debug, Clone)]
pub struct Home {
    root: PathBuf,
    ipc: Endpoint,
}

impl Home {
    /// `explicit` (the `--home` flag) or the documented location: `$HOME/.synchronized` on
    /// Unix, `%LOCALAPPDATA%\synchronized` on Windows (machine-local, never a roaming profile).
    pub fn locate(explicit: Option<PathBuf>) -> io::Result<Self> {
        let root = match explicit {
            Some(dir) => dir,
            None => platform::default_root()?,
        };
        let ipc = Endpoint::for_home(&root)?;
        Ok(Self { root, ipc })
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
    /// The projects this Device created, joined or asked to join (`crate::projects`), 0600.
    pub fn projects(&self) -> PathBuf {
        self.root.join("projects.json")
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
    /// The daemon's IPC endpoint (`crate::ipc::transport`): `serve`, `hook`, `gate`, `trust`
    /// and `revoke` talk to it.
    pub fn ipc(&self) -> &Endpoint {
        &self.ipc
    }

    /// Creates every directory of the layout, owner-only.
    pub fn ensure_layout(&self) -> io::Result<()> {
        for dir in [
            self.root.clone(),
            self.root.join("keys"),
            self.inbox(),
            self.citadel_backend(),
            self.root.join("log"),
        ] {
            platform::create_private_dir(&dir)?;
        }
        Ok(())
    }
}

/// Writes `bytes` to `path`, owner-only, atomically.
pub fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let tmp = path.with_extension("tmp");
    {
        let mut file = open_private(
            OpenOptions::new().write(true).create(true).truncate(true),
            &tmp,
        )?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
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
    platform::check_private(path, &meta)?;
    fs::read(path).map(Some)
}

pub fn remove_if_present(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}
