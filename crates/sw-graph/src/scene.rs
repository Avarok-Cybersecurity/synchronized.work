//! A tree and its positions as one draw list: edges first, then nodes, labels and regions.

use crate::draw::{DrawOp, RegionRole};
use crate::layout::Pos;
use crate::tree::{NodeKind, Tree};

/// Every colour and size explicit (PCND); the site and the App pass their own theme.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    pub background: u32,
    pub edge: u32,
    pub edge_width: f32,
    pub project: u32,
    pub task: u32,
    pub person: u32,
    pub agent: u32,
    pub progress: u32,
    pub label: u32,
    pub label_size: f32,
}

impl Theme {
    fn fill(&self, kind: NodeKind) -> u32 {
        match kind {
            NodeKind::Project => self.project,
            NodeKind::Task => self.task,
            NodeKind::Person => self.person,
            NodeKind::Agent => self.agent,
            NodeKind::Progress => self.progress,
        }
    }
}

pub fn scene(tree: &Tree, positions: &[Pos], theme: &Theme) -> Vec<DrawOp> {
    let mut ops = vec![DrawOp::Clear {
        color: theme.background,
    }];
    for (node, p) in tree.nodes().iter().zip(positions) {
        if let Some(parent) = node.parent.and_then(|i| positions.get(i)) {
            ops.push(DrawOp::Line {
                x1: parent.x,
                y1: parent.y,
                x2: p.x,
                y2: p.y,
                line_width: theme.edge_width,
                color: theme.edge,
            });
        }
    }
    for (node, p) in tree.nodes().iter().zip(positions) {
        let (w, h) = node.size;
        let color = theme.fill(node.kind);
        ops.push(match node.kind {
            NodeKind::Person | NodeKind::Agent => DrawOp::FillCircle {
                cx: p.x,
                cy: p.y,
                r: w.min(h) / 2.0,
                color,
            },
            NodeKind::Project | NodeKind::Task | NodeKind::Progress => DrawOp::FillRect {
                x: p.x - w / 2.0,
                y: p.y - h / 2.0,
                w,
                h,
                color,
            },
        });
        ops.push(DrawOp::FillText {
            x: p.x - w / 2.0,
            y: p.y + h / 2.0 + theme.label_size,
            text: node.label.clone(),
            size: theme.label_size,
            color: theme.label,
        });
        ops.push(DrawOp::Region {
            id: node.key.clone(),
            x: p.x - w / 2.0,
            y: p.y - h / 2.0,
            w,
            h,
            label: node.label.clone(),
            role: RegionRole::Button,
        });
    }
    ops
}
