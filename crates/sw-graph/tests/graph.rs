#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

use sw_graph::*;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

const PARAMS: LayoutParams = LayoutParams {
    node_gap: 10.0,
    level_gap: 80.0,
};

fn node(key: &str, parent: Option<usize>, kind: NodeKind, w: f32) -> NodeSpec {
    NodeSpec {
        key: key.into(),
        parent,
        kind,
        label: key.into(),
        size: (w, 20.0),
    }
}

/// Project → 2 Tasks; task A → Person → 2 Agents → Progress each; task B → Person.
fn sample() -> Tree {
    use NodeKind::*;
    Tree::new(vec![
        node("p", None, Project, 60.0),
        node("ta", Some(0), Task, 40.0),
        node("tb", Some(0), Task, 40.0),
        node("alice", Some(1), Person, 20.0),
        node("a1", Some(3), Agent, 20.0),
        node("a2", Some(3), Agent, 20.0),
        node("g1", Some(4), Progress, 50.0),
        node("g2", Some(5), Progress, 50.0),
        node("bob", Some(2), Person, 20.0),
    ])
    .unwrap()
}

fn depths(tree: &Tree) -> Vec<usize> {
    let mut d = vec![0; tree.len()];
    for (i, n) in tree.nodes().iter().enumerate() {
        if let Some(p) = n.parent {
            d[i] = d[p] + 1;
        }
    }
    d
}

#[test]
fn no_two_nodes_on_a_level_overlap() {
    let tree = sample();
    let layout = layout(&tree, &PARAMS);
    let d = depths(&tree);
    for i in 0..tree.len() {
        for j in 0..tree.len() {
            if i < j && d[i] == d[j] {
                let (pi, pj) = (layout.positions[i], layout.positions[j]);
                let need =
                    (tree.nodes()[i].size.0 + tree.nodes()[j].size.0) / 2.0 + PARAMS.node_gap;
                assert!((pi.x - pj.x).abs() >= need - 1e-3, "{i} and {j} overlap");
            }
        }
    }
}

#[test]
fn parents_are_centred_over_their_children_and_levels_are_rows() {
    let tree = sample();
    let pos = layout(&tree, &PARAMS).positions;
    for (parent, kids) in [(0usize, [1usize, 2]), (3, [4, 5])] {
        let mid = (pos[kids[0]].x + pos[kids[1]].x) / 2.0;
        assert!((pos[parent].x - mid).abs() < 1e-3, "parent {parent}");
    }
    let d = depths(&tree);
    for (i, p) in pos.iter().enumerate() {
        assert_eq!(p.y, d[i] as f32 * PARAMS.level_gap);
    }
    let left = tree
        .nodes()
        .iter()
        .zip(&pos)
        .map(|(n, p)| p.x - n.size.0 / 2.0);
    assert!(
        left.fold(f32::INFINITY, f32::min).abs() < 1e-3,
        "leftmost edge at 0"
    );
}

#[test]
fn siblings_keep_their_order_and_layout_is_deterministic() {
    let tree = sample();
    let a = layout(&tree, &PARAMS);
    assert_eq!(a, layout(&tree, &PARAMS));
    assert!(a.positions[1].x < a.positions[2].x);
    assert!(a.positions[4].x < a.positions[5].x);
    let keyed = a.keyed(&tree);
    assert_eq!(keyed[3], ("alice", a.positions[3]));
}

#[test]
fn a_forest_lays_roots_side_by_side() {
    let tree = Tree::new(vec![
        node("r1", None, NodeKind::Project, 30.0),
        node("r2", None, NodeKind::Project, 30.0),
    ])
    .unwrap();
    let pos = layout(&tree, &PARAMS).positions;
    assert_eq!(pos[1].x - pos[0].x, 40.0);
}

#[test]
fn malformed_trees_are_refused() {
    let forward = vec![
        node("a", Some(1), NodeKind::Task, 1.0),
        node("b", None, NodeKind::Task, 1.0),
    ];
    assert_eq!(Tree::new(forward), Err(GraphError::BadParent { node: 0 }));
    assert_eq!(Tree::new(vec![]), Err(GraphError::Empty));
    let nan = vec![node("a", None, NodeKind::Task, f32::NAN)];
    assert_eq!(Tree::new(nan), Err(GraphError::BadSize { node: 0 }));
}

#[test]
fn hit_test_finds_the_node_under_the_point() {
    let tree = sample();
    let pos = layout(&tree, &PARAMS).positions;
    assert_eq!(hit_test(&tree, &pos, pos[8].x, pos[8].y), Some(8));
    assert_eq!(hit_test(&tree, &pos, -500.0, -500.0), None);
}

#[test]
fn spring_converges_and_lite_swaps_at_once() {
    let from = vec![Pos { x: 0.0, y: 0.0 }];
    let to = vec![Pos { x: 100.0, y: 50.0 }];
    let spring = Spring {
        stiffness: 170.0,
        damping: 26.0,
        rest: 0.01,
    };
    let mut tween = Tween::new(from.clone(), to.clone()).unwrap();
    assert!(tween.step(16.0, Motion::Full(spring)));
    assert!(tween.positions()[0].x > 0.0 && tween.positions()[0].x < 100.0);
    let mut frames = 1;
    while tween.step(16.0, Motion::Full(spring)) {
        frames += 1;
        assert!(frames < 600, "spring never settled");
    }
    assert_eq!(tween.positions(), to.as_slice());
    let mut lite = Tween::new(from, to.clone()).unwrap();
    assert!(!lite.step(16.0, Motion::Lite));
    assert_eq!(lite.positions(), to.as_slice());
    assert!(Tween::new(vec![], to).is_none(), "length mismatch refused");
}

#[test]
fn scene_draws_every_edge_node_and_region() {
    let tree = sample();
    let pos = layout(&tree, &PARAMS).positions;
    let theme = Theme {
        background: 0x0000_00ff,
        edge: 0x8888_88ff,
        edge_width: 1.0,
        project: 0x5b3fa8ff,
        task: 0x76b900ff,
        person: 0xffffffff,
        agent: 0xccccccff,
        progress: 0x333333ff,
        label: 0xffffffff,
        label_size: 12.0,
    };
    let ops = scene(&tree, &pos, &theme);
    let count = |f: fn(&DrawOp) -> bool| ops.iter().filter(|o| f(o)).count();
    assert_eq!(count(|o| matches!(o, DrawOp::Line { .. })), tree.len() - 1);
    assert_eq!(count(|o| matches!(o, DrawOp::Region { .. })), tree.len());
    assert_eq!(
        count(|o| matches!(o, DrawOp::FillCircle { .. })),
        4,
        "persons and agents"
    );
    assert!(matches!(ops[0], DrawOp::Clear { .. }));
}
