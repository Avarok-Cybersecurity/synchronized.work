//! The Worker's routes. The route table is here and nowhere else.

use worker::{Context, Env, Method, Request, Response, Result, event};

use crate::env_source::EnvSource;
use crate::security::HEADERS;

/// The one ServerDO (design §3.2 Q1): `idFromName("node")`.
pub const SERVER_BINDING: &str = "SERVER";
pub const SERVER_NAME: &str = "node";
/// The site's static assets (`[assets]` in wrangler.toml, built from `site/`).
pub const ASSETS_BINDING: &str = "ASSETS";
/// The deployed commit, set by the deploy job (`--var SW_BUILD_SHA:<sha>`).
pub const VAR_BUILD_SHA: &str = "SW_BUILD_SHA";
/// Where work.avarok.net returns a browser sign-in (sign-in contract §5).
pub const AUTH_RETURN: &str = "/auth/return";

#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    crate::console::init(&env);
    let response = match (req.method(), req.path().as_str()) {
        (_, "/citadel") => {
            let stub = env
                .durable_object(SERVER_BINDING)?
                .id_from_name(SERVER_NAME)?
                .get_stub()?;
            return stub.fetch_with_request(req).await;
        }
        (_, "/healthz") => healthz(&env)?,
        (_, "/.well-known/citadel") => well_known(&env)?,
        (Method::Post, AUTH_RETURN) => auth_return()?,
        _ => env.assets(ASSETS_BINDING)?.fetch_request(req).await?,
    };
    secured(response)
}

/// The asset response's headers are immutable; a copy carries the security headers.
fn secured(response: Response) -> Result<Response> {
    let headers = response.headers().clone();
    for (name, value) in HEADERS {
        headers.set(name, value)?;
    }
    Ok(response.with_headers(headers))
}

/// `{"v":1,"sha":…,"node":"ready"|"unprovisioned"}`, 200 only when the node can start. The
/// site's sign-in entry reads it, so an unprovisioned node is never presented as open. The
/// reason a node is unprovisioned is logged, never served.
fn healthz(env: &Env) -> Result<Response> {
    let sha = env.var(VAR_BUILD_SHA).ok().map(|v| v.to_string());
    let ready = match sw_node::Provisioning::from_source(&EnvSource(env)) {
        Ok(_) => true,
        Err(e) => {
            log::warn!(target: "sw_worker", "/healthz: node unprovisioned: {e}");
            false
        }
    };
    let body = serde_json::json!({
        "v": 1,
        "sha": sha,
        "node": if ready { "ready" } else { "unprovisioned" },
    });
    let mut response = Response::from_json(&body)?.with_status(if ready { 200 } else { 503 });
    response.headers_mut().set("cache-control", "no-store")?;
    Ok(response)
}

/// The issuer POSTs `{code, state}` here. Completing a sign-in needs this browser's
/// Controller key and the node session, which the site does not have yet, so the code is
/// dropped unread (it is useless without the verifier, which never left the browser) and the
/// browser is sent to the page that says so.
fn auth_return() -> Result<Response> {
    let mut response = Response::empty()?.with_status(303);
    response.headers_mut().set("location", AUTH_RETURN)?;
    Ok(response)
}

/// Built from the provisioned key on every request: deterministic, so every isolate serves
/// the same bytes. Unprovisioned, it is not served at all (404), and the reason is logged.
fn well_known(env: &Env) -> Result<Response> {
    let provisioning = match sw_node::Provisioning::from_source(&EnvSource(env)) {
        Ok(p) => p,
        Err(e) => {
            log::error!(target: "sw_worker", "/.well-known/citadel unavailable: {e}");
            return Response::error("not found", 404);
        }
    };
    let json = sw_node::well_known::well_known_json(&provisioning)
        .map_err(|e| worker::Error::RustError(format!("well-known: {e:?}")))?;
    let mut response = Response::from_bytes(json)?;
    let headers = response.headers_mut();
    headers.set("content-type", "application/json")?;
    headers.set("cache-control", "max-age=300")?;
    Ok(response)
}
