#![cfg(windows)]
#![allow(unsafe_code)] // LogonUser and impersonation: a second user exists only through Win32
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! A process running as ANOTHER local user cannot open a pipe made with the owner-only
//! descriptor; the control, a pipe identical but for a DACL that also grants Everyone, it can.
//!
//! It needs a second local account, so it is `#[ignore]`d and run by CI's Windows job, which
//! creates one and passes it in SW_WINACL_OTHER_USER and SW_WINACL_OTHER_PASSWORD:
//!   cargo test -p sw-winacl --test other_user -- --ignored

use std::ffi::OsStr;
use std::io;
use std::os::windows::ffi::OsStrExt as _;
use std::ptr::null_mut;

use sw_winacl::policy::{Object, owner_only_sddl};
use sw_winacl::{Me, SecurityDescriptor};
use tokio::net::windows::named_pipe::ServerOptions;
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Security::{
    ImpersonateLoggedOnUser, LOGON32_LOGON_NETWORK, LOGON32_PROVIDER_DEFAULT, LogonUserW,
    RevertToSelf,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_GENERIC_READ, FILE_GENERIC_WRITE, OPEN_EXISTING,
};

fn wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(Some(0)).collect()
}

fn var(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} is not set; see this file's docs"))
}

/// Opens the pipe `name` as the other user, on a thread of its own.
fn open_as_other_user(name: String) -> io::Result<()> {
    std::thread::spawn(move || {
        let user = wide(&var("SW_WINACL_OTHER_USER"));
        let password = wide(&var("SW_WINACL_OTHER_PASSWORD"));
        let local = wide(".");
        let mut token: HANDLE = null_mut();
        // SAFETY: NUL-terminated strings; `token` receives a handle closed below.
        let logged_on = unsafe {
            LogonUserW(
                user.as_ptr(),
                local.as_ptr(),
                password.as_ptr(),
                LOGON32_LOGON_NETWORK,
                LOGON32_PROVIDER_DEFAULT,
                &mut token,
            )
        };
        assert_ne!(logged_on, 0, "LogonUser: {}", io::Error::last_os_error());
        // SAFETY: `token` is the logon's token; this thread reverts before it ends.
        assert_ne!(unsafe { ImpersonateLoggedOnUser(token) }, 0);
        let path = wide(&name);
        // SAFETY: NUL-terminated path; the handle, if any, is closed at once.
        let handle = unsafe {
            CreateFileW(
                path.as_ptr(),
                FILE_GENERIC_READ | FILE_GENERIC_WRITE,
                0,
                null_mut(),
                OPEN_EXISTING,
                0,
                null_mut(),
            )
        };
        let result = if handle == INVALID_HANDLE_VALUE {
            Err(io::Error::last_os_error())
        } else {
            // SAFETY: a handle CreateFileW just opened, closed once.
            unsafe { CloseHandle(handle) };
            Ok(())
        };
        // SAFETY: ends this thread's impersonation; closes the token opened above.
        unsafe {
            RevertToSelf();
            CloseHandle(token);
        }
        result
    })
    .join()
    .unwrap()
}

#[test]
#[ignore = "needs a second local account (CI's Windows job creates one)"]
fn another_user_cannot_open_an_owner_only_pipe() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let _entered = runtime.enter();
    let me = Me::current().unwrap();
    let mut options = ServerOptions::new();
    options.first_pipe_instance(true);
    let base = format!(r"\\.\pipe\sw-winacl-other-user-{}", std::process::id());

    let owner_only = format!("{base}-owner-only");
    let _pipe = me
        .descriptor(Object::Pipe)
        .unwrap()
        .create_pipe(&options, &owner_only)
        .unwrap();
    let err = open_as_other_user(owner_only).unwrap_err();
    assert!(sw_winacl::is_access_denied(&err), "{err}");

    // Control: the same pipe but for an ACE granting Everyone; the other user gets in.
    let open = format!("{base}-open");
    let everyone = format!("{}(A;;GA;;;WD)", owner_only_sddl(Object::Pipe, me.sid()));
    let _pipe = SecurityDescriptor::from_sddl(&everyone)
        .unwrap()
        .create_pipe(&options, &open)
        .unwrap();
    open_as_other_user(open).unwrap();
}
