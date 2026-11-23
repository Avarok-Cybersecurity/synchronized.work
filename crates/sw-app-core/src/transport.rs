//! Which transport the App runs on (design §7.4, `citadel-session-abi` §1): `citadel.session`
//! when granted (the main path); otherwise, or when the open answers `no_identity`, the
//! `net.host` long-poll; with neither, an error with a next step. Never a silent default.
//! After `no_identity`, a long-poll that cannot stand in is the no-identity screen.

use crate::ctx::Granted;
use crate::screen::AppError;

/// The agent's `AppSessionFailure.reason` for a member without a portable identity.
pub const NO_IDENTITY: &str = "no_identity";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    Session,
    Fallback,
}

pub fn is_no_identity(reason: &str) -> bool {
    reason == NO_IDENTITY
}

/// At start.
pub fn choose(granted: &Granted) -> Result<Transport, AppError> {
    match (granted.citadel_session, granted.net_host) {
        (true, _) => Ok(Transport::Session),
        (false, true) => Ok(Transport::Fallback),
        (false, false) => Err(AppError::NoTransport),
    }
}

/// After `SessionOpen` failed with `reason`.
pub fn after_open_failure(reason: &str, granted: &Granted) -> Result<Transport, AppError> {
    match (is_no_identity(reason), granted.net_host) {
        (true, true) => Ok(Transport::Fallback),
        (true, false) => Err(AppError::NoIdentity),
        (false, _) => Err(AppError::OpenFailed {
            reason: reason.to_owned(),
        }),
    }
}
