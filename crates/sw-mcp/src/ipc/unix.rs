//! The daemon's endpoint on Unix: a socket at `<home>/daemon.sock`, mode 0600, in the 0700
//! state directory.

use std::future::Future;
use std::io;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use tokio::net::{UnixListener, UnixStream};

use super::transport::{Listener, Transport};

#[derive(Debug, Clone)]
pub struct UnixSocket {
    path: PathBuf,
}

impl UnixSocket {
    pub fn for_home(root: &Path) -> io::Result<Self> {
        Ok(Self {
            path: root.join("daemon.sock"),
        })
    }
}

#[derive(Debug)]
pub struct SocketListener(UnixListener);

impl Transport for UnixSocket {
    type Client = UnixStream;
    type Listener = SocketListener;

    fn describe(&self) -> String {
        self.path.display().to_string()
    }

    fn connect(&self) -> impl Future<Output = io::Result<UnixStream>> + Send {
        UnixStream::connect(self.path.clone())
    }

    fn bind(&self) -> Result<SocketListener, String> {
        let path = &self.path;
        if std::os::unix::net::UnixStream::connect(path).is_ok() {
            return Err(format!(
                "another sw-mcp daemon is already running ({})",
                path.display()
            ));
        }
        crate::home::remove_if_present(path).map_err(|e| e.to_string())?;
        let listener = UnixListener::bind(path).map_err(|e| format!("{}: {e}", path.display()))?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| e.to_string())?;
        Ok(SocketListener(listener))
    }
}

impl Listener for SocketListener {
    type Conn = UnixStream;

    async fn accept(&mut self) -> io::Result<UnixStream> {
        self.0.accept().await.map(|(stream, _)| stream)
    }
}
