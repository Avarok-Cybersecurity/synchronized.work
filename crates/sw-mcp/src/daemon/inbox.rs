//! Verified signals waiting for an agent (or for a person's approval), one 0600 file each under
//! `inbox/`. A signal is written here before its `ACK{delivered}` goes out (design §4.5), and
//! removed when the agent acks it, a person refuses it, or it expires.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::gate::{Admitted, SignalKind};
use crate::home::{read_private, remove_if_present, write_private};
use crate::trust::fingerprint_words;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueuedSignal {
    /// Hex of `SHA3-256(Signed)`.
    pub signal_id: String,
    pub controller_cid: u64,
    pub controller_label: String,
    pub controller_fingerprint: String,
    pub iat: u64,
    pub exp: u64,
    /// The agent this signal is for.
    pub agent_id: String,
    pub task_id: Option<String>,
    /// The prompt's text, or `pause` / `resume` / `stop` for a control.
    pub text: String,
    pub control: bool,
    pub interrupt: bool,
    /// `false` while the Controller is inside its approval window and no person approved it.
    pub approved: bool,
    /// Handed to the agent (by `await_signal` or the hook), not yet acked.
    pub handed_out: bool,
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

impl QueuedSignal {
    pub fn new(a: &Admitted, agent_id: String) -> Self {
        use sw_protocol::payload::{ControlOp, PromptMode};
        let (task_id, text, control, interrupt) = match &a.kind {
            SignalKind::Prompt(p) => (
                Some(p.task_id.clone()),
                p.text.clone(),
                false,
                p.mode == PromptMode::Interrupt,
            ),
            SignalKind::Control(c) => {
                let op = match c.op {
                    ControlOp::Pause => "pause",
                    ControlOp::Resume => "resume",
                    _ => "stop",
                };
                (None, op.to_owned(), true, true)
            }
        };
        Self {
            signal_id: hex(&a.signal_id),
            controller_cid: a.controller_cid,
            controller_label: a.controller_label.clone(),
            controller_fingerprint: fingerprint_words(&a.kid),
            iat: a.iat,
            exp: a.exp,
            agent_id,
            task_id,
            text,
            control,
            interrupt,
            approved: !a.needs_approval,
            handed_out: false,
        }
    }

    /// How the prompt is shown to the agent: named as the owner's instruction, with who sent
    /// it, when, and how to acknowledge it (design §9.3).
    pub fn as_instruction(&self) -> String {
        let what = if self.control {
            format!("Control from your owner: {}. Do this now.", self.text)
        } else {
            format!("Prompt from your owner:\n\n{}", self.text)
        };
        format!(
            "[synchronized.work] Verified remote instruction from your owner's trusted controller \
             \"{}\" (fingerprint: {}), signed at {} ms since the epoch, for task {}.\n\n{what}\n\n\
             When finished, call the sw-mcp tool ack_signal with signal_id {} and executed true \
             (or false with a reason).",
            self.controller_label,
            self.controller_fingerprint,
            self.iat,
            self.task_id.as_deref().unwrap_or("(any)"),
            self.signal_id,
        )
    }
}

#[derive(Debug)]
pub struct Inbox {
    dir: PathBuf,
    signals: BTreeMap<String, QueuedSignal>,
}

impl Inbox {
    /// Loads every queued signal from `dir`; a file that does not parse stops the daemon.
    pub fn load(dir: PathBuf) -> Result<Self, String> {
        let mut signals = BTreeMap::new();
        let entries = std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        for entry in entries {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.extension().is_none_or(|x| x != "json") {
                continue;
            }
            let bytes = read_private(&path)
                .map_err(|e| e.to_string())?
                .unwrap_or_default();
            let s: QueuedSignal =
                serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
            signals.insert(s.signal_id.clone(), s);
        }
        Ok(Self { dir, signals })
    }

    fn path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.json"))
    }

    pub fn len(&self) -> usize {
        self.signals.len()
    }

    pub fn is_empty(&self) -> bool {
        self.signals.is_empty()
    }

    pub fn put(&mut self, s: QueuedSignal) -> Result<(), String> {
        let bytes = serde_json::to_vec(&s).map_err(|e| e.to_string())?;
        write_private(&self.path(&s.signal_id), &bytes).map_err(|e| e.to_string())?;
        self.signals.insert(s.signal_id.clone(), s);
        Ok(())
    }

    pub fn take(&mut self, id: &str) -> Result<Option<QueuedSignal>, String> {
        remove_if_present(&self.path(id)).map_err(|e| e.to_string())?;
        Ok(self.signals.remove(id))
    }

    pub fn get(&self, id: &str) -> Option<&QueuedSignal> {
        self.signals.get(id)
    }

    pub fn all(&self) -> impl Iterator<Item = &QueuedSignal> {
        self.signals.values()
    }

    /// The oldest approved signal for `agent_id` not yet handed out (controls first).
    pub fn next_for(&self, agent_id: &str, interrupt_only: bool) -> Option<&QueuedSignal> {
        let ready = |s: &&QueuedSignal| {
            s.agent_id == agent_id
                && s.approved
                && !s.handed_out
                && (!interrupt_only || s.interrupt)
        };
        let mut ready: Vec<&QueuedSignal> = self.signals.values().filter(ready).collect();
        ready.sort_by_key(|s| (!s.control, s.iat));
        ready.into_iter().next()
    }

    /// Ids of signals whose `exp` has passed.
    pub fn expired(&self, now_ms: u64) -> Vec<String> {
        self.signals
            .values()
            .filter(|s| s.exp <= now_ms)
            .map(|s| s.signal_id.clone())
            .collect()
    }
}
