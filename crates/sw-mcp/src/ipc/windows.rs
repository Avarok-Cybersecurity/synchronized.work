//! The daemon's endpoint on Windows: a named pipe (`super::transport::pipe_name`) whose owner
//! is this user and whose protected DACL grants only this user, created with
//! FILE_FLAG_FIRST_PIPE_INSTANCE (if any process already holds the name, the daemon refuses to
//! start rather than share it) and PIPE_REJECT_REMOTE_CLIENTS (never reachable over SMB). A
//! client refuses a pipe that is not owner-only for its user, so it never talks to a squatter.

use std::io;
use std::os::windows::io::AsHandle as _;
use std::path::Path;
use std::time::Duration;

use sw_winacl::policy::Object;
use sw_winacl::{Me, SecurityDescriptor};
use tokio::net::windows::named_pipe::{
    ClientOptions, NamedPipeClient, NamedPipeServer, ServerOptions,
};

use super::transport::{Listener, Transport, pipe_name};

/// How long a client waits for a free pipe instance, and how often it looks.
const BUSY_WAIT: Duration = Duration::from_secs(2);
/// The daemon makes the next instance as soon as one connects; a client that finds no
/// instance in this window is told the daemon is not running.
const GAP_WAIT: Duration = Duration::from_millis(100);
const RETRY: Duration = Duration::from_millis(10);

#[derive(Debug, Clone)]
pub struct NamedPipe {
    name: String,
}

impl NamedPipe {
    pub fn for_home(root: &Path) -> io::Result<Self> {
        let me = Me::current()?;
        let root = std::path::absolute(root)?;
        Ok(Self {
            name: pipe_name(me.sid(), &root.to_string_lossy()),
        })
    }
}

/// The options of every instance: local clients only, and the first one alone may create the
/// name.
fn options(first: bool) -> ServerOptions {
    let mut options = ServerOptions::new();
    options
        .first_pipe_instance(first)
        .reject_remote_clients(true)
        .access_inbound(true)
        .access_outbound(true);
    options
}

#[derive(Debug)]
pub struct PipeListener {
    name: String,
    descriptor: SecurityDescriptor,
    next: NamedPipeServer,
}

impl Transport for NamedPipe {
    type Client = NamedPipeClient;
    type Listener = PipeListener;

    fn describe(&self) -> String {
        self.name.clone()
    }

    async fn connect(&self) -> io::Result<NamedPipeClient> {
        let me = Me::current()?;
        let mut waited = Duration::ZERO;
        loop {
            match ClientOptions::new().open(&self.name) {
                Ok(client) => {
                    me.check_handle(&self.name, client.as_handle())?;
                    return Ok(client);
                }
                Err(e) if sw_winacl::is_pipe_busy(&e) && waited < BUSY_WAIT => {}
                Err(e) if e.kind() == io::ErrorKind::NotFound && waited < GAP_WAIT => {}
                Err(e) => return Err(e),
            }
            tokio::time::sleep(RETRY).await;
            waited += RETRY;
        }
    }

    fn bind(&self) -> Result<PipeListener, String> {
        let descriptor = Me::current()
            .and_then(|me| me.descriptor(Object::Pipe))
            .map_err(|e| format!("the pipe's owner-only descriptor: {e}"))?;
        let first = descriptor.create_pipe(&options(true), &self.name).map_err(|e| {
            if sw_winacl::is_access_denied(&e) {
                format!(
                    "another sw-mcp daemon is already running, or another process holds its pipe ({}): {e}",
                    self.name
                )
            } else {
                format!("{}: {e}", self.name)
            }
        })?;
        Ok(PipeListener {
            name: self.name.clone(),
            descriptor,
            next: first,
        })
    }
}

impl Listener for PipeListener {
    type Conn = NamedPipeServer;

    async fn accept(&mut self) -> io::Result<NamedPipeServer> {
        let connected = self.next.connect().await;
        // The next instance exists before this one is handed off, so a client arriving now
        // finds one; a failed instance is replaced, never retried.
        let fresh = self.descriptor.create_pipe(&options(false), &self.name)?;
        let current = std::mem::replace(&mut self.next, fresh);
        connected.map(|()| current)
    }
}
