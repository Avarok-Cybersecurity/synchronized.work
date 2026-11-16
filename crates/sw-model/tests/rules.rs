#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

use sw_model::embedding::{DIMS, cosine, embed, tokens};
use sw_model::readiness::{Blocker, Candidate, Readiness, admit, rank, readiness};
use sw_model::visibility::{Level, Viewer, level, notify_subject, redact};
use sw_model::{Id128, IdFp, Progress, Visibility};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

const T: Id128 = Id128([1; 16]);

#[test]
fn readiness_table() {
    let not_done = |_| false;
    let rows: &[(&[Blocker], u16)] = &[
        (&[], 1000),
        (&[Blocker::Human], 0),
        (&[Blocker::Dependency(T)], 200),
        (&[Blocker::Lock], 500),
        (&[Blocker::Disk], 500),
        (&[Blocker::RateLimit], 500),
        (&[Blocker::Other("x".into())], 500),
        (&[Blocker::Lock, Blocker::Dependency(T)], 200),
        (&[Blocker::Lock, Blocker::Human], 0),
    ];
    for (blockers, milli) in rows {
        assert_eq!(
            readiness(blockers, not_done).milli(),
            *milli,
            "{blockers:?}"
        );
    }
    assert_eq!(
        readiness(&[Blocker::Dependency(T)], |t| t == T),
        Readiness::READY
    );
}

#[test]
fn admission_is_readiest_first_then_oldest_and_respects_the_cap() {
    let c = |key: u8, milli, since| Candidate {
        key,
        readiness: Readiness::from_milli(milli).unwrap(),
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
    assert!(Readiness::from_milli(1001).is_none());
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
    let blockers = [Blocker::Human];
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
