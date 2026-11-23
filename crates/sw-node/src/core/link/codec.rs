//! The stored link codes and viewer links, in the wire's canonical CBOR.

use sw_protocol::wire::{MapIn, MapOut};
use sw_protocol::{WireError, cbor};

use super::{Code, ViewerLink};

pub fn code_to_bytes(c: &Code) -> Result<Vec<u8>, WireError> {
    let mut out = MapOut::default();
    out.field("person", &c.person);
    out.field("project", &c.project);
    out.field("exp", &c.exp);
    out.field("spent", &c.spent);
    cbor::encode(&out.build())
}

pub fn code_from_bytes(bytes: &[u8]) -> Result<Code, WireError> {
    let mut map = MapIn::new(cbor::decode(bytes)?)?;
    let code = Code {
        person: map.field("person")?,
        project: map.field("project")?,
        exp: map.field("exp")?,
        spent: map.field("spent")?,
    };
    map.finish()?;
    Ok(code)
}

pub fn viewer_to_bytes(l: &ViewerLink) -> Result<Vec<u8>, WireError> {
    let mut out = MapOut::default();
    out.field("person", &l.person);
    out.field("project", &l.project);
    out.field("node_id", &l.node_id);
    out.field("install_id", &l.install_id);
    out.field("acting_username", &l.acting_username);
    out.field("at", &l.at);
    cbor::encode(&out.build())
}

pub fn viewer_from_bytes(bytes: &[u8]) -> Result<ViewerLink, WireError> {
    let mut map = MapIn::new(cbor::decode(bytes)?)?;
    let link = ViewerLink {
        person: map.field("person")?,
        project: map.field("project")?,
        node_id: map.field("node_id")?,
        install_id: map.field("install_id")?,
        acting_username: map.field("acting_username")?,
        at: map.field("at")?,
    };
    map.finish()?;
    Ok(link)
}
