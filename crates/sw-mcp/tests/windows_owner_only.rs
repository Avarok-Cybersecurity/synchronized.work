#![cfg(windows)]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! Windows owner-only, through the production code: the daemon's named pipe refuses a remote
//! client and refuses to share its name; a client refuses a pipe that is not its user's alone;
//! a key file whose ACL is widened, or made to inherit, is refused. Each has its control: the
//! same step against an object without the protection succeeds, so the test can tell.

use std::os::windows::io::AsHandle as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use sw_mcp::home::{Home, read_private, write_private};
use sw_mcp::ipc::{Request, Transport as _, call_async};
use sw_winacl::Me;
use sw_winacl::policy::{Object, owner_only_sddl};
use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeServer, ServerOptions};

fn home(name: &str) -> Home {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    let home = Home::locate(Some(dir)).unwrap();
    home.ensure_layout().unwrap();
    home
}

/// A pipe some other program made with Windows' default security, as a squatter would.
fn default_pipe(name: &str, first: bool) -> std::io::Result<NamedPipeServer> {
    ServerOptions::new().first_pipe_instance(first).create(name)
}

/// The same pipe name, reached through the SMB redirector: what a remote client opens.
fn via_smb(name: &str) -> String {
    name.replacen(r"\\.\", r"\\localhost\", 1)
}

#[tokio::test]
async fn a_remote_client_is_refused() {
    let home = home("win-remote");
    let name = home.ipc().describe();
    let _listener = home.ipc().bind().unwrap();
    // PIPE_REJECT_REMOTE_CLIENTS: the server end refuses it (Windows answers access denied).
    let refused = ClientOptions::new().open(via_smb(&name));
    assert!(
        refused.is_err(),
        "a client over SMB reached the daemon's pipe"
    );
    // Control: an owner-only pipe that accepts remote clients is reachable the same way.
    let open = format!("{name}-control");
    let me = Me::current().unwrap();
    let mut options = ServerOptions::new();
    options.reject_remote_clients(false);
    let _control = me
        .descriptor(Object::Pipe)
        .unwrap()
        .create_pipe(&options, &open)
        .unwrap();
    ClientOptions::new().open(via_smb(&open)).unwrap();
}

#[tokio::test]
async fn a_name_another_process_holds_stops_the_daemon() {
    let home = home("win-squat");
    let name = home.ipc().describe();
    let squatter = default_pipe(&name, true).unwrap();
    let err = home.ipc().bind().unwrap_err();
    assert!(err.contains("another process holds"), "{err}");
    // Control: with the name free, the daemon takes it, and then a second instance is refused.
    drop(squatter);
    let _daemon = home.ipc().bind().unwrap();
    assert!(default_pipe(&name, true).is_err());
}

#[tokio::test]
async fn a_client_refuses_a_pipe_that_is_not_its_users_alone() {
    let home = home("win-impostor");
    let name = home.ipc().describe();
    let _impostor = default_pipe(&name, true).unwrap();
    let err = call_async(home.ipc().clone(), Request::GateList, 0)
        .await
        .unwrap_err();
    assert!(err.contains("not owner-only"), "{err}");
}

#[tokio::test]
async fn the_daemons_pipe_admits_only_this_user() {
    let home = home("win-pipe-dacl");
    let name = home.ipc().describe();
    let _listener = home.ipc().bind().unwrap();
    let me = Me::current().unwrap();
    let client = ClientOptions::new().open(&name).unwrap();
    me.check_handle(&name, client.as_handle()).unwrap();
    // Control: Windows' default pipe security (Everyone may read) fails the same check.
    let other = format!("{name}-default");
    let _default = default_pipe(&other, true).unwrap();
    let client = ClientOptions::new().open(&other).unwrap();
    assert!(me.check_handle(&other, client.as_handle()).is_err());
    assert_eq!(me.check(&owner_only_sddl(Object::Pipe, me.sid())), Ok(()));
}

fn icacls(path: &Path, args: &[&str]) {
    let out = Command::new("icacls")
        .arg(path)
        .args(args)
        .output()
        .unwrap();
    assert!(out.status.success(), "icacls {args:?}: {out:?}");
}

#[test]
fn a_key_file_with_a_widened_acl_is_refused() {
    let home = home("win-key-widened");
    let key = home.device_keys();
    write_private(&key, b"key").unwrap();
    // Control: as written, it is owner-only and reads back.
    Me::current().unwrap().check_path(&key).unwrap();
    assert_eq!(read_private(&key).unwrap().as_deref(), Some(&b"key"[..]));
    icacls(&key, &["/grant", "*S-1-1-0:(R)"]);
    let err = read_private(&key).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied, "{err}");
    assert!(
        err.to_string().contains("S-1-1-0") || err.to_string().contains("WD"),
        "{err}"
    );
}

#[test]
fn a_key_file_that_inherits_its_acl_is_refused() {
    let home = home("win-key-inherits");
    let key = home.device_keys();
    write_private(&key, b"key").unwrap();
    assert!(read_private(&key).unwrap().is_some());
    icacls(&key, &["/inheritance:e"]);
    let err = read_private(&key).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied, "{err}");
    // Rewriting it makes it owner-only again.
    write_private(&key, b"key").unwrap();
    assert!(read_private(&key).unwrap().is_some());
}

#[test]
fn the_state_directory_is_owner_only_and_not_inherited() {
    let home = home("win-layout");
    let me = Me::current().unwrap();
    for dir in [home.root().to_path_buf(), home.root().join("keys")] {
        me.check_path(&dir).unwrap();
    }
}
