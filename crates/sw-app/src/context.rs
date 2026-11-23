//! `AppContext` -> the core's [`AppCtx`]. Only what design §5.3 lets the App know enters:
//! `acting_username`, `install_id`, `node_id`, `app_id`, `scope`, the grant, the size, the
//! theme, the locale and the host's ABI minor. The context is destructured field by field with
//! no `..`, so a field added to `AppContext` stops this compiling until it is decided on.

use citadel_app_sdk_types::{AppContext, AppScope, Capability};
use sw_app_core::{AppCtx, CanvasAbi, Granted, Scope, Size};
use sw_graph::Motion;

use crate::{config, theme};

pub fn scope_of(scope: &AppScope) -> Scope {
    match scope {
        AppScope::Workspace => Scope::Workspace,
        AppScope::Node { node_id } => Scope::Node {
            node_id: node_id.clone(),
        },
        AppScope::NodeTree { node_id } => Scope::NodeTree {
            node_id: node_id.clone(),
        },
        AppScope::Users { usernames } => Scope::Users {
            usernames: usernames.clone(),
        },
        AppScope::SelfOnly { username } => Scope::SelfOnly {
            username: username.clone(),
        },
    }
}

pub fn granted_of(granted: &[Capability]) -> Granted {
    let has = |cap: Capability| granted.contains(&cap);
    Granted {
        citadel_session: has(Capability::CitadelSession),
        net_host: has(Capability::NetHost),
        notifications: has(Capability::Notifications),
        storage_user: has(Capability::StorageUser),
        storage_local: has(Capability::StorageLocal),
    }
}

/// The App's motion: the host states no reduced-motion preference, so `config::SPRING`.
pub fn motion() -> Motion {
    Motion::Full(config::SPRING)
}

pub fn ctx_of(ctx: &AppContext) -> AppCtx {
    let AppContext {
        app_id,
        install_id,
        scope,
        granted,
        surface: _,
        size,
        dpr: _,
        theme,
        locale,
        acting_username,
        node_id,
        close_grace_ms: _,
        sdk_abi_minor,
    } = ctx;
    AppCtx {
        acting_username: acting_username.clone(),
        install_id: install_id.clone(),
        node_id: node_id.clone(),
        app_id: app_id.clone(),
        scope: scope_of(scope),
        granted: granted_of(granted),
        canvas: CanvasAbi::from_abi_minor(*sdk_abi_minor),
        size: Size {
            width: size.width as f32,
            height: size.height as f32,
        },
        theme: theme::theme_of(theme, motion()),
        locale: locale.clone(),
    }
}
