//! The local, human-facing subcommands: `trust`, `revoke` and `gate`. Trust is decided here,
//! in this terminal, after the person compares fingerprints (design §5.4); the daemon is then
//! told to re-read the allowlist.

use std::io::{BufRead as _, Write as _};

use clap::Subcommand;
use data_encoding::BASE64URL_NOPAD;
use serde_json::Value;

use crate::config::Config;
use crate::home::Home;
use crate::ipc::{Request, call};
use crate::signin::load_assertion;
use crate::trust::{fingerprint_words, kid_prefix};

#[derive(Debug, Subcommand)]
pub enum TrustCmd {
    /// List the trusted Controllers.
    List,
    /// Trust a Controller of the signed-in Person, after comparing its fingerprint.
    Add {
        /// The Controller's CID on the node.
        #[arg(long)]
        cid: u64,
        /// Its ML-DSA-65 signing key, base64url (as the site shows it).
        #[arg(long)]
        key: String,
        /// A name for it in prompts and logs.
        #[arg(long)]
        label: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum GateCmd {
    /// Signals waiting here, with whether each needs an approval.
    List,
    /// Let a signal held by approval mode reach its agent.
    Approve { signal_id: String },
    /// Refuse a held signal; the Controller sees `refused: owner_refused`.
    Refuse { signal_id: String },
}

pub(crate) fn confirm(prompt: &str) -> Result<bool, String> {
    print!("{prompt} Type yes to confirm: ");
    std::io::stdout().flush().map_err(|e| e.to_string())?;
    let mut line = String::new();
    std::io::stdin()
        .lock()
        .read_line(&mut line)
        .map_err(|e| e.to_string())?;
    Ok(line.trim() == "yes")
}

/// Tells a running daemon to re-read its files; a stopped one reads them when it starts.
fn reload(home: &Home) {
    match call(&home.socket(), &Request::Reload, 0) {
        Ok(_) => println!("the daemon has the new allowlist"),
        Err(e) => println!("{e}; it will read the allowlist when it starts"),
    }
}

pub fn trust(home: &Home, config: &Config, cmd: TrustCmd) -> Result<(), String> {
    let keys = crate::store::load_keys(home)?;
    let mut list = crate::store::load_trust(home, &keys)?;
    match cmd {
        TrustCmd::List => {
            for c in list.entries() {
                let state = if c.paused { "PAUSED" } else { "active" };
                println!(
                    "{}  {state}  cid {}  {:?}  {}",
                    kid_prefix(&c.kid),
                    c.cid,
                    c.label,
                    fingerprint_words(&c.kid)
                );
            }
            Ok(())
        }
        TrustCmd::Add { cid, key, label } => {
            let assertion =
                load_assertion(home, crate::daemon::now_ms() / 1000).map_err(|e| e.to_string())?;
            let owner = crate::adapters::CitadelIdentity;
            let owner = sw_protocol::ports::IdentityPort::id_fp(&owner, &assertion.identity_pk)
                .map_err(|e| format!("{e:?}"))?;
            let pk = BASE64URL_NOPAD
                .decode(key.as_bytes())
                .map_err(|e| format!("--key: {e}"))?;
            let kid = sw_crypto::kid_of(&pk);
            println!("Controller {label:?} (cid {cid}) of {}", owner.as_str());
            println!("Fingerprint: {}", fingerprint_words(&kid));
            println!(
                "Its prompts will need your approval here (`sw-mcp gate approve`) for the first {} hours.",
                config.approval_mode_new_controller_hours
            );
            if !confirm("Does the Controller's screen show exactly these six words?")? {
                return Err("not trusted".into());
            }
            list.trust(&pk, cid, owner, label, crate::daemon::now_ms())
                .map_err(|e| e.to_string())?;
            crate::store::save_trust(home, &keys, &list)?;
            println!("trusted {}", kid_prefix(&kid));
            reload(home);
            Ok(())
        }
    }
}

pub fn revoke(home: &Home, prefix: &str) -> Result<(), String> {
    let keys = crate::store::load_keys(home)?;
    let mut list = crate::store::load_trust(home, &keys)?;
    let gone = list.revoke_prefix(prefix);
    if gone.is_empty() {
        return Err(format!(
            "no trusted controller's kid starts with {prefix:?} (give at least 8 hex characters)"
        ));
    }
    crate::store::save_trust(home, &keys, &list)?;
    for kid in &gone {
        println!("revoked {}", kid_prefix(kid));
    }
    reload(home);
    Ok(())
}

pub fn gate(home: &Home, cmd: GateCmd) -> Result<(), String> {
    let request = match cmd {
        GateCmd::List => Request::GateList,
        GateCmd::Approve { signal_id } => Request::GateApprove { signal_id },
        GateCmd::Refuse { signal_id } => Request::GateRefuse { signal_id },
    };
    let answer: Value = call(&home.socket(), &request, 0)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&answer).map_err(|e| e.to_string())?
    );
    Ok(())
}
