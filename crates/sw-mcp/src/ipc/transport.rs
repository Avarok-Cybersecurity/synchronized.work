//! The seam between the daemon's request protocol (`crate::ipc`) and the OS channel it runs
//! over (SBIO): a Unix socket, 0600 (`super::unix`), or a named pipe whose DACL admits only
//! this user (`super::windows`). Never TCP. Every method is required; none has a default.

use std::future::Future;
use std::io;

use data_encoding::HEXLOWER;
use tokio::io::{AsyncRead, AsyncWrite};

/// One connection, either end: a byte stream the protocol writes a line to and reads one from.
pub trait Conn: AsyncRead + AsyncWrite + Send + Unpin + 'static {}

impl<T: AsyncRead + AsyncWrite + Send + Unpin + 'static> Conn for T {}

/// Where the daemon listens and its clients connect.
pub trait Transport: std::fmt::Debug + Clone + Send + Sync + 'static {
    type Client: Conn;
    type Listener: Listener;
    /// The endpoint, for messages: a socket path or a pipe name.
    fn describe(&self) -> String;
    /// Connects to the daemon. Refuses (`PermissionDenied`) an endpoint that is not this
    /// user's own, so a client never hands a request to an impostor.
    fn connect(&self) -> impl Future<Output = io::Result<Self::Client>> + Send;
    /// Takes the endpoint for this daemon. Refuses if another daemon answers on it or, on
    /// Windows, if any other process holds the name.
    fn bind(&self) -> Result<Self::Listener, String>;
}

/// The daemon's side: one accepted connection at a time.
pub trait Listener: std::fmt::Debug + Send + 'static {
    type Conn: Conn;
    fn accept(&mut self) -> impl Future<Output = io::Result<Self::Conn>> + Send;
}

/// The named pipe of one user's daemon over one state directory: `\\.\pipe\sw-mcp-<hex>`, the
/// first 128 bits of SHA3-256 over the user's SID and the directory (lowercased: Windows paths
/// are case-insensitive). Two users, or two `--home`s of one user, never share a pipe. The name
/// is not a secret: the pipe's DACL and FILE_FLAG_FIRST_PIPE_INSTANCE are what protect it.
pub fn pipe_name(user_sid: &str, home_root: &str) -> String {
    let mut input = b"sw-mcp ipc pipe v1\0".to_vec();
    input.extend_from_slice(user_sid.as_bytes());
    input.push(0);
    input.extend_from_slice(home_root.to_lowercase().as_bytes());
    let digest = sw_crypto::sha3_256(&input);
    format!(r"\\.\pipe\sw-mcp-{}", HEXLOWER.encode(&digest[..16]))
}

#[cfg(test)]
mod tests {
    use super::pipe_name;

    const SID: &str = "S-1-5-21-1004336348-1177238915-682003330-1001";

    #[test]
    fn the_pipe_name_is_per_user_and_per_home() {
        let mine = pipe_name(SID, r"C:\Users\ada\AppData\Local\synchronized");
        assert!(mine.starts_with(r"\\.\pipe\sw-mcp-"), "{mine}");
        assert_eq!(mine.len(), r"\\.\pipe\sw-mcp-".len() + 32);
        assert_eq!(
            mine,
            pipe_name(SID, r"c:\users\ADA\appdata\local\synchronized")
        );
        let other_user = pipe_name(
            "S-1-5-21-1004336348-1177238915-682003330-1002",
            r"C:\Users\ada\AppData\Local\synchronized",
        );
        assert_ne!(mine, other_user);
        assert_ne!(mine, pipe_name(SID, r"D:\work\sw-home"));
    }
}
