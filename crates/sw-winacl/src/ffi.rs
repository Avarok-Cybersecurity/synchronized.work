//! The Win32 calls: who this process runs as, the owner and DACL of a path or a handle as SDDL,
//! setting an owner-only descriptor on a path, and creating a named pipe instance with one.
//! The only module of the workspace allowed `unsafe`; every block states why it is sound.
#![allow(unsafe_code)]

use std::ffi::{OsStr, c_void};
use std::io;
use std::os::windows::ffi::OsStrExt as _;
use std::os::windows::io::{AsRawHandle as _, BorrowedHandle};
use std::path::Path;
use std::ptr::{null, null_mut};

use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_ACCESS_DENIED, ERROR_PIPE_BUSY, ERROR_SUCCESS, HANDLE, LocalFree,
    WIN32_ERROR,
};
use windows_sys::Win32::Security::Authorization::{
    ConvertSecurityDescriptorToStringSecurityDescriptorW, ConvertSidToStringSidW,
    ConvertStringSecurityDescriptorToSecurityDescriptorW, GetNamedSecurityInfoW, GetSecurityInfo,
    SDDL_REVISION_1, SE_FILE_OBJECT, SE_KERNEL_OBJECT, SetNamedSecurityInfoW,
};
use windows_sys::Win32::Security::{
    ACL, DACL_SECURITY_INFORMATION, GetSecurityDescriptorDacl, GetSecurityDescriptorOwner,
    GetTokenInformation, OWNER_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION,
    PSECURITY_DESCRIPTOR, PSID, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER, TokenUser,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

const OWNER_AND_DACL: u32 = OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION;

fn wide(s: &OsStr) -> Vec<u16> {
    s.encode_wide().chain(Some(0)).collect()
}

fn win32(code: WIN32_ERROR) -> io::Result<()> {
    match code {
        ERROR_SUCCESS => Ok(()),
        // A WIN32_ERROR is a small positive code; the cast keeps its value.
        code => Err(io::Error::from_raw_os_error(code as i32)),
    }
}

fn ok(result: windows_sys::core::BOOL) -> io::Result<()> {
    if result == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

/// Memory the OS allocated with `LocalAlloc` for this process: freed once, on drop.
struct Local(*mut c_void);

impl Drop for Local {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: the pointer came from an API documented to LocalAlloc its output, and it
            // is freed exactly once, here.
            unsafe { LocalFree(self.0) };
        }
    }
}

/// A NUL-terminated UTF-16 string the OS allocated, as a Rust string.
fn take_wide(p: *mut u16) -> String {
    let _owned = Local(p.cast());
    let mut len = 0;
    // SAFETY: `p` is a NUL-terminated string the API just wrote; it is read up to the NUL
    // and stays allocated until `_owned` drops, after the copy.
    let units = unsafe {
        while *p.add(len) != 0 {
            len += 1;
        }
        std::slice::from_raw_parts(p, len)
    };
    String::from_utf16_lossy(units)
}

/// This process's user SID, as `S-1-5-21-…`.
pub fn current_user_sid() -> io::Result<String> {
    let mut token: HANDLE = null_mut();
    // SAFETY: GetCurrentProcess returns a pseudo-handle; `token` receives a handle we close.
    ok(unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) })?;
    let result = (|| {
        let mut len = 0u32;
        // SAFETY: a size query with a null buffer; only `len` is written.
        unsafe { GetTokenInformation(token, TokenUser, null_mut(), 0, &mut len) };
        if len == 0 {
            return Err(io::Error::last_os_error());
        }
        // u64 storage keeps TOKEN_USER (pointer-aligned) aligned.
        let mut buf = vec![0u64; (len as usize).div_ceil(8)];
        // SAFETY: `buf` holds at least `len` bytes, the size the first call asked for.
        ok(unsafe {
            GetTokenInformation(token, TokenUser, buf.as_mut_ptr().cast(), len, &mut len)
        })?;
        // SAFETY: the call succeeded, so `buf` starts with a TOKEN_USER whose SID points into it.
        let sid = unsafe { (*buf.as_ptr().cast::<TOKEN_USER>()).User.Sid };
        let mut text = null_mut();
        // SAFETY: `sid` is valid while `buf` lives; `text` receives a LocalAlloc'd string.
        ok(unsafe { ConvertSidToStringSidW(sid, &mut text) })?;
        Ok(take_wide(text))
    })();
    // SAFETY: `token` is the handle OpenProcessToken opened, closed once.
    unsafe { CloseHandle(token) };
    result
}

fn to_sddl(sd: PSECURITY_DESCRIPTOR) -> io::Result<String> {
    let mut text = null_mut();
    // SAFETY: `sd` is a valid descriptor its caller owns; `text` receives a LocalAlloc'd string.
    ok(unsafe {
        ConvertSecurityDescriptorToStringSecurityDescriptorW(
            sd,
            SDDL_REVISION_1,
            OWNER_AND_DACL,
            &mut text,
            null_mut(),
        )
    })?;
    Ok(take_wide(text))
}

