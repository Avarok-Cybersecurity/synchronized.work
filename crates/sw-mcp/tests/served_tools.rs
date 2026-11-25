#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! The MCP server offers every tool router it composes, not only the one `#[tool_router]` names
//! `tool_router`: the handler must serve `SwServer::tool_router` (the sum), which rmcp's default
//! for `#[tool_handler]` does not.

mod support;

use rmcp::handler::server::ServerHandler;
use sw_mcp::home::Home;
use sw_mcp::serve::SwServer;

#[test]
fn every_composed_router_is_served() {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("served-tools");
    let _ = std::fs::remove_dir_all(&dir);
    let home = Home::locate(Some(dir)).unwrap();
    let server = SwServer::new(home, support::device::config(1), "/".into(), false);
    for name in [
        "attach",
        "sign_in_start",
        "project_status",
        "scratchpad_post",
        "project_create",
    ] {
        assert!(server.get_tool(name).is_some(), "{name} is not served");
    }
}
