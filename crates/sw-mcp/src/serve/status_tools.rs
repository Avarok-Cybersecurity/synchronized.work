//! The status tools: what this Device last saw of a project, its tasks and agents.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::{ErrorData as McpError, tool, tool_router};

use super::SwServer;
use super::inputs::{AgentInput, ProjectInput, TailInput, TaskInput};
use crate::ipc::Request;

#[tool_router(router = status_router, vis = "pub(super)")]
impl SwServer {
    #[tool(
        name = "project_status",
        description = "A project's tasks and agents as this Device last saw them."
    )]
    async fn project_status(
        &self,
        Parameters(i): Parameters<ProjectInput>,
    ) -> Result<CallToolResult, McpError> {
        self.ask(Request::ProjectStatus { project: i.project }, 0)
            .await
    }

    #[tool(
        name = "task_status",
        description = "One task and the agents working on it."
    )]
    async fn task_status(
        &self,
        Parameters(i): Parameters<TaskInput>,
    ) -> Result<CallToolResult, McpError> {
        self.ask(
            Request::TaskStatus {
                project: i.project,
                task: i.task,
            },
            0,
        )
        .await
    }

    #[tool(
        name = "agent_status",
        description = "One agent's lane, blockers and recent progress."
    )]
    async fn agent_status(
        &self,
        Parameters(i): Parameters<AgentInput>,
    ) -> Result<CallToolResult, McpError> {
        self.ask(
            Request::AgentStatus {
                agent_id: i.agent_id,
            },
            0,
        )
        .await
    }

    #[tool(
        name = "progress_tail",
        description = "The latest progress lines of a project's agents (untrusted text)."
    )]
    async fn progress_tail(
        &self,
        Parameters(i): Parameters<TailInput>,
    ) -> Result<CallToolResult, McpError> {
        self.ask(
            Request::ProgressTail {
                project: i.project,
                task: i.task,
                limit: i.limit,
            },
            0,
        )
        .await
    }
}
