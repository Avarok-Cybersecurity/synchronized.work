//! Tidy tree (Reingold–Tilford): each subtree is laid out once, then placed against its left
//! siblings as close as their contours allow; a parent is centred over its children.
//! Contours are explicit per-depth extents, O(nodes × depth).

use crate::tree::Tree;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pos {
    pub x: f32,
    pub y: f32,
}

/// Every spacing is explicit (PCND): the two views choose their own.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutParams {
    /// Horizontal gap between neighbouring nodes at the same depth.
    pub node_gap: f32,
    /// Vertical distance between depth levels (centre to centre).
    pub level_gap: f32,
}

/// Centre positions, one per node in input order, with the leftmost edge at `x = 0` and the
/// roots' centres at `y = 0`.
#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    pub positions: Vec<Pos>,
}

impl Layout {
    /// `(key, position)` per node: what the site's adapter hands React Flow, so the web view
    /// draws exactly the positions the App canvas draws.
    pub fn keyed<'t>(&self, tree: &'t Tree) -> Vec<(&'t str, Pos)> {
        tree.nodes()
            .iter()
            .zip(&self.positions)
            .map(|(n, p)| (n.key.as_str(), *p))
            .collect()
    }
}

/// Per depth (0 = the subtree's root), the leftmost and rightmost extents relative to the
/// subtree root's centre.
type Contour = Vec<(f32, f32)>;

struct Placed {
    contour: Contour,
    /// `(node, x offset from the subtree root)` for every node in the subtree.
    offsets: Vec<(usize, f32)>,
}

pub fn layout(tree: &Tree, params: &LayoutParams) -> Layout {
    let forest = place_children(tree, tree.roots(), params);
    let mut positions = vec![Pos { x: 0.0, y: 0.0 }; tree.len()];
    let mut depth = vec![0usize; tree.len()];
    for (i, node) in tree.nodes().iter().enumerate() {
        if let Some(p) = node.parent {
            depth[i] = depth[p] + 1;
        }
    }
    for (node, x) in &forest.offsets {
        positions[*node] = Pos {
            x: *x,
            y: depth[*node] as f32 * params.level_gap,
        };
    }
    let min_left = forest
        .contour
        .iter()
        .map(|(l, _)| *l)
        .fold(f32::INFINITY, f32::min);
    for p in &mut positions {
        p.x -= min_left;
    }
    Layout { positions }
}

fn place(tree: &Tree, node: usize, params: &LayoutParams) -> Placed {
    let half = tree.nodes()[node].size.0 / 2.0;
    let kids = tree.children(node);
    if kids.is_empty() {
        return Placed {
            contour: vec![(-half, half)],
            offsets: vec![(node, 0.0)],
        };
    }
    let below = place_children(tree, kids, params);
    let mut contour = vec![(-half, half)];
    contour.extend(below.contour);
    let mut offsets = vec![(node, 0.0)];
    offsets.extend(below.offsets);
    Placed { contour, offsets }
}

/// Lays `nodes` side by side as siblings and centres the row on 0 (the midpoint of the first
/// and last sibling), returning the row's combined contour.
fn place_children(tree: &Tree, nodes: &[usize], params: &LayoutParams) -> Placed {
    let mut row: Option<Placed> = None;
    let mut centres = Vec::with_capacity(nodes.len());
    for &child in nodes {
        let sub = place(tree, child, params);
        let shift = match &row {
            None => 0.0,
            Some(acc) => acc
                .contour
                .iter()
                .zip(&sub.contour)
                .map(|((_, right), (left, _))| right - left + params.node_gap)
                .fold(f32::NEG_INFINITY, f32::max),
        };
        centres.push(shift);
        row = Some(match row {
            None => sub,
            Some(acc) => merge(acc, sub, shift),
        });
    }
    let mut row = row.unwrap_or(Placed {
        contour: Vec::new(),
        offsets: Vec::new(),
    });
    let mid = match (centres.first(), centres.last()) {
        (Some(first), Some(last)) => (first + last) / 2.0,
        _ => 0.0,
    };
    for (_, x) in &mut row.offsets {
        *x -= mid;
    }
    for (l, r) in &mut row.contour {
        *l -= mid;
        *r -= mid;
    }
    row
}

fn merge(mut acc: Placed, sub: Placed, shift: f32) -> Placed {
    for (d, (left, right)) in sub.contour.into_iter().enumerate() {
        let (l, r) = (left + shift, right + shift);
        match acc.contour.get_mut(d) {
            Some(extent) => *extent = (extent.0.min(l), extent.1.max(r)),
            None => acc.contour.push((l, r)),
        }
    }
    acc.offsets
        .extend(sub.offsets.into_iter().map(|(n, x)| (n, x + shift)));
    acc
}
