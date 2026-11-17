//! `ENROL` / `REVOKE` (sw-wire §3 row 11, design §5.2): what an accepted envelope does to the
//! Person's record. The gate already proved who signed; this decides whether the change is
//! allowed and what the new record is.

use sw_crypto::kid_of;
use sw_model::{Endpoint, PersonPolicy, PersonRecord};
use sw_protocol::gate::Verified;
use sw_protocol::payload::{Enrol, EnrolOp};

use super::NodeRefusal;
use super::directory::Directory;

/// The record after `enrol`, or why it is refused. A Person the node has never seen can only
/// arrive by assertion: a root-signed envelope names its key by `kid` alone, so the node has
/// no key to check it with until a record exists.
pub fn apply(
    dir: &Directory,
    verified: &Verified,
    enrol: &Enrol,
    new_person_policy: PersonPolicy,
    max_persons: usize,
) -> Result<PersonRecord, NodeRefusal> {
    let iss = &verified.body.iss;
    let mut record = match (dir.person(iss), &verified.sign_in) {
        (Some(existing), _) => existing.clone(),
        (None, Some(claims)) if enrol.op == EnrolOp::Enrol => {
            if dir.persons() >= max_persons {
                return Err(NodeRefusal::CapReached("persons"));
            }
            PersonRecord {
                id_fp: iss.clone(),
                pk: claims.identity.clone(),
                display: iss.as_str().to_owned(),
                endpoints: Vec::new(),
                epoch: enrol.epoch,
                policy: new_person_policy,
            }
        }
        (None, _) => return Err(NodeRefusal::UnknownPerson),
    };
    match enrol.op {
        EnrolOp::Enrol => add_endpoint(dir, &mut record, enrol)?,
        EnrolOp::Revoke => revoke(&mut record, enrol)?,
    }
    Ok(record)
}

fn add_endpoint(
    dir: &Directory,
    record: &mut PersonRecord,
    enrol: &Enrol,
) -> Result<(), NodeRefusal> {
    if enrol.epoch != record.epoch {
        return Err(NodeRefusal::StaleEpoch);
    }
    let (Some(cid), Some(kind), Some(keys)) = (enrol.endpoint_cid, enrol.kind, &enrol.pubkeys)
    else {
        return Err(NodeRefusal::Malformed);
    };
    if dir.cid_taken_by_other(cid, &record.id_fp)
        || dir.key_taken_by_other(&kid_of(&keys.sign.0), &record.id_fp)
    {
        return Err(NodeRefusal::TakenByAnotherPerson);
    }
    record.endpoints.retain(|e| e.cid != cid);
    record.endpoints.push(Endpoint {
        cid,
        kind,
        sign_pk: keys.sign.0.clone(),
        seal_pk: keys.seal.as_ref().map(|s| s.0.clone()),
        epoch: enrol.epoch,
    });
    Ok(())
}

/// A revoke names an Endpoint key of this Person and moves the epoch forward, so an enrol
/// signed before it (at the old epoch) can no longer be replayed into the record.
fn revoke(record: &mut PersonRecord, enrol: &Enrol) -> Result<(), NodeRefusal> {
    let Some(kid) = enrol.kid else {
        return Err(NodeRefusal::Malformed);
    };
    if enrol.epoch <= record.epoch {
        return Err(NodeRefusal::StaleEpoch);
    }
    let before = record.endpoints.len();
    record.endpoints.retain(|e| kid_of(&e.sign_pk) != kid);
    if record.endpoints.len() == before {
        return Err(NodeRefusal::NoSuchEndpoint);
    }
    record.epoch = enrol.epoch;
    Ok(())
}
