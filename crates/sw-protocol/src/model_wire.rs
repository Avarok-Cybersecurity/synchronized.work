//! Wire forms of the `sw-model` types that travel as they are (sw-wire contract §3, named
//! types). The model owns each type's rules; this file only spells them on the wire.

use ciborium::value::Value;
use sw_model::readiness::{Blocker, BlockerKind, Readiness};
use sw_model::tier::{JoinPolicy, TierName};
use sw_model::{EndpointKind, IdFp, Lane, Status, Visibility};

use crate::WireError;
use crate::payload::limits;
use crate::wire::{MapIn, MapOut, Wire, wire_str};

wire_str!(EndpointKind { "device" => EndpointKind::Device, "controller" => EndpointKind::Controller });
wire_str!(Lane {
    "running" => Lane::Running, "waiting" => Lane::Waiting, "blocked" => Lane::Blocked,
    "done" => Lane::Done, "stalled" => Lane::Stalled,
});
wire_str!(BlockerKind {
    "human" => BlockerKind::Human, "dependency" => BlockerKind::Dependency,
    "lock" => BlockerKind::Lock, "disk" => BlockerKind::Disk,
    "rate_limit" => BlockerKind::RateLimit,
});
wire_str!(TierName {
    "off" => TierName::Off, "aware" => TierName::Aware, "synchronized" => TierName::Synchronized,
});
wire_str!(Visibility {
    "owner" => Visibility::Owner, "members" => Visibility::Members, "summary" => Visibility::Summary,
});
wire_str!(Status { "open" => Status::Open, "done" => Status::Done, "cancelled" => Status::Cancelled });

/// `tstr(16)`: the issuing Person's `id_fp`.
impl Wire for IdFp {
    fn to_value(&self) -> Value {
        Value::Text(self.as_str().to_owned())
    }
    fn from_value(value: Value) -> Result<Self, WireError> {
        IdFp::parse(&String::from_value(value)?).map_err(|_| WireError::Range("id_fp"))
    }
}

/// `uint 0..=1000`, permille; no floats on the wire.
impl Wire for Readiness {
    fn to_value(&self) -> Value {
        self.permille().to_value()
    }
    fn from_value(value: Value) -> Result<Self, WireError> {
        Readiness::from_permille(u16::from_value(value)?).ok_or(WireError::Range("readiness"))
    }
}

/// `{kind, text ≤ 280 B, task_id? (iff dependency)}`.
impl Wire for Blocker {
    fn to_value(&self) -> Value {
        let mut out = MapOut::default();
        out.field("kind", &self.kind);
        out.field("text", &self.text);
        out.field("task_id", &self.task_id);
        out.build()
    }
    fn from_value(value: Value) -> Result<Self, WireError> {
        let mut map = MapIn::new(value)?;
        let blocker = Blocker {
            kind: map.field("kind")?,
            text: map.field("text")?,
            task_id: map.field("task_id")?,
        };
        map.finish()?;
        if blocker.is_well_formed() {
            Ok(blocker)
        } else {
            Err(WireError::Shape("Blocker text or task_id"))
        }
    }
}

/// `{mode: "manual"} | {mode: "auto", max_coworkers: 1..=64}`.
impl Wire for JoinPolicy {
    fn to_value(&self) -> Value {
        let mut out = MapOut::default();
        match self {
            JoinPolicy::Manual => out.field("mode", &"manual".to_owned()),
            JoinPolicy::Auto { max_coworkers } => {
                out.field("mode", &"auto".to_owned());
                out.field("max_coworkers", max_coworkers);
            }
        }
        out.build()
    }
    fn from_value(value: Value) -> Result<Self, WireError> {
        let mut map = MapIn::new(value)?;
        let mode: String = map.field("mode")?;
        let policy = match mode.as_str() {
            "manual" => JoinPolicy::Manual,
            "auto" => {
                let max_coworkers: u16 = map.field("max_coworkers")?;
                if !(1..=limits::MAX_COWORKERS).contains(&max_coworkers) {
                    return Err(WireError::Range("max_coworkers"));
                }
                JoinPolicy::Auto { max_coworkers }
            }
            _ => return Err(WireError::UnknownCode("JoinPolicy")),
        };
        map.finish()?;
        Ok(policy)
    }
}
