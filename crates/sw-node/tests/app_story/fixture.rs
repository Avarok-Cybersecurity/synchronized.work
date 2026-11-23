//! The committed transcript's JSON: keys in sorted order (serde_json's map), bytes as lowercase
//! hex, the CID as a decimal string (it is a `u64`, which a browser's numbers cannot hold).

use data_encoding::HEXLOWER;
use serde_json::{Value, json};

use super::{CODE, GENERATED_BY, Recorded};
use crate::app_link::{APP, Step};

const ABOUT: &str = "What one citadel.session carried between the synchronized.work App (sw-app, \
     mounted with `context`) and the real node core on a fresh install: the App says HELLO, the \
     member types `link_code` and Enter on the link screen (just before the `redeem` step), the \
     node's receipt links it, the App subscribes and catches up to the live tree; `tail` is \
     played later and admits a second Person to Task `t`, which the App shows as one \
     notification.";

/// The transcript as a consumer reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fixture {
    pub acting_username: String,
    pub install_id: String,
    pub node_id: String,
    pub app_id: String,
    pub server: String,
    pub client_build: String,
    pub link_code: String,
    pub cid: u64,
    /// Each message with its `what`.
    pub steps: Vec<(Step, String)>,
    pub tail: Vec<(Step, String)>,
    pub project_name: String,
    pub after_catch_up: Vec<String>,
    pub after_join: Vec<String>,
    pub title: String,
    pub body: String,
}

impl Fixture {
    /// What [`super::record`] produced, as the fixture holds it; the story has one notification.
    pub fn of(r: &Recorded) -> Self {
        let [notified] = r.notified.as_slice() else {
            panic!("the story notifies once, not {:?}", r.notified);
        };
        let named = |steps: &[Step]| steps.iter().map(|s| (s.clone(), s.what())).collect();
        Self {
            acting_username: r.ctx.acting_username.clone(),
            install_id: r.ctx.install_id.clone(),
            node_id: r.ctx.node_id.clone(),
            app_id: r.ctx.app_id.clone(),
            server: sw_app::config::SERVER.to_owned(),
            client_build: sw_app::config::CLIENT_BUILD.to_owned(),
            link_code: CODE.to_owned(),
            cid: APP,
            steps: named(&r.steps),
            tail: named(&r.tail),
            project_name: r.project_name.clone(),
            after_catch_up: r.after_catch_up.clone(),
            after_join: r.after_join.clone(),
            title: notified.title.clone(),
            body: notified.body.clone(),
        }
    }

    pub fn to_json(&self) -> String {
        let value = json!({
            "about": ABOUT,
            "generated_by": GENERATED_BY,
            "context": {
                "acting_username": self.acting_username,
                "install_id": self.install_id,
                "node_id": self.node_id,
                "app_id": self.app_id,
                "server": self.server,
                "client_build": self.client_build,
            },
            "link_code": self.link_code,
            "session": { "cid": self.cid.to_string() },
            "steps": steps_json(&self.steps),
            "tail": steps_json(&self.tail),
            "expected": {
                "project_name": self.project_name,
                "labels_after_catch_up": self.after_catch_up,
                "labels_after_join": self.after_join,
                "notification": { "title": self.title, "body": self.body },
            },
        });
        let mut text = serde_json::to_string_pretty(&value).unwrap();
        text.push('\n');
        text
    }

    pub fn from_json(text: &str) -> Self {
        let v: Value = serde_json::from_str(text).unwrap();
        let s = |v: &Value| v.as_str().unwrap().to_owned();
        let list = |v: &Value| v.as_array().unwrap().iter().map(s).collect();
        let (c, e) = (&v["context"], &v["expected"]);
        Self {
            acting_username: s(&c["acting_username"]),
            install_id: s(&c["install_id"]),
            node_id: s(&c["node_id"]),
            app_id: s(&c["app_id"]),
            server: s(&c["server"]),
            client_build: s(&c["client_build"]),
            link_code: s(&v["link_code"]),
            cid: s(&v["session"]["cid"]).parse().unwrap(),
            steps: steps_of(&v["steps"]),
            tail: steps_of(&v["tail"]),
            project_name: s(&e["project_name"]),
            after_catch_up: list(&e["labels_after_catch_up"]),
            after_join: list(&e["labels_after_join"]),
            title: s(&e["notification"]["title"]),
            body: s(&e["notification"]["body"]),
        }
    }
}

fn steps_json(steps: &[(Step, String)]) -> Value {
    steps
        .iter()
        .map(|(step, what)| {
            let hex = HEXLOWER.encode(step.bytes());
            match step {
                Step::Expect(_) => json!({ "expect": hex, "what": what }),
                Step::Deliver(_) => json!({ "deliver": hex, "what": what }),
            }
        })
        .collect()
}

fn steps_of(v: &Value) -> Vec<(Step, String)> {
    let bytes = |h: &Value| HEXLOWER.decode(h.as_str().unwrap().as_bytes()).unwrap();
    v.as_array()
        .unwrap()
        .iter()
        .map(|o| {
            let step = match (o.get("expect"), o.get("deliver")) {
                (Some(h), None) => Step::Expect(bytes(h)),
                (None, Some(h)) => Step::Deliver(bytes(h)),
                _ => panic!("a step is either `expect` or `deliver`: {o}"),
            };
            (step, o["what"].as_str().unwrap().to_owned())
        })
        .collect()
}
