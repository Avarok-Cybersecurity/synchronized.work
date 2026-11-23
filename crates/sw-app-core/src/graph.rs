//! The live tree, with no layout or renderer of its own (design §7.3): the points come from
//! `sw_view::points`, the positions from `sw_view::view_of` (which runs `sw-graph`'s tidy
//! tree), the tree from `sw_view::tree_of`, the movement from `sw_graph::Tween` and the draw
//! list from `sw_graph::scene`. What is added here is only the view transform that fits the
//! tree to the pane ([`crate::fit`]).

use std::collections::BTreeMap;
use sw_client::live::ProjectState;

use sw_graph::{DrawOp, GraphError, Motion, NodeSpec, Pos, Tree, Tween, scene};
use sw_view::{ViewError, ViewGraph, points, tree_of, view_of};

use crate::copy;
use crate::ctx::{AppTheme, Size};
use crate::fit::{self, Area, FitParams};
use crate::painter::TextMeasure;

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

    /// The tree's draw list fitted into `area` of a pane of `size` ([`crate::fit`]): node
    /// shapes and edges through the transform, labels at their legible size and cut to their
    /// node, every region at least the minimum size, inside the pane, named in full.
    pub fn ops(
        &self,
        theme: &AppTheme,
        size: Size,
        area: Area,
        params: &FitParams,
        text: &dyn TextMeasure,
    ) -> Result<Vec<DrawOp>, GraphError> {
        let (Some((graph, tree)), Some(tween)) = (&self.graph, &self.tween) else {
            return Ok(vec![DrawOp::Clear {
                color: theme.graph.background,
            }]);
        };
        let rest: Vec<(Pos, (f32, f32))> = graph
            .nodes
            .iter()
            .zip(tree.nodes())
            .map(|(n, spec)| (Pos { x: n.x, y: n.y }, spec.size))
            .collect();
        let view = fit::fit(&rest, area, theme.graph.label_size, params);
        let s = view.scale;
        let scaled = tree
            .nodes()
            .iter()
            .map(|n| NodeSpec {
                size: (n.size.0 * s, n.size.1 * s),
                label: view
                    .labels
                    .then(|| fit::truncate(&n.label, n.size.0 * s, view.label_px, text))
                    .flatten()
                    .unwrap_or_default(),
                ..n.clone()
            })
            .collect();
        let at: Vec<Pos> = tween.positions().iter().map(|p| view.at(*p)).collect();
        let mut look = theme.graph;
        look.label_size = view.label_px;
        let full: BTreeMap<&str, &str> = tree
            .nodes()
            .iter()
            .map(|n| (n.key.as_str(), n.label.as_str()))
            .collect();
        let ops = scene(&Tree::new(scaled)?, &at, &look);
        Ok(ops
            .into_iter()
            .filter_map(|op| match op {
                DrawOp::FillText { text, .. } if text.is_empty() => None,
                DrawOp::Region {
                    id,
                    x,
                    y,
                    w,
                    h,
                    label,
                    role,
                } => {
                    let [x, y, w, h] = fit::region((x, y, w, h), params.region_min, size);
                    let label = full.get(id.as_str()).map_or(label, |l| (*l).to_owned());
                    Some(DrawOp::Region {
                        id,
                        x,
                        y,
                        w,
                        h,
                        label,
                        role,
                    })
                }
                other => Some(other),
            })
            .collect())
    }
}
