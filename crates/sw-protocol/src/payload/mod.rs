//! The 17 message types (design §4.3) and their payloads.

mod coop;
mod node;
mod signal;
mod status;

pub use coop::{JoinAnswer, JoinRequest, Suggestion, TaskOp, TaskWrite};
pub use node::{Enrol, EnrolOp, Event, Link, LinkOp, Notify, Subscribe, SubscribeOp};
pub use signal::{ControlCode, ControlInner, PromptInner, PromptMode, SealedPayload};
pub use status::{
    Ack, AckResult, Hello, Presence, ProgressEvent, ProgressItem, ProgressTail, StatusUpdate,
};

use ciborium::value::Value;

use crate::WireError;
use crate::wire::Wire;

/// Size limits (design §4.3). The SSOT for every party.
pub mod limits {
    pub const PROGRESS_EVENTS_PER_BATCH: usize = 64;
    pub const PROGRESS_BODY_BYTES: usize = 512;
    pub const TAIL_LINE_BYTES: usize = 512;
    pub const JOIN_NOTE_BYTES: usize = 280;
    pub const SUGGESTION_BYTES: usize = 2048;
    /// Lifetime (`exp - iat`) of a signal (`PROMPT`, `CONTROL`).
    pub const SIGNAL_LIFETIME_MS: u64 = 24 * 60 * 60 * 1000;
    /// Lifetime of every other signed message.
    pub const OTHER_LIFETIME_MS: u64 = 5 * 60 * 1000;
    /// How far in the future an `iat` may be.
    pub const MAX_CLOCK_SKEW_MS: u64 = 60 * 1000;
}

/// Which key must sign a message of a type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignerRole {
    /// The Person's portable identity key (`ENROL` / `REVOKE`).
    Identity,
    Device,
    Controller,
    /// Any enrolled Endpoint of the issuer.
    Endpoint,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Payload {
    Hello(Hello),
    StatusUpdate(StatusUpdate),
    ProgressEvent(ProgressEvent),
    Prompt(SealedPayload),
    Control(SealedPayload),
    Ack(Ack),
    Presence(Presence),
    JoinRequest(JoinRequest),
    JoinAnswer(JoinAnswer),
    TaskWrite(TaskWrite),
    Enrol(Enrol),
    Suggestion(Suggestion),
    Event(Event),
    Notify(Notify),
    Subscribe(Subscribe),
    Link(Link),
    ProgressTail(ProgressTail),
}

macro_rules! dispatch {
    ($($code:literal => $variant:ident($t:ty)),* $(,)?) => {
        impl Payload {
            /// The `typ` code (design §4.3).
            pub fn typ(&self) -> u8 {
                match self { $(Self::$variant(_) => $code),* }
            }
            pub fn to_value(&self) -> Value {
                match self { $(Self::$variant(p) => p.to_value()),* }
            }
            pub fn from_value(typ: u8, value: Value) -> Result<Self, WireError> {
                let payload = match typ {
                    $($code => Self::$variant(<$t>::from_value(value)?),)*
                    _ => return Err(WireError::UnknownCode("typ")),
                };
                payload.check_limits()?;
                Ok(payload)
            }
        }
    };
}

dispatch! {
    1 => Hello(Hello), 2 => StatusUpdate(StatusUpdate), 3 => ProgressEvent(ProgressEvent),
    4 => Prompt(SealedPayload), 5 => Control(SealedPayload), 6 => Ack(Ack),
    7 => Presence(Presence), 8 => JoinRequest(JoinRequest), 9 => JoinAnswer(JoinAnswer),
    10 => TaskWrite(TaskWrite), 11 => Enrol(Enrol), 12 => Suggestion(Suggestion),
    13 => Event(Event), 14 => Notify(Notify), 15 => Subscribe(Subscribe), 16 => Link(Link),
    17 => ProgressTail(ProgressTail),
}

impl Payload {
    /// `None` for the types that travel unsigned (reads, presence, node-originated data,
    /// `LINK_REDEEM`); otherwise the key that must sign. A signed message of an unsigned
    /// type and an unsigned message of a signed type are both refused.
    pub fn signer(&self) -> Option<SignerRole> {
        match self {
            Self::Hello(_) | Self::Presence(_) | Self::Event(_) | Self::Notify(_) => None,
            Self::Subscribe(_) => None,
            Self::Link(link) if link.op == LinkOp::Redeem => None,
            Self::Enrol(_) => Some(SignerRole::Identity),
            Self::StatusUpdate(_) | Self::ProgressEvent(_) | Self::Ack(_) => {
                Some(SignerRole::Device)
            }
            Self::ProgressTail(_) => Some(SignerRole::Device),
            Self::Prompt(_) | Self::Control(_) | Self::JoinAnswer(_) => {
                Some(SignerRole::Controller)
            }
            Self::JoinRequest(_) | Self::TaskWrite(_) | Self::Suggestion(_) | Self::Link(_) => {
                Some(SignerRole::Endpoint)
            }
        }
    }

    /// The longest `exp - iat` allowed (design §4.2).
    pub fn max_lifetime_ms(&self) -> u64 {
        match self {
            Self::Prompt(_) | Self::Control(_) => limits::SIGNAL_LIFETIME_MS,
            _ => limits::OTHER_LIFETIME_MS,
        }
    }

    /// May travel over Citadel P2P (design §4.8).
    pub fn p2p_eligible(&self) -> bool {
        matches!(
            self,
            Self::ProgressEvent(_)
                | Self::Prompt(_)
                | Self::Control(_)
                | Self::Ack(_)
                | Self::ProgressTail(_)
        )
    }

    fn check_limits(&self) -> Result<(), WireError> {
        use limits::*;
        let over = |text: &str, max: usize| text.len() > max;
        let bad = match self {
            Self::ProgressEvent(p) => {
                p.events.len() > PROGRESS_EVENTS_PER_BATCH
                    || p.events.iter().any(|e| over(&e.body, PROGRESS_BODY_BYTES))
            }
            Self::ProgressTail(t) => over(&t.line, TAIL_LINE_BYTES),
            Self::JoinRequest(j) => over(&j.note, JOIN_NOTE_BYTES),
            Self::Suggestion(s) => over(&s.text, SUGGESTION_BYTES),
            Self::TaskWrite(t) => t.embedding.as_ref().is_some_and(|e| e.len() != 256),
            Self::Prompt(s) | Self::Control(s) => s.sealed.check_shape().is_err(),
            _ => false,
        };
        if bad {
            Err(WireError::Length("payload limit"))
        } else {
            Ok(())
        }
    }
}
