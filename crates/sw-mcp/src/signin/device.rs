//! Finishing a sign-in on this Device, for the MCP tool and the `sw-mcp sign-in` command alike:
//! the code goes to the pinned issuer over HTTPS, the assertion is checked and stored, and the
//! daemon is told to read it.

use serde_json::{Value, json};

use super::{HttpIssuer, PendingSignIn, complete_sign_in};
use crate::config::Config;
use crate::home::Home;
use crate::ipc::{Request, call_async};

pub async fn complete_here(
    home: &Home,
    config: &Config,
    pending: &PendingSignIn,
    code: &str,
) -> Result<Value, String> {
    let keys = crate::store::load_keys(home)?;
    let device = keys.device_label(&config.device_slug);
    let issuer = &config.sign_in_issuer;
    let port = HttpIssuer::new(issuer, citadel_identity::WELL_KNOWN_FETCH_TIMEOUT_S)?;
    let now_s = crate::daemon::now_ms() / 1000;
    let accepted = complete_sign_in(&port, home, pending, code, &device, issuer, now_s)
        .await
        .map_err(|e| e.to_string())?;
    let reloaded = call_async(home.ipc().clone(), Request::Reload, 0).await;
    Ok(
        json!({ "signed_in": true, "device": accepted.device, "assertion_exp_s": accepted.exp_s,
        "daemon": reloaded.unwrap_or_else(|e| json!({ "not_reloaded": e })) }),
    )
}
