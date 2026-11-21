//! The sign-in tools (contract §1): the verifier stays in this process's memory between them.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::{ErrorData as McpError, tool, tool_router};
use serde_json::json;

use super::inputs::SignInCompleteInput;
use super::{SwServer, reply};
use crate::adapters::OsEntropy;
use crate::signin;

#[tool_router(router = sign_in_router, vis = "pub(super)")]
impl SwServer {
    #[tool(
        name = "sign_in_start",
        description = "Begin signing this machine in to synchronized.work with your portable Citadel identity. Returns a link to open in your browser and three match words; the page must show the same three words. After signing in there, paste the one-time code it shows into sign_in_complete."
    )]
    async fn sign_in_start(&self) -> Result<CallToolResult, McpError> {
        let mut entropy = OsEntropy;
        let (pending, link) = signin::start(
            entropy.array(),
            entropy.array(),
            &self.config.sign_in_issuer,
        );
        *self.pending.lock().await = Some(pending);
        reply(Ok(json!({
            "open_this_link": link.url,
            "match_words": link.match_words.join(" "),
            "then": "Check that the page shows the same three words, sign in, and pass the code it shows to sign_in_complete.",
        })))
    }

    #[tool(
        name = "sign_in_complete",
        description = "Finish signing in with the one-time code (XXXX-XXXX) the sign-in page showed. Never share that code with anyone."
    )]
    async fn sign_in_complete(
        &self,
        Parameters(input): Parameters<SignInCompleteInput>,
    ) -> Result<CallToolResult, McpError> {
        let mut guard = self.pending.lock().await;
        let Some(pending) = guard.as_ref() else {
            return reply(Err(
                "no sign-in in progress: call sign_in_start first".into()
            ));
        };
        let done = signin::complete_here(&self.home, &self.config, pending, &input.code).await;
        if done.is_ok() {
            *guard = None;
        }
        reply(done)
    }
}
