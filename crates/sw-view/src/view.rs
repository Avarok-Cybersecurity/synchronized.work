//! The view graph React Flow draws: one node per point of the tree, each with the position
//! `sw-graph` computed (SSOT for both views), and nothing React has to decide. Edges are not
//! stored: each node names its parent, and the view derives the edge from that.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sw_graph::{GraphError, LayoutParams, NodeKind, NodeSpec, Tree, layout};

/// Every size and gap is chosen by the caller (PCND); the site passes its theme's.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewParams {
    pub node_gap: f32,
    pub level_gap: f32,
    pub sizes: KindSizes,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KindSizes {
    pub project: (f32, f32),
    pub task: (f32, f32),
    pub person: (f32, f32),
    pub agent: (f32, f32),
    pub progress: (f32, f32),
}

impl KindSizes {
    pub fn of(&self, kind: Kind) -> (f32, f32) {
        match kind {
            Kind::Project => self.project,
            Kind::Task => self.task,
            Kind::Person => self.person,
            Kind::Agent => self.agent,
            Kind::Progress => self.progress,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Project,
    Task,
    Person,
    Agent,
    Progress,
}

impl From<Kind> for NodeKind {
    fn from(k: Kind) -> Self {
        match k {
            Kind::Project => NodeKind::Project,
            Kind::Task => NodeKind::Task,
            Kind::Person => NodeKind::Person,
            Kind::Agent => NodeKind::Agent,
            Kind::Progress => NodeKind::Progress,
        }
    }
}

/// One point, before layout: what the tree builder decides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Point {
    pub id: String,
    pub parent: Option<usize>,
    pub kind: Kind,
    pub label: String,
    pub detail: String,
    /// `off` / `aware` / `synchronized` (Tasks), `running` / … (Progress): a text the view
    /// shows beside its ring or badge, so state is never colour alone.
    pub state: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewNode {
    pub id: String,
    pub parent: Option<String>,
    pub kind: Kind,
    pub label: String,
    pub detail: String,
    pub state: Option<String>,
    /// Centre, in view units.
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewGraph {
    /// `true` for the fixture demo; the view labels it as such on every screen.
    pub demo: bool,
    /// Parents before children (the order `sw-graph` requires).
    pub nodes: Vec<ViewNode>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ViewError {
    Graph(GraphError),
    /// A node's parent id is not an earlier node.
    UnknownParent(String),
}

/// Lays out `points` with each kind's size from `params`.
pub fn view_of(
    points: Vec<Point>,
    demo: bool,
    params: &ViewParams,
) -> Result<ViewGraph, ViewError> {
    let ids: Vec<String> = points.iter().map(|p| p.id.clone()).collect();
    let nodes = points
        .into_iter()
        .map(|p| {
            let (w, h) = params.sizes.of(p.kind);
            ViewNode {
                parent: p.parent.map(|i| ids[i].clone()),
                id: p.id,
                kind: p.kind,
                label: p.label,
                detail: p.detail,
                state: p.state,
                x: 0.0,
                y: 0.0,
                w,
                h,
            }
        })
        .collect();
    relayout(ViewGraph { demo, nodes }, params.node_gap, params.level_gap)
}

/// Positions for the graph's nodes at their current `w`/`h` (the sizes the view measured);
/// everything else is kept as it is.
pub fn relayout(
    mut graph: ViewGraph,
    node_gap: f32,
    level_gap: f32,
) -> Result<ViewGraph, ViewError> {
    let tree = tree_of(&graph)?;
    let laid = layout(
        &tree,
        &LayoutParams {
            node_gap,
            level_gap,
        },
    );
    for (node, p) in graph.nodes.iter_mut().zip(&laid.positions) {
        node.x = p.x;
        node.y = p.y;
    }
    Ok(graph)
}

/// The `sw-graph` tree a view graph describes.
pub fn tree_of(graph: &ViewGraph) -> Result<Tree, ViewError> {
    let mut index: BTreeMap<&str, usize> = BTreeMap::new();
    let mut specs: Vec<NodeSpec> = Vec::with_capacity(graph.nodes.len());
    for (i, node) in graph.nodes.iter().enumerate() {
        let parent = match &node.parent {
            None => None,
            Some(id) => Some(
                *index
                    .get(id.as_str())
                    .ok_or_else(|| ViewError::UnknownParent(id.clone()))?,
            ),
        };
        index.insert(&node.id, i);
        specs.push(NodeSpec {
            key: node.id.clone(),
            parent,
            kind: node.kind.into(),
            label: node.label.clone(),
            size: (node.w, node.h),
        });
    }
    Tree::new(specs).map_err(ViewError::Graph)
}
