//! A complete provisioning with test values, for the tests that start a node.
#![allow(dead_code)]

use std::collections::BTreeMap;

use sw_node::Source;
use sw_node::provision::*;

/// Every path enabled, the server relay last: the shape wrangler.toml provisions.
pub const PATH_POLICY: &str = r#"{"rules":[{"kind":"Direct","enabled":true,"attempt_timeout":{"secs":10,"nanos":0},"cost_weight":1,"classes":15},{"kind":"PeerRelay","enabled":true,"attempt_timeout":{"secs":10,"nanos":0},"cost_weight":2,"classes":15},{"kind":"Turn","enabled":true,"attempt_timeout":{"secs":10,"nanos":0},"cost_weight":3,"classes":15},{"kind":"ServerRelay","enabled":true,"attempt_timeout":{"secs":10,"nanos":0},"cost_weight":4,"classes":15}],"denied":0}"#;

pub struct Vars(pub BTreeMap<&'static str, String>);

impl Source for Vars {
    fn get(&self, name: &str) -> Option<String> {
        self.0.get(name).cloned()
    }
}

/// A complete provisioning with test values (the sign-in issuer is the support world's, seed 0x51).
pub fn complete() -> Vars {
    let issuer = sw_crypto::SigningKey::from_seed(&[0x51; 32]);
    let issuer = data_encoding::BASE64URL_NOPAD.encode(issuer.public_key());
    let mut m = BTreeMap::new();
    for (k, v) in [
        (SECRET_SERVER_KEY_SEED, "11".repeat(32)),
        (SECRET_OPRF_SEED, "22".repeat(32)),
        (VAR_SERVER_KEY_ISSUED_AT, "1794700000".into()),
        (VAR_FORMER_DOMAINS, "-".into()),
        (VAR_CITADEL_WSS, "wss://synchronized.work/citadel".into()),
        (VAR_SIGN_IN_ISSUER_PK, issuer),
        (VAR_ARGON_LANES, "1".into()),
        (VAR_ARGON_MEM_KIB, "4096".into()),
        (VAR_ARGON_TIME_COST, "1".into()),
        (VAR_KSF_MEM_KIB, "19456".into()),
        (VAR_KSF_ITERATIONS, "2".into()),
        (VAR_KSF_LANES, "1".into()),
        (VAR_STUN_SERVERS, "a:3478,b:3478,c:3478".into()),
        (VAR_PATH_POLICY, PATH_POLICY.into()),
        (VAR_PERSON_APPROVAL, "manual".into()),
        (VAR_PERSON_RATE_PER_MIN, "10".into()),
        (sw_node::limits::VAR_INBOX_MAX_SIGNALS, "1000".into()),
        (sw_node::limits::VAR_INBOX_MAX_BYTES, "4194304".into()),
        (sw_node::limits::VAR_MAX_PERSONS, "200".into()),
        (sw_node::limits::VAR_MAX_TASKS, "5000".into()),
        (sw_node::limits::VAR_PROVISIONAL_PER_IP, "64".into()),
        (sw_node::limits::VAR_PROVISIONAL_TOTAL, "256".into()),
        (
            sw_node::limits::VAR_LEGACY_REGISTRATION_HASHES,
            "refused".into(),
        ),
        (
            sw_node::limits::VAR_OUTBOUND_QUEUED_PER_STREAM,
            "4194304".into(),
        ),
        (
            sw_node::limits::VAR_OUTBOUND_QUEUED_TOTAL,
            "33554432".into(),
        ),
    ] {
        m.insert(k, v);
    }
    Vars(m)
}
