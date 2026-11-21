//! Owner-only security on Windows for sw-mcp (the counterpart of mode 0600/0700 on Unix).
//!
//! [`policy`] is the rule, over SDDL text, and builds on every platform so it is tested
//! everywhere. On Windows, [`Me`] applies and checks it: it sets an owner-only, protected DACL
//! on a path, and refuses a path or a pipe whose security descriptor is any wider.

pub mod policy;

#[cfg(windows)]
mod ffi;

#[cfg(windows)]
pub use ffi::{SecurityDescriptor, is_access_denied, is_pipe_busy};

#[cfg(windows)]
mod me {
    use std::io;
    use std::os::windows::io::BorrowedHandle;
    use std::path::Path;

    use crate::ffi::{self, SecurityDescriptor};
    use crate::policy::{self, Object};

    /// The user this process runs as: their SID and the alias, if any, SDDL prints for it.
    #[derive(Debug, Clone)]
    pub struct Me {
        sid: String,
        alias: Option<String>,
    }

    impl Me {
        pub fn current() -> io::Result<Self> {
            let sid = ffi::current_user_sid()?;
            let printed = SecurityDescriptor::from_sddl(&format!("O:{sid}"))?.to_sddl()?;
            let alias = policy::owner_of(&printed)
                .filter(|owner| *owner != sid)
                .map(str::to_owned);
            Ok(Self { sid, alias })
        }

        pub fn sid(&self) -> &str {
            &self.sid
        }

        pub fn descriptor(&self, object: Object) -> io::Result<SecurityDescriptor> {
            SecurityDescriptor::from_sddl(&policy::owner_only_sddl(object, &self.sid))
        }

        /// `Ok` when `sddl` is owner-only for this user, else why not.
        pub fn check(&self, sddl: &str) -> Result<(), String> {
            let mut names = vec![self.sid.as_str()];
            names.extend(self.alias.as_deref());
            policy::check_owner_only(sddl, &names)
        }

        /// Gives `path` this user as owner and an owner-only, protected DACL.
        pub fn secure_path(&self, path: &Path, object: Object) -> io::Result<()> {
            self.descriptor(object)?.apply_to_path(path)
        }

        /// Refuses (`PermissionDenied`) a path whose owner or DACL is wider than owner-only.
        pub fn check_path(&self, path: &Path) -> io::Result<()> {
            let sddl = ffi::sddl_of_path(path)?;
            self.check(&sddl)
                .map_err(|why| denied(path.display(), &why))
        }

        /// Refuses (`PermissionDenied`) an open pipe that is not owner-only for this user.
        pub fn check_handle(&self, what: &str, handle: BorrowedHandle<'_>) -> io::Result<()> {
            let sddl = ffi::sddl_of_handle(handle)?;
            self.check(&sddl).map_err(|why| denied(what, &why))
        }
    }

    fn denied(what: impl std::fmt::Display, why: &str) -> io::Error {
        io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{what} is not owner-only ({why}); expected an owner-only, non-inherited ACL"),
        )
    }
}

#[cfg(windows)]
pub use me::Me;
