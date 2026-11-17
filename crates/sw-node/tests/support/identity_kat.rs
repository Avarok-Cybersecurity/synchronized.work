//! The identities of `fixtures/identity-kat.json` (C0), as the root keys that sign `ENROL` and
//! `REVOKE` for an already-known Person. Each signing key is rebuilt from its published `xi`
//! and must have the public key `citadel-identity` derives from the published seed, and the
//! fingerprint the node computes with the same crate, so the tests sign with the contract's
//! keys and the node checks them with the production verifier.

use citadel_identity::Identity;
use serde_json::Value as Json;
use sw_crypto::SigningKey;
use sw_model::IdFp;
use sw_node::identity::CitadelIdentity;
use sw_protocol::ports::IdentityPort;

const KAT: &str = include_str!("../../../../fixtures/identity-kat.json");

pub struct RootKey {
    pub key: SigningKey,
    pub id_fp: IdFp,
}

fn bytes32(j: &Json, name: &str, field: &str) -> [u8; 32] {
    let hex = j["identities"][name][field]
        .as_str()
        .unwrap_or_else(|| panic!("identity kat: {name}.{field}"));
    let bytes = data_encoding::HEXLOWER.decode(hex.as_bytes()).unwrap();
    bytes.try_into().unwrap()
}

/// Identity `name` (`"A"` or `"B"`) of the KAT.
pub fn root_key(name: &str) -> RootKey {
    let j: Json = serde_json::from_str(KAT).unwrap();
    let derived = Identity::from_seed(&bytes32(&j, name, "seed_hex"));
    let key = SigningKey::from_seed(&bytes32(&j, name, "xi_hex"));
    assert_eq!(
        key.public_key(),
        derived.pk(),
        "identity kat {name}: xi and seed disagree"
    );
    let published = j["identities"][name]["pk_b64url"].as_str().unwrap();
    assert_eq!(derived.public_key().to_base64url(), published);
    let id_fp = CitadelIdentity.id_fp(key.public_key()).unwrap();
    assert_eq!(
        id_fp.as_str(),
        j["identities"][name]["id_fp"].as_str().unwrap()
    );
    assert_eq!(id_fp.as_str(), derived.id_fp());
    RootKey { key, id_fp }
}
