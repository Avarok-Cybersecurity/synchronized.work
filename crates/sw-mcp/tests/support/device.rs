//! The Device home and node-side helpers of the daemon end-to-end test.
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use citadel_identity::{MlDsa65Pk, ServerKey, SignInAssertion, WellKnownCitadel, canonical_domain};
use serde_json::Value as Json;
use sw_mcp::config::Config;
use sw_mcp::daemon::now_ms;
use sw_mcp::home::{Home, write_private};
use sw_mcp::ipc::{Request, call_async};
use sw_mcp::keys::DeviceKeys;
use sw_mcp::signin::{IssuerPort, IssuerReply};
use sw_protocol::body::Body;
use sw_protocol::signed::SwMessage;
use sw_testkit::LabelledEntropy;

use super::{HOUR_MS, SIGN_IN_KAT, node};

pub struct PinOnly(Vec<u8>);
impl PinOnly {
    pub fn new(doc: Vec<u8>) -> Self {
        Self(doc)
    }
}
impl IssuerPort for PinOnly {
    async fn well_known(&self) -> Result<Vec<u8>, String> {
        Ok(self.0.clone())
    }
    async fn complete(&self, _: Vec<u8>) -> Result<IssuerReply, String> {
        Err("not used".into())
    }
}

pub fn config(port: u16) -> Config {
    Config::parse(&format!(
        r#"server = "ws://127.0.0.1:{port}/citadel"
device_slug = "e2e"
sign_in_issuer = "work.avarok.net"
approval_mode_new_controller_hours = 24
ack_timeout_s = 600
reconnect_delay_s = 1
stun_servers = ["127.0.0.1:3478", "127.0.0.1:3479", "127.0.0.1:3480"]
log_level = "debug"
harness = "stop_hook"
keystore = "file"
[rate]
per_kid_per_min = 10
pending_max = 50
refusals_per_min_before_pause = 3
"#
    ))
    .unwrap()
}

/// A signed-in, trusting Device home: keys, the pinned KAT issuer, an assertion for these keys,
/// and the KAT Controller trusted two days ago.
pub async fn device_home(kat: &sw_testkit::kat::Kat) -> (Home, DeviceKeys, ServerKey) {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("daemon-e2e");
    let _ = std::fs::remove_dir_all(&dir);
    let home = Home::locate(Some(dir)).unwrap();
    home.ensure_layout().unwrap();
    let keys = DeviceKeys::generate(&mut LabelledEntropy::new("e2e-device"));
    write_private(&home.device_keys(), &keys.to_bytes().unwrap()).unwrap();
    let j: Json = serde_json::from_str(SIGN_IN_KAT).unwrap();
    let seed = data_encoding::HEXLOWER
        .decode(j["issuer"]["key"]["seed_hex"].as_str().unwrap().as_bytes())
        .unwrap();
    let issuer = ServerKey::from_seed(&seed.try_into().unwrap());
    let doc = WellKnownCitadel {
        v: 1,
        canonical_domain: canonical_domain("work.avarok.net").unwrap(),
        citadel_wss: None,
        server_key: issuer.public_key().clone(),
        former_domains: vec![],
        issued_at_s: 1_794_000_000,
    };
    let port = PinOnly(issuer.sign_well_known(&doc).unwrap());
    sw_mcp::signin::pin_issuer(&port, &home, "work.avarok.net")
        .await
        .unwrap();
    let assertion = SignInAssertion {
        identity: MlDsa65Pk::from_bytes(&kat.identity_pk).unwrap(),
        device: keys.device_label("e2e"),
        aud: "sw-mcp".into(),
        exp_s: now_ms() / 1000 + 30 * 86_400,
        nonce: [0x42; 32],
    };
    write_private(
        &home.assertion(),
        &issuer.sign_assertion(&assertion).unwrap(),
    )
    .unwrap();
    let mut list = sw_mcp::trust::TrustList::default();
    let two_days_ago = now_ms() - 48 * HOUR_MS;
    list.trust(
        kat.controller.public_key(),
        kat.controller_cid,
        kat.person.clone(),
        "browser".into(),
        two_days_ago,
    )
    .unwrap();
    sw_mcp::store::save_trust(&home, &keys, &list).unwrap();
    (home, keys, issuer)
}

pub fn signed(m: &[u8]) -> (u8, Body) {
    let SwMessage::Signed { typ, signed } = SwMessage::from_bytes(m).unwrap() else {
        panic!("unsigned")
    };
    (typ, Body::from_bytes(&signed.body.0).unwrap())
}

pub async fn ask(home: &Home, r: Request) -> Json {
    call_async(home.ipc().clone(), r, 0).await.unwrap()
}

/// The next message of `typ` from the device, skipping others (SUBSCRIBE, progress).
pub async fn loop_until_typ(node: &mut node::StandInNode, typ: u8) -> Vec<u8> {
    loop {
        let m = node.next(30).await;
        if let Ok(SwMessage::Signed { typ: t, .. }) = SwMessage::from_bytes(&m)
            && t == typ
        {
            return m;
        }
    }
}
