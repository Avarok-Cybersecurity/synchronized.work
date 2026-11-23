//! Why `SessionOpen` was refused, as the member is told it. The reason is the agent's
//! `AppSessionFailure.reason` name, or the binding's form of the host's `Refused`
//! (`ungranted`, `limit: ..`, `policy: ..`, `remote: ..`, or a host reason as it is).
//! `no_identity` never comes here: `transport::after_open_failure` turns it into the
//! fallback or the no-identity screen.

use crate::copy;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenRefusal {
    /// The workspace did not grant `citadel.session` to this server.
    Ungranted,
    /// The host's policy (egress allow-list, scope, the acting user's permission).
    Policy,
    Limit,
    IdentityLocked,
    /// The agent runs without app egress or without an identity.
    NotConfigured,
    /// The window is not attached to the member session it named.
    SignedOut,
    /// The workspace server did not say what the install may reach.
    Authority,
    /// Allowed, and synchronized.work could not be reached.
    Unreachable,
    /// A request the agent could not accept (a defect, shown rather than hidden).
    Invalid,
    /// A reason this App does not know; it is shown as a detail.
    Other,
}

impl OpenRefusal {
    pub fn of(reason: &str) -> Self {
        let kind = reason.split(':').next().map_or(reason, str::trim);
        match kind {
            "ungranted" => Self::Ungranted,
            "policy" => Self::Policy,
            "limit" => Self::Limit,
            "identity_locked" => Self::IdentityLocked,
            "not_configured" => Self::NotConfigured,
            "session" => Self::SignedOut,
            "authority" => Self::Authority,
            "unavailable" | "remote" => Self::Unreachable,
            "invalid" => Self::Invalid,
            _ => Self::Other,
        }
    }

    /// What happened, then what to do.
    pub fn lines(self) -> &'static [&'static str] {
        match self {
            Self::Ungranted => copy::REFUSED_UNGRANTED,
            Self::Policy => copy::REFUSED_POLICY,
            Self::Limit => copy::REFUSED_LIMIT,
            Self::IdentityLocked => copy::REFUSED_IDENTITY_LOCKED,
            Self::NotConfigured => copy::REFUSED_NOT_CONFIGURED,
            Self::SignedOut => copy::REFUSED_SIGNED_OUT,
            Self::Authority => copy::REFUSED_AUTHORITY,
            Self::Unreachable => copy::REFUSED_UNREACHABLE,
            Self::Invalid => copy::REFUSED_INVALID,
            Self::Other => copy::REFUSED_OTHER,
        }
    }
}
