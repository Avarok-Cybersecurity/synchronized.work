//! How the node stores what it owns, in the wire's canonical CBOR (text-keyed maps, unknown
//! keys refused), so a stored record decodes exactly as it was written on every target.

use std::collections::{BTreeMap, BTreeSet};

use ciborium::value::Value;
use sw_model::tier::{Coop, JoinPolicy, JoinRequest, Tier, TierName};
use sw_model::{IdFp, Status};
use sw_protocol::wire::{MapIn, MapOut, Raw};
use sw_protocol::{WireError, cbor};

use super::task::{AgentState, TaskState};

fn tier_value(tier: &Tier) -> Value {
    let mut out = MapOut::default();
    out.field("name", &tier.name());
    match tier {
        Tier::Off => {}
        Tier::Aware { policy } => out.field("policy", policy),
        Tier::Synchronized { policy, coworkers } => {
            out.field("policy", policy);
            out.field(
                "coworkers",
                &coworkers.iter().cloned().collect::<Vec<IdFp>>(),
            );
        }
    }
    out.build()
}

fn tier_from(value: Value) -> Result<Tier, WireError> {
    let mut map = MapIn::new(value)?;
    let name: TierName = map.field("name")?;
    let policy: Option<JoinPolicy> = map.field("policy")?;
    let coworkers: Option<Vec<IdFp>> = map.field("coworkers")?;
    map.finish()?;
    match (name, policy, coworkers) {
        (TierName::Off, None, None) => Ok(Tier::Off),
        (TierName::Aware, Some(policy), None) => Ok(Tier::Aware { policy }),
        (TierName::Synchronized, Some(policy), Some(c)) if !c.is_empty() => {
            Ok(Tier::Synchronized {
                policy,
                coworkers: c.into_iter().collect::<BTreeSet<IdFp>>(),
            })
        }
        _ => Err(WireError::Shape("stored tier")),
    }
}

fn task_value(t: &TaskState) -> Value {
    let mut out = MapOut::default();
    out.field("id", &t.id);
    out.field("project", &t.project);
    out.field("parent", &t.parent);
    out.field("title", &t.title);
    out.field("description", &t.description);
    out.field("visibility", &t.visibility);
    out.field("owner", &t.coop.owner);
    out.field("status", &t.coop.status);
    out.field("tier", &Raw(tier_value(&t.coop.tier)));
    let pending: Vec<Raw> = t
        .coop
        .pending
        .iter()
        .map(|r| {
            let mut m = MapOut::default();
            m.field("person", &r.person);
            m.field("readiness", &r.readiness);
            m.field("at", &r.at_ms);
            Raw(m.build())
        })
        .collect();
    out.field("pending", &pending);
    let agents: Vec<Raw> = t
        .agents
        .iter()
        .map(|(id, a)| {
            let mut m = MapOut::default();
            m.field("id", id);
            m.field("person", &a.person);
            m.field("device", &a.device);
            m.field("lane", &a.lane);
            m.field("last_seen", &a.last_seen);
            Raw(m.build())
        })
        .collect();
    out.field("agents", &agents);
    let requests: Vec<Raw> = t
        .requests
        .iter()
        .map(|(signal_id, person)| {
            let mut m = MapOut::default();
            m.field("signal_id", signal_id);
            m.field("person", person);
            Raw(m.build())
        })
        .collect();
    out.field("requests", &requests);
    out.field("created_at", &t.created_at);
    out.field("updated_at", &t.updated_at);
    out.build()
}

pub fn task_to_bytes(task: &TaskState) -> Result<Vec<u8>, WireError> {
    cbor::encode(&task_value(task))
}

pub fn task_from_bytes(bytes: &[u8]) -> Result<TaskState, WireError> {
    let mut map = MapIn::new(cbor::decode(bytes)?)?;
    let tier = tier_from(map.field::<Raw>("tier")?.0)?;
    let mut pending = Vec::new();
    for item in map.field::<Vec<Raw>>("pending")? {
        let mut m = MapIn::new(item.0)?;
        pending.push(JoinRequest {
            person: m.field("person")?,
            readiness: m.field("readiness")?,
            at_ms: m.field("at")?,
        });
        m.finish()?;
    }
    let mut agents = BTreeMap::new();
    for item in map.field::<Vec<Raw>>("agents")? {
        let mut m = MapIn::new(item.0)?;
        let id: String = m.field("id")?;
        let agent = AgentState {
            person: m.field("person")?,
            device: m.field("device")?,
            lane: m.field("lane")?,
            last_seen: m.field("last_seen")?,
        };
        m.finish()?;
        agents.insert(id, agent);
    }
    let mut requests = BTreeMap::new();
    for item in map.field::<Vec<Raw>>("requests")? {
        let mut m = MapIn::new(item.0)?;
        let signal_id: [u8; 32] = m.field("signal_id")?;
        requests.insert(signal_id, m.field("person")?);
        m.finish()?;
    }
    let task = TaskState {
        id: map.field("id")?,
        project: map.field("project")?,
        parent: map.field("parent")?,
        title: map.field("title")?,
        description: map.field("description")?,
        visibility: map.field("visibility")?,
        coop: Coop {
            owner: map.field("owner")?,
            status: map.field::<Status>("status")?,
            tier,
            pending,
        },
        agents,
        requests,
        created_at: map.field("created_at")?,
        updated_at: map.field("updated_at")?,
    };
    map.finish()?;
    Ok(task)
}
