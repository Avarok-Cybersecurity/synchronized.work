//! The onboarding commands for a terminal: sign in, create a project, invite a person, spend
//! an invite, and answer the people who spent one. Each is one request to the running daemon,
//! except `sign-in`, whose verifier lives in this process until the code comes back.

use std::io::{BufRead as _, Write as _};

use clap::Subcommand;
use serde_json::Value;

use crate::adapters::OsEntropy;
use crate::config::Config;
use crate::daemon::pad_ask_timeout_s;
use crate::home::Home;
use crate::ipc::{Request, call};

#[derive(Debug, Subcommand)]
pub enum ProjectCmd {
    /// Create a project you own (its scratchpad keeps notes for 7 days).
    Create { name: String },
    /// The projects this device created or asked to join.
    List,
}

#[derive(Debug, Subcommand)]
pub enum InviteCmd {
    /// A one-time invite code for one person; send it to them privately.
    Create {
        project: String,
        /// How long it can be spent: 1 to 168 hours.
        #[arg(long)]
        valid_hours: u64,
    },
    /// Ask to join a project with the invite code you were sent.
    Redeem {
        code: String,
        /// A short note for the owner.
        #[arg(long, default_value = "")]
        note: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum JoinCmd {
    /// Requests waiting for your answer.
    List {
        #[arg(long)]
        project: Option<String>,
    },
    /// Admit the person who made this request.
    Accept { request_id: String },
    /// Refuse it.
    Refuse {
        request_id: String,
        #[arg(long)]
        reason: Option<String>,
    },
}

fn ask(home: &Home, request: Request) -> Result<(), String> {
    let v: Value = call(&home.socket(), &request, pad_ask_timeout_s())?;
    println!(
        "{}",
        serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?
    );
    Ok(())
}

pub fn project(home: &Home, cmd: ProjectCmd) -> Result<(), String> {
    ask(
        home,
        match cmd {
            ProjectCmd::Create { name } => Request::ProjectCreate { name },
            ProjectCmd::List => Request::ProjectList,
        },
    )
}

pub fn invite(home: &Home, cmd: InviteCmd) -> Result<(), String> {
    ask(
        home,
        match cmd {
            InviteCmd::Create {
                project,
                valid_hours,
            } => Request::InviteCreate {
                project,
                valid_hours,
            },
            InviteCmd::Redeem { code, note } => Request::InviteRedeem { code, note },
        },
    )
}

pub fn join(home: &Home, cmd: JoinCmd) -> Result<(), String> {
    let request = match cmd {
        JoinCmd::List { project } => Request::JoinRequests { project },
        JoinCmd::Accept { request_id } => Request::JoinAnswer {
            request_id,
            accept: true,
            reason: None,
        },
        JoinCmd::Refuse { request_id, reason } => Request::JoinAnswer {
            request_id,
            accept: false,
            reason,
        },
    };
    ask(home, request)
}

/// Prints the link and match words, reads the one-time code from this terminal, and finishes.
pub async fn sign_in(home: Home, config: Config) -> Result<(), String> {
    let mut entropy = OsEntropy;
    let (pending, link) =
        crate::signin::start(entropy.array(), entropy.array(), &config.sign_in_issuer);
    println!("Open this link in your browser:\n\n  {}\n", link.url);
    println!(
        "Check that the page shows these three words: {}",
        link.match_words.join(" ")
    );
    print!("Sign in there, then type the one-time code it shows (XXXX-XXXX): ");
    std::io::stdout().flush().map_err(|e| e.to_string())?;
    let mut code = String::new();
    std::io::stdin()
        .lock()
        .read_line(&mut code)
        .map_err(|e| e.to_string())?;
    let done = crate::signin::complete_here(&home, &config, &pending, code.trim()).await?;
    println!(
        "{}",
        serde_json::to_string_pretty(&done).map_err(|e| e.to_string())?
    );
    Ok(())
}
