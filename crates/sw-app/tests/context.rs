#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! `AppContext` -> `AppCtx` carries the allowed fields and nothing else (design §5.3), and the
//! constants are the site's and the manifest's.

use std::collections::BTreeMap;

use citadel_app_sdk_types::{AppContext, AppScope, Capability, Size, Surface};
use sw_app::config;
use sw_app::context::{ctx_of, granted_of, scope_of};
use sw_app_core::{CanvasAbi, Scope};

fn context() -> AppContext {
    AppContext {
        app_id: config::APP_ID.into(),
        install_id: "inst-1".into(),
        scope: AppScope::Node {
            node_id: "n-1".into(),
        },
        granted: vec![Capability::CitadelSession, Capability::StorageUser],
        surface: Surface::NodePage {},
        size: Size {
            width: 800,
            height: 600,
        },
        dpr: 1.0,
        theme: BTreeMap::new(),
        locale: "en".into(),
        acting_username: "alice".into(),
        node_id: "n-1".into(),
        close_grace_ms: 500,
        sdk_abi_minor: 1,
    }
}

#[test]
fn fields_outside_design_5_3_do_not_reach_the_core() {
    let a = context();
    let b = AppContext {
        surface: Surface::NodeTab { tab_id: "t".into() },
        dpr: 3.0,
        close_grace_ms: 9,
        ..context()
    };
    assert_eq!(ctx_of(&a), ctx_of(&b));
}

#[test]
fn each_allowed_field_does_reach_the_core() {
    let base = ctx_of(&context());
    let changed = [
        AppContext {
            app_id: "x.y".into(),
            ..context()
        },
        AppContext {
            install_id: "inst-2".into(),
            ..context()
        },
        AppContext {
            scope: AppScope::Workspace,
            ..context()
        },
        AppContext {
            granted: vec![Capability::NetHost],
            ..context()
        },
        AppContext {
            size: Size {
                width: 1,
                height: 600,
            },
            ..context()
        },
        AppContext {
            theme: BTreeMap::from([("primary".into(), "#000".into())]),
            ..context()
        },
        AppContext {
            locale: "de".into(),
            ..context()
        },
        AppContext {
            acting_username: "bob".into(),
            ..context()
        },
        AppContext {
            node_id: "n-2".into(),
            ..context()
        },
        AppContext {
            sdk_abi_minor: 0,
            ..context()
        },
    ];
    for ctx in changed {
        assert_ne!(ctx_of(&ctx), base, "{ctx:?}");
    }
}

#[test]
fn the_core_is_told_the_canvas_the_host_paints() {
    assert_eq!(ctx_of(&context()).canvas, CanvasAbi::Cw5);
    assert_eq!(
        ctx_of(&AppContext {
            sdk_abi_minor: 0,
            ..context()
        })
        .canvas,
        CanvasAbi::Abi1
    );
}

#[test]
fn every_scope_and_grant_maps_to_its_namesake() {
    let node = || "n".to_owned();
    assert_eq!(scope_of(&AppScope::Workspace), Scope::Workspace);
    assert_eq!(
        scope_of(&AppScope::Node { node_id: node() }),
        Scope::Node { node_id: node() }
    );
    let tree = AppScope::NodeTree { node_id: node() };
    assert_eq!(scope_of(&tree), Scope::NodeTree { node_id: node() });
    let users = AppScope::Users {
        usernames: vec!["a".into()],
    };
    assert_eq!(
        scope_of(&users),
        Scope::Users {
            usernames: vec!["a".into()]
        }
    );
    let me = AppScope::SelfOnly {
        username: "a".into(),
    };
    assert_eq!(
        scope_of(&me),
        Scope::SelfOnly {
            username: "a".into()
        }
    );
    let caps = [
        Capability::CitadelSession,
        Capability::NetHost,
        Capability::Notifications,
        Capability::StorageUser,
        Capability::StorageLocal,
    ];
    for (i, cap) in caps.into_iter().enumerate() {
        let g = granted_of(&[cap]);
        let flags = [
            g.citadel_session,
            g.net_host,
            g.notifications,
            g.storage_user,
            g.storage_local,
        ];
        let want: Vec<bool> = (0..caps.len()).map(|j| j == i).collect();
        assert_eq!(flags.to_vec(), want, "{cap:?}");
    }
    let none = granted_of(&[Capability::NetServer, Capability::ClipboardWrite]);
    assert!(!(none.citadel_session || none.net_host || none.notifications));
}

#[test]
fn the_view_params_are_the_sites() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../site/src/view/params.json"
    );
    let site: sw_view::ViewParams =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(config::VIEW, site);
}

#[test]
fn the_config_is_the_constants() {
    let c = config::app_config();
    assert_eq!(c.server, config::SERVER);
    assert_eq!(c.poll.origin, format!("https://{}", config::POLL_HOST));
    assert_eq!(c.poll.max_ms, 25_000);
    assert_eq!(
        (c.session.max_msg_bytes, c.session.max_msgs_per_s),
        (65536, 8)
    );
    assert!(c.site_url.starts_with("https://"));
}
