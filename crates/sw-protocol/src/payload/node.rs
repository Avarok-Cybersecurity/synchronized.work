//! Payloads between an Endpoint and the node (types 11, 13, 14, 15, 16). Text keys.

use sw_model::{Cid, EndpointKind, IdFp};

use crate::WireError;
use crate::wire::{Bytes, Raw, wire_str, wire_text};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnrolOp {
    Enrol,
    Revoke,
}
wire_str!(EnrolOp { "enrol" => EnrolOp::Enrol, "revoke" => EnrolOp::Revoke });

wire_text! {
    pub struct PubKeys {
        /// ML-DSA-65 verifying key, 1952 bytes.
        sign: Bytes,
        /// ML-KEM-1024 encapsulation key, 1568 bytes; a Device's.
        seal: Option<Bytes>,
    }
}

wire_text! {
    /// 11 `ENROL` / `REVOKE`, signed by the identity (root) key. An enrol names
    /// `endpoint_cid`, `kind` and `pubkeys`; a revoke names `kid`. **ENROL by assertion**
    /// (`sign-in-with-citadel.md` §6) carries `assertion` and is signed by the enrolling
    /// Device's own key instead.
    pub struct Enrol {
        op: EnrolOp,
        epoch: u64,
        endpoint_cid: Option<Cid>,
        kind: Option<EndpointKind>,
        pubkeys: Option<PubKeys>,
        kid: Option<[u8; 32]>,
        assertion: Option<Bytes>,
    }
}

impl Enrol {
    pub(crate) fn validate(&self) -> Result<(), WireError> {
        let ok = match self.op {
            EnrolOp::Enrol => match (&self.endpoint_cid, &self.kind, &self.pubkeys, &self.kid) {
                (Some(_), Some(kind), Some(keys), None) => {
                    keys.sign.0.len() == sw_crypto::SIGN_PUBLIC_KEY_LEN
                        && match (&keys.seal, kind) {
                            (Some(seal), EndpointKind::Device) => {
                                seal.0.len() == sw_crypto::KEM_ENCAPSULATION_KEY_LEN
                            }
                            (None, EndpointKind::Controller) => true,
                            _ => false,
                        }
                }
                _ => false,
            },
            EnrolOp::Revoke => {
                self.kid.is_some()
                    && self.endpoint_cid.is_none()
                    && self.kind.is_none()
                    && self.pubkeys.is_none()
                    && self.assertion.is_none()
            }
        };
        if ok {
            Ok(())
        } else {
            Err(WireError::Shape("ENROL/REVOKE fields for its op"))
        }
    }
}

wire_text! {
    /// What an `EVENT` carries: the type's payload after the visibility filter, so kept as a
    /// map rather than re-validated as that type (a filtered `create` may lack its `title`).
    pub struct EventBody {
        typ: u8,
        from: Cid,
        iss: IdFp,
        payload: Raw,
    }
    check = EventBody::check;
}

impl EventBody {
    /// The types the ProjectDO projects into its log.
    pub const TYPES: [u8; 6] = [2, 3, 7, 8, 9, 10];

    fn check(&self) -> Result<(), WireError> {
        if !Self::TYPES.contains(&self.typ) {
            return Err(WireError::UnknownCode("EventBody.typ"));
        }
        self.payload.expect_map()
    }
}

wire_text! {
    /// 13 `EVENT`: node → viewers, the visibility-filtered projection. Unsigned.
    pub struct Event {
        project_id: String,
        cursor: u64,
        kind: String,
        body: EventBody,
    }
}

wire_text! {
    /// 14 `NOTIFY`: a significant event for one Person's Endpoints. Unsigned.
    pub struct Notify {
        event_id: [u8; 32],
        kind: String,
        subject: String,
        text: String,
        at: u64,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscribeOp {
    Subscribe,
    Unsubscribe,
}
wire_str!(SubscribeOp { "subscribe" => SubscribeOp::Subscribe, "unsubscribe" => SubscribeOp::Unsubscribe });

wire_text! {
    /// 15 `SUBSCRIBE` / `UNSUBSCRIBE`. Unsigned (a read).
    pub struct Subscribe {
        op: SubscribeOp,
        project_id: String,
        since_cursor: u64,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkOp {
    Code,
    Redeem,
}
wire_str!(LinkOp { "code" => LinkOp::Code, "redeem" => LinkOp::Redeem });

wire_text! {
    /// 16 `LINK_CODE` (Endpoint-signed) / `LINK_REDEEM` (unsigned, with the App context).
    pub struct Link {
        op: LinkOp,
        code: String,
        acting_username: Option<String>,
        install_id: Option<String>,
        node_id: Option<String>,
    }
}

impl Link {
    pub const CODE_LEN: usize = 8;

    pub(crate) fn validate(&self) -> Result<(), WireError> {
        let context = [&self.acting_username, &self.install_id, &self.node_id];
        let ok = self.code.chars().count() == Self::CODE_LEN
            && match self.op {
                LinkOp::Code => context.iter().all(|f| f.is_none()),
                LinkOp::Redeem => context.iter().all(|f| f.is_some()),
            };
        if ok {
            Ok(())
        } else {
            Err(WireError::Shape("LINK fields for its op"))
        }
    }
}
