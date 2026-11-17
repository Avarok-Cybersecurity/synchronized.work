#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! PCND: the node refuses to start without every provisioned value (§11.4 "smoke": a scratch
//! Worker without provisioning → the node refuses to start). Each name is removed in turn
//! and must be the one the refusal names.

mod support;

use std::collections::BTreeMap;

use support::vars::{Vars, complete};
use sw_node::provision::*;
use sw_node::{ProvisionError, Provisioning};

#[test]
fn a_complete_provisioning_starts_and_names_synchronized_work() {
    let p = Provisioning::from_source(&complete()).unwrap();
    assert_eq!(p.identity.as_str(), "synchronized.work");
}

#[test]
fn every_missing_value_refuses_the_start_by_name() {
    let names: Vec<&'static str> = complete().0.keys().copied().collect();
    for name in names {
        let mut vars = complete();
        vars.0.remove(name);
        assert_eq!(
            Provisioning::from_source(&vars).map(|_| ()),
            Err(ProvisionError::Missing(name)),
            "without {name}"
        );
    }
}

#[test]
fn an_empty_source_refuses_the_start() {
    assert!(Provisioning::from_source(&Vars(BTreeMap::new())).is_err());
}

#[test]
fn a_short_seed_and_an_unknown_approval_are_malformed() {
    let mut vars = complete();
    vars.0.insert(SECRET_SERVER_KEY_SEED, "11".repeat(31));
    assert!(matches!(
        Provisioning::from_source(&vars),
        Err(ProvisionError::Malformed {
            name: SECRET_SERVER_KEY_SEED,
            ..
        })
    ));
    let mut vars = complete();
    vars.0.insert(VAR_PERSON_APPROVAL, "sometimes".into());
    assert!(Provisioning::from_source(&vars).is_err());
    let mut vars = complete();
    vars.0.insert(VAR_PATH_POLICY, "{}".into());
    assert!(matches!(
        Provisioning::from_source(&vars),
        Err(ProvisionError::Malformed {
            name: VAR_PATH_POLICY,
            ..
        })
    ));
}

#[test]
fn the_well_known_document_is_this_nodes_signed_identity() {
    let p = Provisioning::from_source(&complete()).unwrap();
    let json = sw_node::well_known::well_known_json(&p).unwrap();
    let domain = citadel_identity::canonical_domain("synchronized.work").unwrap();
    let doc = citadel_identity::parse_well_known(&json, &domain).unwrap();
    assert_eq!(
        doc.citadel_wss.as_deref(),
        Some("wss://synchronized.work/citadel")
    );
    assert_eq!(doc.issued_at_s, 1_794_700_000);
    let key = citadel_identity::ServerKey::from_seed(&[0x11; 32]);
    assert_eq!(&doc.server_key, key.public_key());
    assert_eq!(
        json,
        sw_node::well_known::well_known_json(&p).unwrap(),
        "deterministic"
    );
}
