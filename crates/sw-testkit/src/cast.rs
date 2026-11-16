//! The fixed people and keys every fixture and test uses.

use std::collections::BTreeMap;

use sw_crypto::{KemSecretKey, Kid, SigningKey, kid_of};
use sw_model::{Cid, EndpointKind, IdFp};
use sw_protocol::gate::{KeyDirectory, KeyEntry, KeyRole};
use sw_protocol::ports::MessageSigner;

use crate::StandInIdentity;

/// 2026-11-16T15:20:00Z: the fixtures' "now".
pub const FIXTURE_NOW_MS: u64 = 1_794_842_400_000;

#[derive(Debug)]
pub struct Cast {
    pub alice: StandInIdentity,
    pub alice_controller: SigningKey,
    pub alice_controller_cid: Cid,
    pub alice_device: SigningKey,
    pub alice_device_kem: KemSecretKey,
    pub alice_device_cid: Cid,
    pub bob: StandInIdentity,
    pub bob_controller: SigningKey,
    pub bob_controller_cid: Cid,
}

impl Cast {
    pub fn new() -> Self {
        Self {
            alice: StandInIdentity::from_seed(&[0x11; 32]),
            alice_controller: SigningKey::from_seed(&[0x21; 32]),
            alice_controller_cid: 1001,
            alice_device: SigningKey::from_seed(&[0x31; 32]),
            alice_device_kem: KemSecretKey::from_seed(&[0x41; 64]),
            alice_device_cid: 2001,
            bob: StandInIdentity::from_seed(&[0x12; 32]),
            bob_controller: SigningKey::from_seed(&[0x22; 32]),
            bob_controller_cid: 1002,
        }
    }

    /// The node's view: every key of both people.
    pub fn node_directory(&self) -> Directory {
        let mut d = Directory::default();
        d.add(&self.alice, self.alice.id_fp(), KeyRole::Identity);
        d.add(&self.bob, self.bob.id_fp(), KeyRole::Identity);
        let endpoint = |cid, kind| KeyRole::Endpoint { cid, kind };
        d.add(
            &self.alice_controller,
            self.alice.id_fp(),
            endpoint(self.alice_controller_cid, EndpointKind::Controller),
        );
        d.add(
            &self.alice_device,
            self.alice.id_fp(),
            endpoint(self.alice_device_cid, EndpointKind::Device),
        );
        d.add(
            &self.bob_controller,
            self.bob.id_fp(),
            endpoint(self.bob_controller_cid, EndpointKind::Controller),
        );
        d
    }

    /// Alice's Device's local allowlist: only Alice's paired Controller.
    pub fn device_allowlist(&self) -> Directory {
        let mut d = Directory::default();
        let role = KeyRole::Endpoint {
            cid: self.alice_controller_cid,
            kind: EndpointKind::Controller,
        };
        d.add(&self.alice_controller, self.alice.id_fp(), role);
        d
    }
}

impl Default for Cast {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Default, Clone)]
pub struct Directory(BTreeMap<Kid, KeyEntry>);

impl Directory {
    pub fn add(&mut self, key: &dyn MessageSigner, owner: IdFp, role: KeyRole) {
        let entry = KeyEntry {
            public_key: key.public_key().to_vec(),
            owner,
            role,
        };
        self.0.insert(kid_of(key.public_key()), entry);
    }
}

impl KeyDirectory for Directory {
    fn key(&self, kid: &Kid) -> Option<KeyEntry> {
        self.0.get(kid).cloned()
    }
}
