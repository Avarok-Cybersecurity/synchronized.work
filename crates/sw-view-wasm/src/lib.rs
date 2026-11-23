//! The site's Rust core (master plan §9a: React and React Flow are the view only). It folds a
//! Project's messages with `sw-client`, turns the result into the tree, lays it out once with
//! `sw-graph` (the same positions the Citadel App canvas draws) and hands React a finished
//! [`ViewGraph`]. The tree and its layout are `sw-view`'s, the one copy the Citadel App core
//! shares. It also starts "Sign in with Citadel". The wasm-bindgen surface is in `bindings` and
//! is thin: every decision is in the native-tested modules.

pub mod demo;
pub mod site;

pub use sw_view::{points, view};

#[cfg(target_arch = "wasm32")]
mod bindings;

pub use view::{
    Kind, KindSizes, Point, ViewError, ViewGraph, ViewNode, ViewParams, relayout, tree_of, view_of,
};

/// The demo, laid out.
pub fn demo_view(params: &ViewParams) -> Result<ViewGraph, String> {
    let state = demo::state()?;
    view_of(
        points::points(&state, demo::DEMO_PROJECT_NAME),
        true,
        params,
    )
    .map_err(|e| format!("{e:?}"))
}
