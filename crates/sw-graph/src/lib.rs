//! The graph core (design §7.3, master plan §9a): node positions are computed **once**, here,
//! and consumed by both views — React Flow on the site (positions handed over through the
//! site's wasm adapter) and the Citadel App canvas (through the draw-list IR and a
//! [`Painter`]). Nothing here knows a framework, a DOM or a canvas API.

mod draw;
mod hit;
mod layout;
mod motion;
mod scene;
mod tree;

pub use draw::{DrawOp, Painter, RegionRole};
pub use hit::hit_test;
pub use layout::{Layout, LayoutParams, Pos, layout};
pub use motion::{Motion, Spring, Tween};
pub use scene::{Theme, scene};
pub use tree::{GraphError, NodeKind, NodeSpec, Tree};
