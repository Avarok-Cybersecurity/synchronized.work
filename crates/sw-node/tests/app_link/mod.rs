//! The Citadel App's `citadel.session` to the node, in memory: every byte the App sends is the
//! node's input and every byte the node sends that session is the App's, each logged in the
//! order it crossed as a [`Step`], so a test can assert on the flow or commit it as a transcript.
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::VecDeque;

use crate::app_support::FixedAdvance;
use crate::feed_support::Sent;
use crate::support::{NOW, World};
use sw_app_core::{App, Effect, HostSessionStatus, Input};
use sw_protocol::payload::{LinkOp, Payload};
use sw_protocol::signed::SwMessage;

/// The App account's CID: the agent's session to the node.
pub const APP: u64 = 7000;

/// One message on the session, in the order it crossed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// The App sent these bytes to the node.
    Expect(Vec<u8>),
    /// The node sent these bytes to the App's session.
    Deliver(Vec<u8>),
}

impl Step {
    pub fn bytes(&self) -> &[u8] {
        match self {
            Step::Expect(b) | Step::Deliver(b) => b,
        }
    }

    /// What the message is, by its payload: `hello`, `redeem`, `receipt`, `event`, …
    pub fn what(&self) -> String {
        match SwMessage::from_bytes(self.bytes()).unwrap() {
            SwMessage::Unsigned(Payload::Hello(_)) => "hello".into(),
            SwMessage::Unsigned(Payload::Link(l)) if l.op == LinkOp::Redeem => "redeem".into(),
            SwMessage::Unsigned(Payload::Subscribe(_)) => "subscribe".into(),
            SwMessage::Unsigned(Payload::PadReceipt(_)) => "receipt".into(),
            SwMessage::Unsigned(Payload::Event(_)) => "event".into(),
            SwMessage::Unsigned(Payload::Notify(_)) => "notify".into(),
            SwMessage::Unsigned(p) => format!("typ-{}", p.typ()),
            SwMessage::Signed { typ, .. } => format!("signed-typ-{typ}"),
        }
    }
}

pub struct Session {
    pub app: App<FixedAdvance>,
    pub w: World,
    pub steps: Vec<Step>,
}

impl Session {
    pub fn new(app: App<FixedAdvance>, w: World) -> Self {
        Self {
            app,
            w,
            steps: Vec::new(),
        }
    }

    /// The agent opens the session; the App starts with this saved link and is told it is
    /// connected. Returns every effect the App asked of its host other than sends.
    pub fn open(&mut self, record: Option<Vec<u8>>) -> Vec<Effect> {
        let effects = self.w.node.connected(APP, NOW);
        let sent = self.w.apply(effects);
        let mut out = self.app.start();
        out.extend(self.app.handle(Input::LinkLoaded { record }));
        out.extend(self.app.handle(Input::SessionOpened {
            cid: APP,
            status: HostSessionStatus::Connected,
        }));
        let mut host = self.run(out);
        host.extend(self.deliver(sent));
        host
    }

    /// Each key pressed in turn, then `Enter`.
    pub fn type_line(&mut self, text: &str) -> Vec<Effect> {
        let mut out = Vec::new();
        for key in text.chars().map(String::from).chain(["Enter".to_owned()]) {
            out.extend(self.app.handle(Input::Key { key }));
        }
        self.run(out)
    }

    /// Carries the App's sends to the node and the node's answers back, until both are quiet.
    pub fn run(&mut self, effects: Vec<Effect>) -> Vec<Effect> {
        let mut host = Vec::new();
        let mut to_node = VecDeque::new();
        self.take(effects, &mut host, &mut to_node);
        self.drain(&mut host, &mut to_node);
        host
    }

    /// What the node sent because someone else acted: the App's share is delivered.
    pub fn deliver(&mut self, sent: Sent) -> Vec<Effect> {
        let mut host = Vec::new();
        let mut to_node = VecDeque::new();
        self.hand_over(sent, &mut host, &mut to_node);
        self.drain(&mut host, &mut to_node);
        host
    }

    fn drain(&mut self, host: &mut Vec<Effect>, to_node: &mut VecDeque<Vec<u8>>) {
        while let Some(body) = to_node.pop_front() {
            let effects = self.w.node.received(APP, &body, NOW).unwrap();
            let sent = self.w.apply(effects);
            self.hand_over(sent, host, to_node);
        }
    }

    fn hand_over(&mut self, sent: Sent, host: &mut Vec<Effect>, to_node: &mut VecDeque<Vec<u8>>) {
        for (to, body) in sent {
            if to == APP {
                self.steps.push(Step::Deliver(body.clone()));
                let effects = self.app.handle(Input::SessionMessage { body });
                self.take(effects, host, to_node);
            }
        }
    }

    /// Sends are logged as the App makes them, before the node has read them.
    fn take(&mut self, effects: Vec<Effect>, host: &mut Vec<Effect>, to: &mut VecDeque<Vec<u8>>) {
        for effect in effects {
            match effect {
                Effect::SessionSend { body } => {
                    self.steps.push(Step::Expect(body.clone()));
                    to.push_back(body);
                }
                other => host.push(other),
            }
        }
    }
}

pub fn notifies(effects: &[Effect]) -> usize {
    effects
        .iter()
        .filter(|e| matches!(e, Effect::Notify(_)))
        .count()
}
