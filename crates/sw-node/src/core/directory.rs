//! The Person records (SSOT) and the key index derived from them: the gate's
//! [`KeyDirectory`] at the node. The index is rebuilt from a record every time that record
//! changes, so it can never disagree with what is stored.

use std::collections::BTreeMap;

use sw_crypto::{Kid, kid_of};
use sw_model::{Cid, EndpointKind, IdFp, PersonRecord};
use sw_protocol::gate::{KeyDirectory, KeyEntry, KeyRole};

#[derive(Debug, Default)]
pub struct Directory {
    persons: BTreeMap<IdFp, PersonRecord>,
    keys: BTreeMap<Kid, KeyEntry>,
    endpoints: BTreeMap<Cid, (IdFp, EndpointKind)>,
}

impl Directory {
    pub fn person(&self, id: &IdFp) -> Option<&PersonRecord> {
        self.persons.get(id)
    }

    pub fn persons(&self) -> usize {
        self.persons.len()
    }

    /// Who an enrolled Endpoint CID belongs to.
    pub fn endpoint(&self, cid: Cid) -> Option<&(IdFp, EndpointKind)> {
        self.endpoints.get(&cid)
    }

    /// Every enrolled Endpoint CID of a Person.
    pub fn endpoints_of(&self, id: &IdFp) -> Vec<Cid> {
        self.persons
            .get(id)
            .map(|p| p.endpoints.iter().map(|e| e.cid).collect())
            .unwrap_or_default()
    }

    /// Replaces a Person's record and re-derives its index entries.
    pub fn put(&mut self, record: PersonRecord) {
        self.remove_index(&record.id_fp);
        let owner = record.id_fp.clone();
        self.keys.insert(
            kid_of(&record.pk),
            KeyEntry {
                public_key: record.pk.clone(),
                owner: owner.clone(),
                role: KeyRole::Identity,
            },
        );
        for e in &record.endpoints {
            let role = KeyRole::Endpoint {
                cid: e.cid,
                kind: e.kind,
            };
            let entry = KeyEntry {
                public_key: e.sign_pk.clone(),
                owner: owner.clone(),
                role,
            };
            self.keys.insert(kid_of(&e.sign_pk), entry);
            self.endpoints.insert(e.cid, (owner.clone(), e.kind));
        }
        self.persons.insert(owner, record);
    }

    fn remove_index(&mut self, id: &IdFp) {
        self.keys.retain(|_, entry| entry.owner != *id);
        self.endpoints.retain(|_, (owner, _)| owner != id);
    }

    /// The CID another Person's Endpoint already holds, which an enrol must not take over.
    pub fn cid_taken_by_other(&self, cid: Cid, id: &IdFp) -> bool {
        self.endpoints
            .get(&cid)
            .is_some_and(|(owner, _)| owner != id)
    }

    /// A signing key enrolled under any other Person.
    pub fn key_taken_by_other(&self, kid: &Kid, id: &IdFp) -> bool {
        self.keys.get(kid).is_some_and(|entry| entry.owner != *id)
    }
}

impl KeyDirectory for Directory {
    fn key(&self, kid: &Kid) -> Option<KeyEntry> {
        self.keys.get(kid).cloned()
    }
}
