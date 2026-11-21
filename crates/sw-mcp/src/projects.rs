//! The projects this Device knows (pure): those its Person created, and those it asked to join
//! by invite, with the name the invite carried. Only the daemon reads and writes
//! `projects.json`; the node stays the authority on who is a member.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Created here; this Person owns it.
    Owner,
    /// An invite was spent; the owner's answer decides.
    Requested,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Known {
    pub name: String,
    pub role: Role,
    /// The owner's `id_fp`.
    pub owner: String,
    pub at: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Projects {
    pub projects: BTreeMap<String, Known>,
}

impl Projects {
    pub fn from_json(bytes: &[u8]) -> Result<Self, String> {
        serde_json::from_slice(bytes).map_err(|e| format!("projects.json: {e}"))
    }

    pub fn to_json(&self) -> Result<Vec<u8>, String> {
        serde_json::to_vec_pretty(self).map_err(|e| format!("projects.json: {e}"))
    }

    /// Records a project; an owned project is never downgraded to a request.
    pub fn record(&mut self, id: &str, known: Known) {
        let keep = self
            .projects
            .get(id)
            .is_some_and(|k| k.role == Role::Owner && known.role != Role::Owner);
        if !keep {
            self.projects.insert(id.to_owned(), known);
        }
    }

    pub fn name_of(&self, id: &str) -> Option<&str> {
        self.projects.get(id).map(|k| k.name.as_str())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn known(role: Role) -> Known {
        Known {
            name: "n".into(),
            role,
            owner: "o".into(),
            at: 1,
        }
    }

    #[test]
    fn an_owned_project_stays_owned_and_the_file_round_trips() {
        let mut p = Projects::default();
        p.record("a", known(Role::Owner));
        p.record("a", known(Role::Requested));
        assert_eq!(p.projects["a"].role, Role::Owner);
        // Control: a request is recorded where nothing was.
        p.record("b", known(Role::Requested));
        assert_eq!(p.projects["b"].role, Role::Requested);
        let again = Projects::from_json(&p.to_json().unwrap()).unwrap();
        assert_eq!(again, p);
        assert_eq!(again.name_of("a"), Some("n"));
        assert!(Projects::from_json(br#"{"projects":{},"x":1}"#).is_err());
    }
}
