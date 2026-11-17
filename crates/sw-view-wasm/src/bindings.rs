//! The JavaScript surface. Values cross as JSON text (no BigInt is involved: the view carries
//! no CIDs) and errors as thrown `Error`s, never as a silent empty result.

use wasm_bindgen::prelude::*;

use crate::view::ViewParams;

fn js_err(e: impl core::fmt::Debug) -> JsError {
    JsError::new(&format!("{e:?}"))
}

/// `ViewParams` JSON in, the laid-out demo `ViewGraph` JSON out.
#[wasm_bindgen]
pub fn demo_view(params_json: &str) -> Result<String, JsError> {
    let params: ViewParams = serde_json::from_str(params_json).map_err(js_err)?;
    let graph = crate::demo_view(&params).map_err(js_err)?;
    serde_json::to_string(&graph).map_err(js_err)
}

/// The "Sign in with Citadel" link for 32 verifier bytes and 16..=32 state bytes.
#[wasm_bindgen]
pub fn sign_in_link(verifier: &[u8], state: &[u8]) -> Result<String, JsError> {
    let verifier: [u8; 32] = verifier
        .try_into()
        .map_err(|_| JsError::new("verifier must be 32 bytes"))?;
    crate::site::link(&verifier, state).map_err(js_err)
}
