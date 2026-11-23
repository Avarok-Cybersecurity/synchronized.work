//! A signed-in sw-mcp home for a test Person: fresh Device keys, the KAT issuer pinned, an
//! assertion for those keys signed by it, and a config.toml naming the loopback node.

use citadel_identity::{Identity, MlDsa65Pk, SignInAssertion, WellKnownCitadel, canonical_domain};
use sw_mcp::adapters::{CitadelIdentity, OsEntropy};
use sw_mcp::daemon::now_ms;
use sw_mcp::home::{Home, write_private};
use sw_mcp::keys::DeviceKeys;
use sw_mcp::signin::{IssuerPort, IssuerReply};
use sw_protocol::ports::IdentityPort as _;

/// The issuer's canonical domain the home pins: a name only, never reached.
const ISSUER: &str = "work.avarok.net";

/// Hands the pin its document; a test home never completes a sign-in over the network.
struct Pinned(Vec<u8>);

impl IssuerPort for Pinned {
    async fn well_known(&self) -> Result<Vec<u8>, String> {
        Ok(self.0.clone())
    }
    async fn complete(&self, _: Vec<u8>) -> Result<IssuerReply, String> {
        Err("a test home does not sign in over the network".into())
    }
}

fn config(port: u16) -> String {
    format!(
        r#"server = "ws://127.0.0.1:{port}/citadel"
device_slug = "dev"
sign_in_issuer = "{ISSUER}"
approval_mode_new_controller_hours = 24
ack_timeout_s = 600
reconnect_delay_s = 1
stun_servers = [{stun}]
log_level = "info"
harness = "stop_hook"
keystore = "file"
[rate]
per_kid_per_min = 10
pending_max = 50
refusals_per_min_before_pause = 3
"#,
        stun = super::serve::LOOPBACK_STUN
            .split(',')
            .map(|s| format!("{s:?}"))
            .collect::<Vec<_>>()
            .join(", "),
    )
}

pub fn write(dir: &str, port: u16, seed: &str) -> Result<String, String> {
    let e = |e: &dyn std::fmt::Display| e.to_string();
    let identity = Identity::from_seed(&super::seed32(seed)?);
    let home = Home::locate(Some(dir.into())).map_err(|x| e(&x))?;
    home.ensure_layout().map_err(|x| e(&x))?;
    write_private(&home.config(), config(port).as_bytes()).map_err(|x| e(&x))?;
    let keys = DeviceKeys::generate(&mut OsEntropy);
    write_private(
        &home.device_keys(),
        &keys.to_bytes().map_err(|x| format!("{x:?}"))?,
    )
    .map_err(|x| e(&x))?;
    let issuer = super::kat_issuer()?;
    let doc = WellKnownCitadel {
        v: 1,
        canonical_domain: canonical_domain(ISSUER).map_err(|x| e(&x))?,
        citadel_wss: None,
        server_key: issuer.public_key().clone(),
        former_domains: vec![],
        issued_at_s: now_ms() / 1000,
    };
    let pinned = Pinned(issuer.sign_well_known(&doc).map_err(|x| format!("{x:?}"))?);
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .map_err(|x| e(&x))?;
    rt.block_on(sw_mcp::signin::pin_issuer(&pinned, &home, ISSUER))
        .map_err(|x| format!("{x:?}"))?;
    let assertion = SignInAssertion {
        identity: MlDsa65Pk::from_bytes(identity.public_key().as_bytes())
            .map_err(|x| format!("{x:?}"))?,
        device: keys.device_label("dev"),
        aud: "sw-mcp".into(),
        exp_s: now_ms() / 1000 + 86_400,
        nonce: [0x42; 32],
    };
    let signed = issuer
        .sign_assertion(&assertion)
        .map_err(|x| format!("{x:?}"))?;
    write_private(&home.assertion(), &signed).map_err(|x| e(&x))?;
    let id = CitadelIdentity.id_fp(identity.public_key().as_bytes());
    Ok(id.map_err(|x| format!("{x:?}"))?.to_string())
}
