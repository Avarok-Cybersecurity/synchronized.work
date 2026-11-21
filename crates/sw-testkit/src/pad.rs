//! A scratchpad member for tests: a Person (stand-in identity), one Device with signing and
//! sealing keys, and that Device's ENROL by assertion from a test issuer — the proof every
//! note and ack carries.

use data_encoding::BASE64URL_NOPAD;
use sw_crypto::{KemSecretKey, SigningKey, kid_of};
use sw_model::{Cid, EndpointKind, IdFp};
use sw_protocol::body::{Audience, Body};
use sw_protocol::payload::{Enrol, EnrolOp, Payload, PubKeys};
use sw_protocol::ports::{MessageSigner, SignInClaims};
use sw_protocol::signed::SwMessage;
use sw_protocol::wire::Bytes;

use crate::StandInIdentity;
use crate::assertion::sign_assertion;

/// The test issuer (work.avarok.net's stand-in) key, from a fixed seed.
pub fn issuer() -> SigningKey {
    SigningKey::from_seed(&[0x5a; 32])
}

#[derive(Debug)]
pub struct PadDevice {
    pub identity: StandInIdentity,
    pub sign: SigningKey,
    pub kem: KemSecretKey,
    pub cid: Cid,
    /// The encoded ENROL-by-assertion `SwMessage`.
    pub proof: Vec<u8>,
}

impl PadDevice {
    /// A Device of the Person `person_seed`, enrolled at `now_ms` with an assertion valid
    /// for an hour after it.
    pub fn new(person_seed: u8, device_seed: u8, cid: Cid, now_ms: u64) -> Self {
        let mut device = Self {
            identity: StandInIdentity::from_seed(&[person_seed; 32]),
            sign: SigningKey::from_seed(&[device_seed; 32]),
            kem: KemSecretKey::from_seed(&[device_seed; 64]),
            cid,
            proof: Vec::new(),
        };
        device.proof = device.enrol_at(now_ms, 255 - device_seed);
        device
    }

    /// This Device's ENROL by assertion as signed at `now_ms` (a re-enrol uses a new nonce).
    pub fn enrol_at(&self, now_ms: u64, nonce: u8) -> Vec<u8> {
        let kid = BASE64URL_NOPAD.encode(&kid_of(self.sign.public_key()));
        let claims = SignInClaims {
            identity: self.identity.public_key().to_vec(),
            device: format!("dev#{kid}"),
            aud: "sw-mcp".into(),
            exp_s: now_ms / 1000 + 3600,
        };
        let assertion = sign_assertion(&issuer(), &claims, &[nonce; 32]);
        let body = Body {
            iss: self.identity.id_fp(),
            from: self.cid,
            aud: Audience::Node,
            iat: now_ms,
            exp: now_ms + 60_000,
            nonce: [nonce; 16],
            seq: now_ms,
            payload: Payload::Enrol(Enrol {
                op: EnrolOp::Enrol,
                epoch: 1,
                endpoint_cid: Some(self.cid),
                kind: Some(EndpointKind::Device),
                pubkeys: Some(PubKeys {
                    sign: Bytes(self.sign.public_key().to_vec()),
                    seal: Some(Bytes(self.kem.encapsulation_key().to_vec())),
                }),
                kid: None,
                assertion: Some(Bytes(assertion)),
            }),
        };
        SwMessage::signed(&body, &self.sign)
            .and_then(|m| m.to_bytes())
            .expect("an ENROL encodes")
    }

    pub fn id_fp(&self) -> IdFp {
        self.identity.id_fp()
    }

    /// Signs `payload` from this Device to the node.
    pub fn signed(&self, payload: Payload, iat: u64, lifetime_ms: u64, nonce: u8) -> Vec<u8> {
        let body = Body {
            iss: self.id_fp(),
            from: self.cid,
            aud: Audience::Node,
            iat,
            exp: iat + lifetime_ms,
            nonce: [nonce; 16],
            seq: iat + u64::from(nonce),
            payload,
        };
        SwMessage::signed(&body, &self.sign)
            .and_then(|m| m.to_bytes())
            .expect("a scratchpad message encodes")
    }
}