/// The owner and DACL of a file or directory, as SDDL.
pub fn sddl_of_path(path: &Path) -> io::Result<String> {
    let name = wide(path.as_os_str());
    let mut sd: PSECURITY_DESCRIPTOR = null_mut();
    // SAFETY: `name` is NUL-terminated; only `sd` is requested, and it is LocalAlloc'd.
    win32(unsafe {
        GetNamedSecurityInfoW(
            name.as_ptr(),
            SE_FILE_OBJECT,
            OWNER_AND_DACL,
            null_mut(),
            null_mut(),
            null_mut(),
            null_mut(),
            &mut sd,
        )
    })?;
    let sd = Local(sd);
    to_sddl(sd.0)
}

/// The owner and DACL of an open kernel object (a named pipe), as SDDL.
pub fn sddl_of_handle(handle: BorrowedHandle<'_>) -> io::Result<String> {
    let mut sd: PSECURITY_DESCRIPTOR = null_mut();
    // SAFETY: the handle is open for the borrow's lifetime; `sd` is LocalAlloc'd.
    win32(unsafe {
        GetSecurityInfo(
            handle.as_raw_handle(),
            SE_KERNEL_OBJECT,
            OWNER_AND_DACL,
            null_mut(),
            null_mut(),
            null_mut(),
            null_mut(),
            &mut sd,
        )
    })?;
    let sd = Local(sd);
    to_sddl(sd.0)
}

/// A self-relative security descriptor parsed from SDDL. Immutable once built.
#[derive(Debug)]
pub struct SecurityDescriptor(*mut c_void);

// SAFETY: the descriptor is a private, immutable heap block; reading it from any thread is sound.
unsafe impl Send for SecurityDescriptor {}
// SAFETY: as above: nothing writes it after construction.
unsafe impl Sync for SecurityDescriptor {}

impl Drop for SecurityDescriptor {
    fn drop(&mut self) {
        drop(Local(self.0));
    }
}

impl SecurityDescriptor {
    pub fn from_sddl(sddl: &str) -> io::Result<Self> {
        let text = wide(OsStr::new(sddl));
        let mut sd: PSECURITY_DESCRIPTOR = null_mut();
        // SAFETY: `text` is NUL-terminated; `sd` receives a LocalAlloc'd descriptor.
        ok(unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                text.as_ptr(),
                SDDL_REVISION_1,
                &mut sd,
                null_mut(),
            )
        })?;
        Ok(Self(sd))
    }

    /// The descriptor as Windows prints it back (aliases resolved), owner and DACL.
    pub fn to_sddl(&self) -> io::Result<String> {
        to_sddl(self.0)
    }

    /// Makes this the owner and the protected DACL of `path`, dropping every inherited ACE.
    pub fn apply_to_path(&self, path: &Path) -> io::Result<()> {
        let (mut owner, mut dacl): (PSID, *mut ACL) = (null_mut(), null_mut());
        let (mut present, mut defaulted) = (0, 0);
        // SAFETY: `self.0` is a valid descriptor; the out-pointers point into it.
        ok(unsafe { GetSecurityDescriptorOwner(self.0, &mut owner, &mut defaulted) })?;
        // SAFETY: as above.
        ok(unsafe { GetSecurityDescriptorDacl(self.0, &mut present, &mut dacl, &mut defaulted) })?;
        if present == 0 || dacl.is_null() || owner.is_null() {
            return Err(io::Error::other("the descriptor has no owner or no DACL"));
        }
        let name = wide(path.as_os_str());
        // SAFETY: `name` is NUL-terminated; `owner` and `dacl` live as long as `self`.
        win32(unsafe {
            SetNamedSecurityInfoW(
                name.as_ptr(),
                SE_FILE_OBJECT,
                OWNER_AND_DACL | PROTECTED_DACL_SECURITY_INFORMATION,
                owner,
                null_mut(),
                dacl,
                null(),
            )
        })
    }

    /// One instance of the pipe `name`, created with this descriptor.
    pub fn create_pipe(&self, options: &ServerOptions, name: &str) -> io::Result<NamedPipeServer> {
        let mut attributes = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: self.0,
            bInheritHandle: 0,
        };
        // SAFETY: `attributes` is a valid SECURITY_ATTRIBUTES whose descriptor outlives the
        // call, which is all create_with_security_attributes_raw requires.
        unsafe { options.create_with_security_attributes_raw(name, (&raw mut attributes).cast()) }
    }
}

/// Every instance of the pipe is serving another client: try again shortly.
pub fn is_pipe_busy(e: &io::Error) -> bool {
    e.raw_os_error() == Some(ERROR_PIPE_BUSY as i32)
}

/// What CreateNamedPipe returns when FILE_FLAG_FIRST_PIPE_INSTANCE finds the name taken.
pub fn is_access_denied(e: &io::Error) -> bool {
    e.raw_os_error() == Some(ERROR_ACCESS_DENIED as i32)
}
