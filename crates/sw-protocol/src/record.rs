//! The stored Person record (sw-wire contract §4): integer keys outside, text keys in
//! `policy`.

use ciborium::value::Value;
use sw_model::{Approval, Endpoint, PersonPolicy, PersonRecord};

use crate::wire::{Bytes, MapIn, MapOut, Wire, wire_str};
use crate::{WireError, cbor};

wire_str!(Approval { "manual" => Approval::Manual, "auto" => Approval::Auto });

impl Wire for PersonPolicy {
    fn to_value(&self) -> Value {
        let mut out = MapOut::default();
        out.field("approval", &self.approval);
        out.field("rate_per_min", &self.rate_per_min);
        out.build()
    }
    fn from_value(value: Value) -> Result<Self, WireError> {
        let mut map = MapIn::new(value)?;
        let policy = Self {
            approval: map.field("approval")?,
            rate_per_min: map.field("rate_per_min")?,
        };
        map.finish()?;
        Ok(policy)
    }
}

/// `{1: cid, 2: kind, 3: sign, 4?: seal, 5: epoch}`.
impl Wire for Endpoint {
    fn to_value(&self) -> Value {
        let mut out = MapOut::default();
        out.field(1u64, &self.cid);
        out.field(2u64, &self.kind);
        out.field(3u64, &Bytes(self.sign_pk.clone()));
        out.field(4u64, &self.seal_pk.clone().map(Bytes));
        out.field(5u64, &self.epoch);
        out.build()
    }
    fn from_value(value: Value) -> Result<Self, WireError> {
        let mut map = MapIn::new(value)?;
        let endpoint = Self {
            cid: map.field(1u64)?,
            kind: map.field(2u64)?,
            sign_pk: map.field::<Bytes>(3u64)?.0,
            seal_pk: map.field::<Option<Bytes>>(4u64)?.map(|b| b.0),
            epoch: map.field(5u64)?,
        };
        map.finish()?;
        Ok(endpoint)
    }
}

/// `{1: id_fp, 2: pk, 3: display, 4: endpoints, 5: epoch, 6: policy}`.
impl Wire for PersonRecord {
    fn to_value(&self) -> Value {
        let mut out = MapOut::default();
        out.field(1u64, &self.id_fp);
        out.field(2u64, &Bytes(self.pk.clone()));
        out.field(3u64, &self.display);
        out.field(4u64, &self.endpoints);
        out.field(5u64, &self.epoch);
        out.field(6u64, &self.policy);
        out.build()
    }
    fn from_value(value: Value) -> Result<Self, WireError> {
        let mut map = MapIn::new(value)?;
        let record = Self {
            id_fp: map.field(1u64)?,
            pk: map.field::<Bytes>(2u64)?.0,
            display: map.field(3u64)?,
            endpoints: map.field(4u64)?,
            epoch: map.field(5u64)?,
            policy: map.field(6u64)?,
        };
        map.finish()?;
        Ok(record)
    }
}

pub fn record_to_bytes(record: &PersonRecord) -> Result<Vec<u8>, WireError> {
    cbor::encode(&record.to_value())
}

pub fn record_from_bytes(bytes: &[u8]) -> Result<PersonRecord, WireError> {
    PersonRecord::from_value(cbor::decode(bytes)?)
}
