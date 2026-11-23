//! The server identity challenge's known answers: `fixtures/server-identity-challenge/
//! frames.json` (the workspace protocol's frames, produced with the workspace types; how is in
//! its `$generated_by`) and `servers["main.work.avarok.net"]` of `fixtures/identity-kat.json`
//! (C0), whose key, cid, nonce and signature the frames were made with.

use citadel_identity::{CanonicalDomain, MlDsa65Pk, ServerKey, canonical_domain};
use serde_json::Value as Json;
use sw_node::core::ServerIdentity;

const FRAMES: &str = include_str!("../../../../fixtures/server-identity-challenge/frames.json");
const KAT: &str = include_str!("../../../../fixtures/identity-kat.json");

pub const KAT_SERVER: &str = "main.work.avarok.net";

fn hex(text: &Json) -> Vec<u8> {
    data_encoding::HEXLOWER
        .decode(text.as_str().unwrap().as_bytes())
        .unwrap()
}

pub struct Frames {
    pub request: Vec<u8>,
    /// The answers, per domain, for the KAT key, cid and nonce.
    pub answers: Json,
    pub near_misses: Vec<(String, Vec<u8>)>,
}

pub fn frames() -> Frames {
    let j: Json = serde_json::from_str(FRAMES).unwrap();
    let near_misses = j["near_misses"].as_array().unwrap().iter();
    Frames {
        request: j["request"].as_str().unwrap().as_bytes().to_vec(),
        answers: j["answers"].clone(),
        near_misses: near_misses
            .map(|m| {
                let why = m["why"].as_str().unwrap().to_owned();
                (why, m["frame"].as_str().unwrap().as_bytes().to_vec())
            })
            .collect(),
    }
}

impl Frames {
    pub fn answer(&self, domain: &str) -> Vec<u8> {
        self.answers[domain].as_str().unwrap().as_bytes().to_vec()
    }
}

pub struct KatServer {
    pub seed: [u8; 32],
    pub cid: u64,
    pub nonce: [u8; 32],
    pub sig: Vec<u8>,
}

pub fn kat_server() -> KatServer {
    let j: Json = serde_json::from_str(KAT).unwrap();
    let s = &j["servers"][KAT_SERVER];
    assert_eq!(s["challenge"]["domain"], KAT_SERVER);
    KatServer {
        seed: hex(&s["key"]["seed_hex"]).try_into().unwrap(),
        cid: s["challenge"]["cid"].as_str().unwrap().parse().unwrap(),
        nonce: hex(&s["challenge"]["nonce_hex"]).try_into().unwrap(),
        sig: hex(&s["challenge"]["sig_hex"]),
    }
}

pub fn domain(name: &str) -> CanonicalDomain {
    canonical_domain(name).unwrap()
}

/// The KAT server's key under `name`.
pub fn kat_identity(name: &str) -> ServerIdentity {
    ServerIdentity::new(domain(name), ServerKey::from_seed(&kat_server().seed))
}

/// A `ServerIdentity` answer as the agent reads it: exactly `domain`, `pk` and `sig`.
#[derive(Debug, PartialEq, Eq)]
pub struct Answer {
    pub domain: String,
    pub pk: Vec<u8>,
    pub sig: Vec<u8>,
}

fn bytes_of(j: &Json) -> Vec<u8> {
    let items = j.as_array().unwrap().iter();
    items
        .map(|b| u8::try_from(b.as_u64().unwrap()).unwrap())
        .collect()
}

pub fn parse_answer(frame: &[u8]) -> Answer {
    let j: Json = serde_json::from_slice(frame).unwrap();
    let a = j["Response"]["ServerIdentity"].as_object().unwrap();
    assert_eq!(j.as_object().unwrap().len(), 1);
    assert_eq!(a.len(), 3, "domain, pk and sig only");
    Answer {
        domain: a["domain"].as_str().unwrap().to_owned(),
        pk: bytes_of(&a["pk"]),
        sig: bytes_of(&a["sig"]),
    }
}

/// Whether `answer` is the agent's accepted answer for session `cid` and `nonce` under `name`.
pub fn verifies(answer: &Answer, name: &str, cid: u64, nonce: &[u8; 32]) -> bool {
    let pk = MlDsa65Pk::from_bytes(&answer.pk).unwrap();
    answer.domain == name
        && citadel_identity::verify_server_challenge(&pk, &domain(name), cid, nonce, &answer.sig)
            .is_ok()
}
