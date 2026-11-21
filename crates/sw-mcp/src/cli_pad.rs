//! `sw-mcp pad …`: the scratchpad from a terminal. `wait` is the wake-up path that works in
//! any Claude Code: run it under the Monitor tool or as a background command; it prints the
//! next event as one JSON line and exits, and the agent re-arms it with that event's cursor.

use std::process::ExitCode;

use clap::Subcommand;
use serde_json::Value;

use crate::home::Home;
use crate::ipc::{Request, call};

/// The exit status of a `wait` that timed out with no event (re-arm with the same cursor).
pub const WAIT_TIMEOUT_EXIT: u8 = 3;

#[derive(Debug, Subcommand)]
pub enum PadCmd {
    /// Block until the next event after `--after` and print it as one JSON line.
    Wait {
        /// The cursor of the last event handled; 0 the first time.
        #[arg(long)]
        after: u64,
        /// Give up after this many seconds (exit 3); keep it under the Monitor's deadline.
        #[arg(long)]
        timeout_s: u64,
    },
    /// Notes waiting for an ack, and notes sent with their acks.
    Inbox {
        #[arg(long)]
        project: Option<String>,
    },
    /// Acknowledge a received note.
    Ack {
        note_id: String,
        #[arg(long)]
        reply: Option<String>,
    },
    /// Trust a sender's hand-offs, after comparing fingerprint words with them.
    Trust {
        id_fp: String,
        #[arg(long)]
        label: String,
    },
    /// Stop trusting a sender: their notes are suggestions only again.
    Untrust { id_fp: String },
}

fn print(v: &Value) -> Result<(), String> {
    println!("{}", serde_json::to_string(v).map_err(|e| e.to_string())?);
    Ok(())
}

pub fn run(home: &Home, cmd: PadCmd) -> Result<ExitCode, String> {
    let socket = home.socket();
    match cmd {
        PadCmd::Wait { after, timeout_s } => {
            let v = call(
                &socket,
                &Request::PadWait {
                    after,
                    max_wait_s: timeout_s,
                },
                timeout_s,
            )?;
            print(&v)?;
            if v.get("timeout").and_then(Value::as_bool) == Some(true) {
                return Ok(ExitCode::from(WAIT_TIMEOUT_EXIT));
            }
        }
        PadCmd::Inbox { project } => print(&call(&socket, &Request::PadInbox { project }, 0)?)?,
        PadCmd::Ack { note_id, reply } => {
            print(&call(&socket, &Request::PadAck { note_id, reply }, 0)?)?
        }
        PadCmd::Trust { id_fp, label } => {
            let words = crate::trust::fingerprint_words(&sw_crypto::sha3_256(id_fp.as_bytes()));
            println!("Sender {id_fp} ({label:?})\nFingerprint: {words}");
            println!(
                "Their notes will be shown to your agent as hand-offs from a trusted colleague (still under your own permissions)."
            );
            if !crate::cli::confirm("Did they read you exactly these six words?")? {
                return Err("not trusted".into());
            }
            print(&call(
                &socket,
                &Request::PadTrust {
                    id_fp,
                    label: Some(label),
                },
                0,
            )?)?;
        }
        PadCmd::Untrust { id_fp } => print(&call(
            &socket,
            &Request::PadTrust { id_fp, label: None },
            0,
        )?)?,
    }
    Ok(ExitCode::SUCCESS)
}
