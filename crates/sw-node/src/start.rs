//! Starting the node: provisioning to SDK settings, stored state to a [`NodeCore`]. The host
//! chooses only the transport (an injected listener in a Durable Object, sockets in tests);
//! every setting below comes from [`Provisioning`], and an invalid one stops the start.

use citadel_sdk::prelude::*;
use citadel_sdk::types::crypto::MaxSecurityLevel;
use citadel_user::auth::pq::oprf::OprfSeed;
use citadel_user::auth::pq::record::KsfParams;
use citadel_user::auth::pq::server::PqAuthServerSettings;
use citadel_user::auth::pq::throttle::ThrottlePolicy;
use citadel_user::credentials::CredentialRequirements;
use sw_protocol::ports::Clock;

use crate::core::load::LoadError;
use crate::core::{NodeCore, Rules, ServerIdentity};
use crate::kernel::SwKernel;
use crate::provision::Provisioning;
use crate::store::{NodeStore, StoreError};

#[derive(Debug)]
pub enum StartError {
    Store(StoreError),
    Load(LoadError),
    Settings(String),
}

impl std::fmt::Display for StartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Store(e) => write!(f, "node store: {e}"),
            Self::Load(e) => write!(f, "stored {}: {}", e.key, e.why),
            Self::Settings(why) => write!(f, "node settings: {why}"),
        }
    }
}

/// The SDK's sign-in throttle as its authors recommend it (five failures or twenty starts per
/// fifteen minutes per username and network): the node has no reason to differ from it.
pub const SIGN_IN_THROTTLE: ThrottlePolicy = ThrottlePolicy::RECOMMENDED;

/// The highest security level any session with this node is built at: every synchronized.work
/// client (sw-mcp, sw-bench) opens its session at `Standard` and none raises it per message.
pub const MAX_SESSION_LEVEL: SecurityLevel = SecurityLevel::Standard;

/// The node's maximum security level ([`MAX_SESSION_LEVEL`]), as the SDK requires it.
pub fn max_security_level() -> Result<MaxSecurityLevel, StartError> {
    MaxSecurityLevel::new(MAX_SESSION_LEVEL).map_err(|e| StartError::Settings(e.to_string()))
}

/// The SDK settings provisioning names. Post-quantum sign-in is on (OPRF + client-side
/// Argon2id, D1), transient connections are off (the production setting the hosted kernel
/// uses), and the server-side Argon2 is the isolate's cost. Links are kept the SDK's `Legacy`
/// way: the node holds C2S sessions only, so the managed P2P link policy has nothing to keep.
pub fn configure<T: PlatformOps>(
    builder: &mut NodeBuilder<StackedRatchet, T>,
    p: &Provisioning,
    backend: BackendType,
) -> Result<(), StartError> {
    let settings = |e: &dyn std::fmt::Debug| StartError::Settings(format!("{e:?}"));
    let ksf = KsfParams {
        mem_kib: p.ksf.mem_kib,
        iterations: p.ksf.iterations,
        lanes: p.ksf.lanes,
    };
    let pq = PqAuthServerSettings::new(OprfSeed::from_bytes(*p.oprf_seed), ksf, SIGN_IN_THROTTLE)
        .map_err(|e| settings(&e))?;
    let _ = builder
        .with_backend(backend)
        .with_link_manager(LinkManagerMode::Legacy)
        .with_max_security_level(max_security_level()?)
        .with_server_limits(p.server_limits.clone())
        .with_stun_servers(p.stun_servers.clone())
        .with_path_policy(p.path_policy.clone())
        .with_server_misc_settings(ServerMiscSettings {
            allow_transient_connections: false,
            credential_requirements: credential_requirements(),
            pq_sign_in: Some(pq),
            admission: None,
        })
        .with_server_argon_settings(ArgonDefaultServerSettings {
            lanes: p.argon.lanes,
            mem_cost: p.argon.mem_kib,
            time_cost: p.argon.time_cost,
            ..ArgonDefaultServerSettings::default()
        });
    Ok(())
}

/// The SDK's credential rules with the username cap raised to the identity crate's, so a V2
/// account name (`<id_fp>.<slug>@synchronized.work`, design §5.2) fits: the SDK's own cap
/// is 37 bytes, shorter than any such name with a slug over three characters.
pub fn credential_requirements() -> CredentialRequirements {
    const MAX_USERNAME: u8 = {
        assert!(citadel_identity::MAX_USERNAME_BYTES <= u8::MAX as usize);
        citadel_identity::MAX_USERNAME_BYTES as u8
    };
    CredentialRequirements {
        max_username_length: MAX_USERNAME,
        ..CredentialRequirements::default()
    }
}

/// The kernel over everything `store` holds. A stored entry that does not decode stops the
/// start: a node that skipped a record would treat its Person as a stranger.
pub async fn prepare<S: NodeStore, C: Clock + Send + Sync + 'static>(
    p: &Provisioning,
    store: S,
    clock: C,
) -> Result<SwKernel<S, C>, StartError> {
    let mut core = NodeCore::new(Rules {
        sign_in_issuer: p.sign_in_issuer.as_bytes().to_vec(),
        limits: p.limits,
        new_person_policy: p.new_person_policy,
        server_identity: Some(ServerIdentity::new(p.identity.clone(), p.server_key())),
    });
    let mut stored = store.load_all().await.map_err(StartError::Store)?;
    // Sorted, so an entry is loaded before the keys nested under it (a note before its acks).
    stored.sort();
    for (key, value) in stored {
        core.load(&key, &value).map_err(StartError::Load)?;
    }
    Ok(SwKernel::new(core, store, clock))
}
