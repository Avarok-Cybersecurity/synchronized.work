//! `sw-mcp`: see the crate docs. Every subcommand reads `config.toml` first and stops with
//! status 2 if it is missing a field (PCND).

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use sw_mcp::cli::{GateCmd, TrustCmd};
use sw_mcp::cli_onboard::{InviteCmd, JoinCmd, ProjectCmd};
use sw_mcp::config::{CONFIG_EXIT, Config, KeyStoreKind};
use sw_mcp::home::Home;
use sw_mcp::hook::HookKind;

#[derive(Debug, Parser)]
#[command(name = "sw-mcp", version, about = "The synchronized.work MCP")]
struct Args {
    /// The state directory; `~/.synchronized` when omitted.
    #[arg(long, global = true)]
    home: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run the Device: the Citadel session to the node, the gate and the socket.
    Daemon,
    /// Serve MCP on stdio to one coding agent.
    Serve {
        /// Push scratchpad events to Claude Code as channel notifications (start Claude Code
        /// with `--dangerously-load-development-channels server:<this server's name>`).
        #[arg(long)]
        channel: bool,
    },
    /// The developer scratchpad: wait for events, read the inbox, ack, trust senders.
    Pad {
        #[command(subcommand)]
        cmd: sw_mcp::cli_pad::PadCmd,
    },
    /// A Claude Code hook (`stop` or `pre-tool-use`), reading the hook JSON on stdin.
    Hook {
        #[arg(value_parser = ["stop", "pre-tool-use"])]
        event: String,
    },
    /// Signals waiting here; approve or refuse the ones approval mode holds.
    Gate {
        #[command(subcommand)]
        cmd: GateCmd,
    },
    /// The trusted Controllers (the local allowlist).
    Trust {
        #[command(subcommand)]
        cmd: TrustCmd,
    },
    /// Remove trusted Controllers whose kid starts with this prefix (8+ hex characters).
    Revoke { kid_prefix: String },
    /// Sign this machine in with your Citadel identity (work.avarok.net), in the browser.
    SignIn,
    /// Create a project, or list the ones this device knows.
    Project {
        #[command(subcommand)]
        cmd: ProjectCmd,
    },
    /// Make an invite for one person, or spend one you were sent.
    Invite {
        #[command(subcommand)]
        cmd: InviteCmd,
    },
    /// The join requests waiting for your answer; accept or refuse them.
    Join {
        #[command(subcommand)]
        cmd: JoinCmd,
    },
    /// A code that links the synchronized.work app in a Citadel workspace to you, for one
    /// project you are a member of (10 minutes, one use).
    LinkCode {
        #[arg(long)]
        project: String,
    },
}

fn load(home: &Home) -> Result<Config, ExitCode> {
    let text = std::fs::read_to_string(home.config()).map_err(|e| {
        eprintln!("sw-mcp: {}: {e}", home.config().display());
        ExitCode::from(CONFIG_EXIT)
    })?;
    Config::parse(&text).map_err(|e| {
        eprintln!("sw-mcp: {e}");
        ExitCode::from(CONFIG_EXIT)
    })
}

fn main() -> ExitCode {
    let args = Args::parse();
    let home = match Home::locate(args.home) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("sw-mcp: {e}");
            return ExitCode::from(CONFIG_EXIT);
        }
    };
    let config = match load(&home) {
        Ok(c) => c,
        Err(code) => return code,
    };
    let result = match args.command {
        Command::Daemon => run_async(daemon(home, config)),
        Command::Serve { channel } => run_async(sw_mcp::serve::run(home, config, channel)),
        Command::Pad { cmd } => {
            return match sw_mcp::cli_pad::run(&home, cmd) {
                Ok(code) => code,
                Err(e) => {
                    eprintln!("sw-mcp: {e}");
                    ExitCode::FAILURE
                }
            };
        }
        Command::Hook { event } => {
            let kind = if event == "stop" {
                HookKind::Stop
            } else {
                HookKind::PreToolUse
            };
            sw_mcp::hook::run(&home, kind).and_then(|out| match out {
                Some(v) => serde_json::to_string(&v)
                    .map(|s| println!("{s}"))
                    .map_err(|e| e.to_string()),
                None => Ok(()),
            })
        }
        Command::Gate { cmd } => sw_mcp::cli::gate(&home, cmd),
        Command::Trust { cmd } => sw_mcp::cli::trust(&home, &config, cmd),
        Command::Revoke { kid_prefix } => sw_mcp::cli::revoke(&home, &kid_prefix),
        Command::SignIn => run_async(sw_mcp::cli_onboard::sign_in(home, config)),
        Command::Project { cmd } => sw_mcp::cli_onboard::project(&home, cmd),
        Command::Invite { cmd } => sw_mcp::cli_onboard::invite(&home, cmd),
        Command::Join { cmd } => sw_mcp::cli_onboard::join(&home, cmd),
        Command::LinkCode { project } => sw_mcp::cli_onboard::link_code(&home, project),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("sw-mcp: {e}");
            ExitCode::FAILURE
        }
    }
}

async fn daemon(home: Home, config: Config) -> Result<(), String> {
    home.ensure_layout().map_err(|e| e.to_string())?;
    sw_mcp::logging::init(&home, config.log_level)?;
    match config.keystore {
        KeyStoreKind::File => log::warn!(
            "keystore = \"file\": Device keys and the Citadel password are protected by {} only",
            sw_mcp::home::PROTECTION
        ),
    }
    sw_mcp::daemon::Daemon::open(home, config)?.run().await
}

fn run_async(work: impl std::future::Future<Output = Result<(), String>>) -> Result<(), String> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?
        .block_on(work)
}
