//! The live tree, with no layout or renderer of its own (design §7.3): the points come from
//! `sw_view::points`, the positions from `sw_view::view_of` (which runs `sw-graph`'s tidy
//! tree), the tree from `sw_view::tree_of`, the movement from `sw_graph::Tween` and the draw
//! list from `sw_graph::scene`. What is added here is only where the canvas origin sits.

use sw_client::live::ProjectState;
use sw_graph::{DrawOp, Motion, Pos, Tree, Tween, scene};
use sw_view::{ViewError, ViewGraph, points, tree_of, view_of};

use crate::copy;
use crate::ctx::AppTheme;

#[derive(Debug, Clone, Default)]
pub struct GraphView {
    graph: Option<(ViewGraph, Tree)>,
    tween: Option<Tween>,
    moving: bool,
}

/// The label of the project point: the root Task's title when this viewer may see it.
pub fn project_name(state: &ProjectState) -> String {
    state
        .tasks
        .get(&state.project_id)
        .and_then(|t| t.title.clone())
        .unwrap_or_else(|| copy::PROJECT_UNTITLED.to_owned())
}

impl GraphView {
    pub fn new() -> Self {
        Self::default()
    }

    /// The node keys in draw order (`sw-view` point ids).
    pub fn keys(&self) -> Vec<&str> {
        self.graph
            .iter()
            .flat_map(|(g, _)| g.nodes.iter().map(|n| n.id.as_str()))
            .collect()
    }

    /// Re-derives the tree from `state`; nodes that were drawn before move from where they
    /// are, new ones grow out of their parent. Returns whether anything has to move.
    pub fn update(&mut self, state: &ProjectState, theme: &AppTheme) -> Result<bool, ViewError> {
        let graph = view_of(
            points::points(state, &project_name(state)),
            false,
            &theme.view,
        )?;
        let tree = tree_of(&graph)?;
        let target: Vec<Pos> = graph.nodes.iter().map(|n| Pos { x: n.x, y: n.y }).collect();
        let mut from: Vec<Pos> = Vec::with_capacity(target.len());
        for (i, node) in graph.nodes.iter().enumerate() {
            let before = self.current(&node.id);
            let parent = tree.nodes()[i].parent.and_then(|p| from.get(p).copied());
            from.push(before.or(parent).unwrap_or(target[i]));
        }
        self.moving = from != target;
        self.tween = Tween::new(from, target);
        self.graph = Some((graph, tree));
        Ok(self.moving)
    }

    fn current(&self, key: &str) -> Option<Pos> {
        let (graph, _) = self.graph.as_ref()?;
        let i = graph.nodes.iter().position(|n| n.id == key)?;
        self.tween.as_ref()?.positions().get(i).copied()
    }

    /// Advances the movement; returns whether another frame is needed.
    pub fn step(&mut self, dt_ms: f32, motion: Motion) -> bool {
        self.moving = self
            .tween
            .as_mut()
            .is_some_and(|tween| tween.step(dt_ms, motion));
        self.moving
    }

    /// Whether the last update or step left anything short of its target.
    pub fn is_moving(&self) -> bool {
        self.moving
    }

    /// The tree's draw list, its leftmost and topmost node inset by the theme's margin.
    pub fn ops(&self, theme: &AppTheme) -> Vec<DrawOp> {
        let (Some((_, tree)), Some(tween)) = (&self.graph, &self.tween) else {
            return vec![DrawOp::Clear {
                color: theme.graph.background,
            }];
        };
        let (half_w, half_h) = tree.nodes().iter().fold((0.0f32, 0.0f32), |(w, h), n| {
            (w.max(n.size.0 / 2.0), h.max(n.size.1 / 2.0))
        });
        let at: Vec<Pos> = tween
            .positions()
            .iter()
            .map(|p| Pos {
                x: p.x + theme.margin + half_w,
                y: p.y + theme.margin + half_h,
            })
            .collect();
        scene(tree, &at, &theme.graph)
    }
}
