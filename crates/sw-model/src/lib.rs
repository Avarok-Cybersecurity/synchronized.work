//! The synchronized.work domain (design rev 3 §2): one Project is a tree
//! `Project → Task (→ Task) → Person → Agent → Progress`, and every decision about it —
//! which tier a Task is in, which lane or join request is admitted next, what a viewer may
//! see — is a pure function here. Adapters (the ProjectDO, `sw-mcp`, the site) only store
//! and transmit what these functions decide.

pub mod embedding;
mod ids;
pub mod readiness;
pub mod tier;
mod tree;
pub mod visibility;

pub use ids::{Cid, Id128, IdError, IdFp};
pub use tree::{
    Agent, Endpoint, EndpointKind, Lane, Person, PersonRecord, Policy, Progress, Project, Role,
    Status, Task, Visibility,
};
