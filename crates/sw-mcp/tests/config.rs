#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! `config.toml` (PCND): every field required, none defaulted, unknown ones refused.

use sw_mcp::config::{Config, ConfigError};

const FULL: &str = r#"
server = "wss://synchronized.work/citadel"
device_slug = "macbook-pro"
sign_in_issuer = "work.avarok.net"
approval_mode_new_controller_hours = 24
ack_timeout_s = 600
reconnect_delay_s = 5
stun_servers = ["stun1.example.net:3478", "stun2.example.net:3478", "stun3.example.net:3478"]
log_level = "info"
harness = "stop_hook"
keystore = "file"

[rate]
per_kid_per_min = 10
pending_max = 50
refusals_per_min_before_pause = 3
"#;

#[test]
fn the_full_config_parses_and_names_the_device_account() {
    let c = Config::parse(FULL).unwrap();
    assert_eq!(c.rate.per_kid_per_min, 10);
    let fp = sw_model::IdFp::parse("3foriftckvpoy2rh").unwrap();
    assert_eq!(
        c.device_username(&fp).unwrap(),
        "3foriftckvpoy2rh.macbook-pro@synchronized.work"
    );
}

#[test]
fn every_missing_field_is_named() {
    assert!(Config::parse(FULL).is_ok());
    for line in FULL.lines().filter(|l| l.contains(" = ")) {
        let field = line.split(" = ").next().unwrap();
        let without = FULL.replace(&format!("{line}\n"), "");
        let Err(ConfigError::Parse(why)) = Config::parse(&without) else {
            panic!("{field} was optional")
        };
        assert!(why.contains(field), "{field}: {why}");
    }
}

#[test]
fn unknown_fields_insecure_servers_and_zero_limits_are_refused() {
    assert!(Config::parse(&format!("{FULL}\nextra = 1\n")).is_err());
    assert!(
        Config::parse(&FULL.replace(
            "approval_mode_new_controller_hours = 24",
            "approval_mode_new_controller_hours = 24\ncolour = 1"
        ))
        .is_err()
    );
    let ws = FULL.replace("wss://synchronized.work", "ws://synchronized.work");
    assert!(matches!(
        Config::parse(&ws),
        Err(ConfigError::Invalid {
            field: "server",
            ..
        })
    ));
    let spoof = FULL.replace("wss://synchronized.work", "ws://127.0.0.1.evil.example");
    assert!(matches!(
        Config::parse(&spoof),
        Err(ConfigError::Invalid {
            field: "server",
            ..
        })
    ));
    let local = FULL.replace("wss://synchronized.work", "ws://127.0.0.1:8787");
    assert!(
        Config::parse(&local).is_ok(),
        "a loopback node may be plain ws://"
    );
    let zero = FULL.replace("per_kid_per_min = 10", "per_kid_per_min = 0");
    assert!(matches!(
        Config::parse(&zero),
        Err(ConfigError::Invalid {
            field: "rate.per_kid_per_min",
            ..
        })
    ));
    let two = FULL.replace(", \"stun3.example.net:3478\"", "");
    assert!(matches!(
        Config::parse(&two),
        Err(ConfigError::Invalid {
            field: "stun_servers",
            ..
        })
    ));
    let slug = FULL.replace("macbook-pro", "MacBook");
    assert!(matches!(
        Config::parse(&slug),
        Err(ConfigError::Invalid {
            field: "device_slug",
            ..
        })
    ));
}

#[test]
fn a_wss_node_must_verify_its_certificate_and_only_loopback_ws_goes_without() {
    use citadel_sdk::prelude::CertVerification;
    let c = Config::parse(FULL).unwrap();
    assert_eq!(c.cert_verification().unwrap(), CertVerification::Required);
    let local = FULL.replace(
        "wss://synchronized.work/citadel",
        "ws://127.0.0.1:8787/citadel",
    );
    assert_eq!(
        Config::parse(&local).unwrap().cert_verification().unwrap(),
        CertVerification::SelfSignedAccepted
    );
}

/// The config.toml every release ships (and install.sh installs) is one this sw-mcp accepts,
/// once its DEVICE_SLUG is replaced; left as shipped it is refused (the control).
#[test]
fn the_released_config_parses_once_its_device_slug_is_set() {
    let shipped = include_str!("../../../release/config.toml");
    let config = sw_mcp::config::Config::parse(&shipped.replace("DEVICE_SLUG", "laptop")).unwrap();
    assert_eq!(config.server, "wss://synchronized.work/citadel");
    assert_eq!(config.sign_in_issuer, "work.avarok.net");
    assert!(sw_mcp::config::Config::parse(shipped).is_err());
}
