//! The Worker's routes. The route table is here and nowhere else.

use worker::{Context, Env, Request, Response, Result, event};

use crate::env_source::EnvSource;

/// The one ServerDO (design §3.2 Q1): `idFromName("node")`.
pub const SERVER_BINDING: &str = "SERVER";
pub const SERVER_NAME: &str = "node";
/// The deployed commit, set by the deploy job (`--var SW_BUILD_SHA:<sha>`).
pub const VAR_BUILD_SHA: &str = "SW_BUILD_SHA";

#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    crate::console::init(&env);
    match req.path().as_str() {
        "/healthz" => healthz(&env),
        "/.well-known/citadel" => well_known(&env),
        "/citadel" => {
            let stub = env
                .durable_object(SERVER_BINDING)?
                .id_from_name(SERVER_NAME)?
                .get_stub()?;
            stub.fetch_with_request(req).await
        }
        _ => Response::error("not found", 404),
    }
}

fn healthz(env: &Env) -> Result<Response> {
    match env.var(VAR_BUILD_SHA) {
        Ok(sha) => Response::ok(sha.to_string()),
        Err(_) => Response::error(format!("{VAR_BUILD_SHA} is not provisioned"), 503),
    }
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
