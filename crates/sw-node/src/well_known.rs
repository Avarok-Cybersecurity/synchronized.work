//! `/.well-known/citadel` for this node (identity-api §6, `well-known-citadel.schema.json`):
//! the document the node's own server key signs. The node IS a Citadel server, so `citadel`
//! is present, naming the `wss://` endpoint.

use citadel_identity::{ServerKey, WELL_KNOWN_VERSION, WellKnownCitadel, WellKnownError};

use crate::provision::Provisioning;

/// The served JSON: deterministic, so every isolate serves the same bytes for the same
/// provisioning (`issued_at` is provisioned, not read from a clock).
pub fn well_known_json(p: &Provisioning) -> Result<Vec<u8>, WellKnownError> {
    let key = ServerKey::from_seed(&p.server_key_seed);
    let doc = WellKnownCitadel {
        v: WELL_KNOWN_VERSION,
        canonical_domain: p.identity.clone(),
        citadel_wss: Some(p.citadel_wss.clone()),
        server_key: key.public_key().clone(),
        former_domains: p.former_domains.clone(),
        issued_at_s: p.server_key_issued_at_s,
    };
    key.sign_well_known(&doc)
}
