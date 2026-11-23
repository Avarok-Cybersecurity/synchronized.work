//! The tree both shells draw (design §7.3: one renderer, two shells). [`points`] turns a
//! viewer's folded [`sw_client::live::ProjectState`] into points in a fixed, id-only order;
//! [`view_of`] lays them out once with `sw-graph`; [`tree_of`] gives back the `sw-graph` tree.
//! The site's wasm adapter (`sw-view-wasm`) and the Citadel App core (`sw-app-core`) both
//! depend on this crate, so neither keeps a copy.

pub mod points;
pub mod view;

pub use view::{
    Kind, KindSizes, Point, ViewError, ViewGraph, ViewNode, ViewParams, relayout, tree_of, view_of,
};
