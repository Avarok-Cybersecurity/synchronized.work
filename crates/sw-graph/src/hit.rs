use crate::layout::Pos;
use crate::tree::Tree;

/// The topmost node whose box (centred on its position) contains `(x, y)`: later nodes are
/// drawn over earlier ones, so the search runs from the end.
pub fn hit_test(tree: &Tree, positions: &[Pos], x: f32, y: f32) -> Option<usize> {
    tree.nodes()
        .iter()
        .zip(positions)
        .enumerate()
        .rev()
        .find(|(_, (node, p))| {
            let (w, h) = node.size;
            (x - p.x).abs() <= w / 2.0 && (y - p.y).abs() <= h / 2.0
        })
        .map(|(i, _)| i)
}
