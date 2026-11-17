#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

use sw_model::embedding::{DIMS, cosine, embed, tokens};
use sw_model::readiness::{Blocker, BlockerKind, Candidate, Readiness, admit, rank, readiness};
use sw_model::visibility::{Level, Viewer, level, notify_subject, redact};
use sw_model::{Id128, IdFp, Progress, Visibility};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

const T: Id128 = Id128([1; 16]);

fn b(kind: BlockerKind) -> Blocker {
    let task_id = (kind == BlockerKind::Dependency).then(|| "task-t".to_owned());
    Blocker {
        kind,
        text: "why".into(),
        task_id,
    }
}

#[test]
fn readiness_table() {
    use BlockerKind::*;
    let not_done = |_: &str| false;
    let rows: &[(&[BlockerKind], u16)] = &[
        (&[], 1000),
        (&[Human], 0),
        (&[Dependency], 200),
        (&[Lock], 500),
        (&[Disk], 500),
        (&[RateLimit], 500),
        (&[Lock, Dependency], 200),
        (&[Lock, Human], 0),
    ];
    for (kinds, permille) in rows {
        let blockers: Vec<Blocker> = kinds.iter().map(|k| b(*k)).collect();
        assert_eq!(
            readiness(&blockers, not_done).permille(),
            *permille,
            "{kinds:?}"
        );
    }
    assert_eq!(
        readiness(&[b(Dependency)], |t| t == "task-t"),
        Readiness::READY
    );
}

#[test]
fn a_blocker_names_a_task_iff_it_is_a_dependency() {
    assert!(b(BlockerKind::Dependency).is_well_formed());
    assert!(b(BlockerKind::Lock).is_well_formed());
    let mut dep = b(BlockerKind::Dependency);
    dep.task_id = None;
    assert!(!dep.is_well_formed());
    let mut lock = b(BlockerKind::Lock);
    lock.task_id = Some("t".into());
    assert!(!lock.is_well_formed());
    lock.task_id = None;
    lock.text = "x".repeat(281);
    assert!(!lock.is_well_formed());
}

#[test]
fn admission_is_readiest_first_then_oldest_and_respects_the_cap() {
    let c = |key: u8, permille, since| Candidate {
        key,
        readiness: Readiness::from_permille(permille).unwrap(),
        since_ms: since,
    };
    let waiting = [
        c(1, 200, 1),
        c(2, 1000, 9),
        c(3, 1000, 4),
        c(4, 0, 0),
        c(5, 500, 2),
    ];
    let order: Vec<u8> = rank(&waiting).into_iter().map(|c| c.key).collect();
    assert_eq!(order, vec![3, 2, 5, 1, 4]);
    assert_eq!(admit(&waiting, 1, 3), vec![3, 2]);
    assert_eq!(admit(&waiting, 3, 3), Vec::<u8>::new());
}

#[test]
fn readiness_above_one_is_refused() {
    assert!(Readiness::from_permille(1001).is_none());
}

#[test]
fn visibility_matrix() {
    use Level::*;
    use Viewer::*;
    let rows = [
        (Visibility::Owner, TaskOwner, Full),
        (Visibility::Owner, ProjectMember, ExistenceAndPercent),
        (Visibility::Owner, Outsider, Nothing),
        (Visibility::Members, TaskOwner, Full),
        (Visibility::Members, ProjectMember, Full),
        (Visibility::Members, Outsider, Nothing),
        (Visibility::Summary, TaskOwner, Full),
        (Visibility::Summary, ProjectMember, PhaseAndPercent),
        (Visibility::Summary, Outsider, Nothing),
    ];
    for (vis, viewer, expect) in rows {
        assert_eq!(level(vis, viewer), expect, "{vis:?} {viewer:?}");
    }
}

#[test]
fn redaction_drops_bodies_below_full() {
    let p = Progress {
        agent: T,
        phase: "build".into(),
        percent: 40,
        last_line: "secret detail".into(),
        updated_at: 1,
        event_cursor: 2,
    };
    let blockers = [b(BlockerKind::Human)];
    let summary = redact(Level::PhaseAndPercent, &p, &blockers).unwrap();
    assert_eq!(
        (
            summary.phase.as_deref(),
            summary.last_line,
            summary.blockers
        ),
        (Some("build"), None, None)
    );
    let exist = redact(Level::ExistenceAndPercent, &p, &blockers).unwrap();
    assert_eq!((exist.percent, exist.phase), (40, None));
    let full = redact(Level::Full, &p, &blockers).unwrap();
    assert_eq!(full.last_line.as_deref(), Some("secret detail"));
    assert!(redact(Level::Nothing, &p, &blockers).is_none());
    assert_eq!(
        notify_subject(Visibility::Owner, Viewer::ProjectMember, "Ship it").as_deref(),
        Some("a task you work under")
    );
}

#[test]
fn embedding_is_deterministic_normalised_and_similarity_ranks() {
    let a = embed(
        "Port the WAL to wasm32",
        "durable write-ahead log for latticedb",
    );
    assert_eq!(
        a,
        embed(
            "Port the WAL to wasm32",
            "durable write-ahead log for latticedb"
        )
    );
    let norm: f32 = a.iter().map(|x| x * x).sum();
    assert!((norm - 1.0).abs() < 1e-5);
    let near = embed("WAL port for wasm32", "latticedb write-ahead log");
    let far = embed("Redesign the billing page", "Stripe overage toggle");
    assert!(cosine(&a, &near) > cosine(&a, &far));
    assert_eq!(embed("", "the of"), [0f32; DIMS]);
}

#[test]
fn tokens_normalise_nfkc_and_drop_stop_words() {
    assert_eq!(
        tokens("The ＷＡＬ of Lattice-DB"),
        vec!["wal", "lattice", "db"]
    );
}

#[test]
fn embedding_known_answer_is_stable() {
    // Pinned so a change of tokeniser, stop list or hash is a visible, deliberate break
    // (the server refuses any client whose vector differs).
    let v = embed("alpha", "beta");
    let nonzero: Vec<usize> = (0..DIMS).filter(|&i| v[i] != 0.0).collect();
    let signs: Vec<(usize, bool)> = nonzero.iter().map(|&i| (i, v[i] > 0.0)).collect();
    assert_eq!(signs, vec![(4, false), (75, false), (115, true)]);
}

#[test]
fn id_fp_shape() {
    assert!(IdFp::parse("abcdefghijklmnop").is_ok());
    assert!(IdFp::parse("abcdefghijklmno1").is_err(), "1 is not base32");
    assert!(IdFp::parse("ABCDEFGHIJKLMNOP").is_err(), "uppercase");
    assert!(IdFp::parse("abc").is_err());
    assert_eq!(Id128::from_hex(&T.to_hex()).unwrap(), T);
    assert!(Id128::from_hex("zz").is_err());
}
