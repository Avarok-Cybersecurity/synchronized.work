//! The stored marks, in the wire's canonical CBOR (text-keyed maps, unknown keys refused).

use sw_protocol::payload::StatusUpdate;
use sw_protocol::wire::{MapIn, MapOut, Raw, Wire};
use sw_protocol::{WireError, cbor};

use super::{AgentMark, Author, TaskMark};

fn put_author(out: &mut MapOut, by: &Author) {
    out.field("iss", &by.iss);
    out.field("from", &by.from);
}

fn take_author(map: &mut MapIn) -> Result<Author, WireError> {
    Ok(Author {
        iss: map.field("iss")?,
        from: map.field("from")?,
    })
}

pub fn task_to_bytes(m: &TaskMark) -> Result<Vec<u8>, WireError> {
    let mut out = MapOut::default();
    out.field("task", &m.task);
    out.field("project", &m.project);
    out.field("cursor", &m.cursor);
    out.field("deleted", &m.deleted);
    out.field("digest", &m.digest);
    put_author(&mut out, &m.by);
    cbor::encode(&out.build())
}

pub fn task_from_bytes(bytes: &[u8]) -> Result<TaskMark, WireError> {
    let mut map = MapIn::new(cbor::decode(bytes)?)?;
    let mark = TaskMark {
        task: map.field("task")?,
        project: map.field("project")?,
        cursor: map.field("cursor")?,
        deleted: map.field("deleted")?,
        digest: map.field("digest")?,
        by: take_author(&mut map)?,
    };
    map.finish()?;
    Ok(mark)
}

pub fn agent_to_bytes(m: &AgentMark) -> Result<Vec<u8>, WireError> {
    let mut out = MapOut::default();
    out.field("project", &m.project);
    out.field("cursor", &m.cursor);
    out.field("status", &Raw(m.status.to_value()));
    put_author(&mut out, &m.by);
    cbor::encode(&out.build())
}

pub fn agent_from_bytes(bytes: &[u8]) -> Result<AgentMark, WireError> {
    let mut map = MapIn::new(cbor::decode(bytes)?)?;
    let mark = AgentMark {
        project: map.field("project")?,
        cursor: map.field("cursor")?,
        status: StatusUpdate::from_value(map.field::<Raw>("status")?.0)?,
        by: take_author(&mut map)?,
    };
    map.finish()?;
    Ok(mark)
}
