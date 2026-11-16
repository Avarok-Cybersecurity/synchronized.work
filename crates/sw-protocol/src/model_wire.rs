//! Wire forms of the `sw-model` types. The integer codes are part of the frozen key table.

use ciborium::value::Value;
use sw_model::readiness::{Blocker, Readiness};
use sw_model::tier::JoinPolicy;
use sw_model::{EndpointKind, Id128, IdFp, Lane, Status, Visibility};

use crate::WireError;
use crate::wire::{MapIn, MapOut, Wire};

/// A fieldless enum carried as a small unsigned code.
macro_rules! wire_code {
    ($t:ty { $($code:literal => $variant:path),* $(,)? }) => {
        impl $crate::wire::Wire for $t {
            fn to_value(&self) -> ciborium::value::Value {
                let code: u8 = match self { $($variant => $code),* };
                $crate::wire::Wire::to_value(&code)
            }
            fn from_value(value: ciborium::value::Value) -> Result<Self, $crate::WireError> {
                match <u8 as $crate::wire::Wire>::from_value(value)? {
                    $($code => Ok($variant),)*
                    _ => Err($crate::WireError::UnknownCode(stringify!($t))),
                }
            }
        }
    };
}
pub(crate) use wire_code;

wire_code!(Lane { 0 => Lane::Running, 1 => Lane::Waiting, 2 => Lane::Blocked, 3 => Lane::Done, 4 => Lane::Stalled });
wire_code!(EndpointKind { 0 => EndpointKind::Device, 1 => EndpointKind::Controller });
wire_code!(Status { 0 => Status::Open, 1 => Status::Done, 2 => Status::Cancelled });
wire_code!(Visibility { 0 => Visibility::Owner, 1 => Visibility::Members, 2 => Visibility::Summary });

impl Wire for IdFp {
    fn to_value(&self) -> Value {
        Value::Text(self.as_str().to_owned())
    }
    fn from_value(value: Value) -> Result<Self, WireError> {
        IdFp::parse(&String::from_value(value)?).map_err(|_| WireError::Range("id_fp"))
    }
}

impl Wire for Id128 {
    fn to_value(&self) -> Value {
        self.0.to_value()
    }
    fn from_value(value: Value) -> Result<Self, WireError> {
        Ok(Id128(<[u8; 16]>::from_value(value)?))
    }
}

impl Wire for Readiness {
    fn to_value(&self) -> Value {
        self.milli().to_value()
    }
    fn from_value(value: Value) -> Result<Self, WireError> {
        Readiness::from_milli(u16::from_value(value)?).ok_or(WireError::Range("readiness"))
    }
}

/// `{0: code, 1: argument}`: lock 0, disk 1, human 2, dependency 3 (task id), rate_limit 4,
/// other 5 (text).
impl Wire for Blocker {
    fn to_value(&self) -> Value {
        let mut out = MapOut::default();
        let code: u8 = match self {
            Blocker::Lock => 0,
            Blocker::Disk => 1,
            Blocker::Human => 2,
            Blocker::Dependency(task) => {
                out.field(1, task);
                3
            }
            Blocker::RateLimit => 4,
            Blocker::Other(text) => {
                out.field(1, text);
                5
            }
        };
        out.field(0, &code);
        out.build()
    }
    fn from_value(value: Value) -> Result<Self, WireError> {
        let mut map = MapIn::new(value)?;
        let blocker = match map.field::<u8>(0)? {
            0 => Blocker::Lock,
            1 => Blocker::Disk,
            2 => Blocker::Human,
            3 => Blocker::Dependency(map.field(1)?),
            4 => Blocker::RateLimit,
            5 => Blocker::Other(map.field(1)?),
            _ => return Err(WireError::UnknownCode("Blocker")),
        };
        map.finish()?;
        Ok(blocker)
    }
}

/// `{0: 0}` manual, `{0: 1, 1: max_coworkers}` auto.
impl Wire for JoinPolicy {
    fn to_value(&self) -> Value {
        let mut out = MapOut::default();
        match self {
            JoinPolicy::Manual => out.field(0, &0u8),
            JoinPolicy::Auto { max_coworkers } => {
                out.field(0, &1u8);
                out.field(1, max_coworkers);
            }
        }
        out.build()
    }
    fn from_value(value: Value) -> Result<Self, WireError> {
        let mut map = MapIn::new(value)?;
        let policy = match map.field::<u8>(0)? {
            0 => JoinPolicy::Manual,
            1 => JoinPolicy::Auto {
                max_coworkers: map.field(1)?,
            },
            _ => return Err(WireError::UnknownCode("JoinPolicy")),
        };
        map.finish()?;
        Ok(policy)
    }
}
