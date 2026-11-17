#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! The layout core the site's React Flow view draws: deterministic on both targets, and a
//! laid-out tree survives the JSON round trip to the view and back.

use sw_view_wasm::{Kind, ViewParams, demo_view, relayout, tree_of};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

/// The site's own sizes and gaps (one file for both), so the golden layout is what ships.
const SITE_PARAMS: &str = include_str!("../../../site/src/view/params.json");

fn params() -> ViewParams {
    serde_json::from_str(SITE_PARAMS).unwrap()
}

const GOLDEN: &str = include_str!("golden/demo-view.json");

#[test]
fn the_demo_layout_is_the_golden_file_on_every_target() {
    let json = serde_json::to_string_pretty(&demo_view(&params()).unwrap()).unwrap();
    assert_eq!(
        json.trim_end(),
        GOLDEN.trim_end(),
        "layout drifted; actual:\n{json}"
    );
}

#[test]
fn the_same_input_lays_out_to_the_same_bytes() {
    let a = serde_json::to_vec(&demo_view(&params()).unwrap()).unwrap();
    let b = serde_json::to_vec(&demo_view(&params()).unwrap()).unwrap();
    assert_eq!(a, b);
}

#[test]
fn a_tree_survives_the_round_trip_through_the_view() {
    let graph = demo_view(&params()).unwrap();
    let before = tree_of(&graph).unwrap();
    let wire = serde_json::to_string(&graph).unwrap();
    let back: sw_view_wasm::ViewGraph = serde_json::from_str(&wire).unwrap();
    assert_eq!(tree_of(&back).unwrap(), before);
    let again = relayout(back, params().node_gap, params().level_gap).unwrap();
    assert_eq!(again, graph);
}

#[test]
fn the_demo_is_labelled_and_holds_the_whole_tree() {
    let graph = demo_view(&params()).unwrap();
    assert!(graph.demo);
    let count = |k: Kind| graph.nodes.iter().filter(|n| n.kind == k).count();
    assert_eq!(
        [
            Kind::Project,
            Kind::Task,
            Kind::Person,
            Kind::Agent,
            Kind::Progress
        ]
        .map(count),
        [1, 3, 4, 3, 3]
    );
    let agent = graph
        .nodes
        .iter()
        .find(|n| n.id.ends_with("agent:agent-7"))
        .unwrap();
    assert_eq!(agent.label, "agent-7");
    assert!(
        agent.detail.contains("online"),
        "the presence fixture marks agent-7 online"
    );
}

#[test]
fn siblings_never_overlap() {
    let graph = demo_view(&params()).unwrap();
    for a in &graph.nodes {
        for b in &graph.nodes {
            if a.id < b.id && a.y == b.y {
                let gap = (a.x - b.x).abs() - (a.w + b.w) / 2.0;
                assert!(
                    gap >= params().node_gap - 1e-3,
                    "{} and {} overlap",
                    a.id,
                    b.id
                );
            }
        }
    }
}

#[test]
fn a_parent_that_is_not_earlier_is_refused() {
    let mut graph = demo_view(&params()).unwrap();
    graph.nodes.swap(0, 1);
    assert!(relayout(graph, 24.0, 120.0).is_err());
}

#[test]
fn unknown_params_fields_are_refused() {
    let mut v = serde_json::to_value(params()).unwrap();
    v["zoom"] = serde_json::json!(2);
    assert!(serde_json::from_value::<ViewParams>(v).is_err());
}
