//! The keys of `fixtures/sw-wire-kat.json` (C0), rebuilt from their seeds and checked
//! against the published public keys and kids before any fixture is judged with them.

use serde_json::Value as Json;
use sw_crypto::{KemSecretKey, SigningKey, kid_of};
use sw_model::{Cid, EndpointKind, IdFp};
use sw_protocol::gate::KeyRole;
use sw_protocol::ports::IdentityPort;

use crate::model::{hex, unhex};
use crate::{Directory, StandInIdentityPort};

#[derive(Debug)]
pub struct Kat {
    pub person: IdFp,
    pub identity_pk: Vec<u8>,
    pub controller: SigningKey,
    pub controller_cid: Cid,
    pub device: SigningKey,
    pub device_kem: KemSecretKey,
    pub device_cid: Cid,
}

fn text<'a>(j: &'a Json, path: &[&str]) -> Result<&'a str, String> {
    let mut at = j;
    for p in path {
        at = at
            .get(p)
            .ok_or_else(|| format!("kat: missing {}", path.join(".")))?;
    }
    at.as_str()
        .ok_or_else(|| format!("kat: {} is not text", path.join(".")))
}

fn uint(j: &Json, path: &[&str]) -> Result<u64, String> {
    let mut at = j;
    for p in path {
        at = at
            .get(p)
            .ok_or_else(|| format!("kat: missing {}", path.join(".")))?;
    }
    at.as_u64()
        .ok_or_else(|| format!("kat: {} is not a uint", path.join(".")))
}

fn fixed<const N: usize>(j: &Json, path: &[&str]) -> Result<[u8; N], String> {
    unhex(text(j, path)?)?
        .try_into()
        .map_err(|_| format!("kat: {} is not {N} bytes", path.join(".")))
}

fn same(label: &str, got: &[u8], want: &str) -> Result<(), String> {
    if hex(got) == want {
        Ok(())
    } else {
        Err(format!("kat: {label} does not match"))
    }
}

impl Kat {
    pub fn parse(json: &str) -> Result<Self, String> {
        let j: Json = serde_json::from_str(json).map_err(|e| e.to_string())?;
        let controller = SigningKey::from_seed(&fixed::<32>(&j, &["controller", "sign_xi_hex"])?);
        same(
            "controller pk",
            controller.public_key(),
            text(&j, &["controller", "sign_pk_hex"])?,
        )?;
        same(
            "controller kid",
            &kid_of(controller.public_key()),
            text(&j, &["controller", "kid_hex"])?,
        )?;
        let device = SigningKey::from_seed(&fixed::<32>(&j, &["device", "sign_xi_hex"])?);
        same(
            "device pk",
            device.public_key(),
            text(&j, &["device", "sign_pk_hex"])?,
        )?;
        same(
            "device kid",
            &kid_of(device.public_key()),
            text(&j, &["device", "kid_hex"])?,
        )?;
        let device_kem = KemSecretKey::from_seed(&fixed::<64>(&j, &["device", "kem_seed_hex"])?);
        same(
            "device ek",
            device_kem.encapsulation_key(),
            text(&j, &["device", "kem_ek_hex"])?,
        )?;
        same(
            "device kem kid",
            &kid_of(device_kem.encapsulation_key()),
            text(&j, &["device", "kem_kid_hex"])?,
        )?;
        let pk = data_encoding::BASE64URL_NOPAD
            .decode(text(&j, &["person", "identity_pk_b64url"])?.as_bytes())
            .map_err(|e| format!("kat: identity pk: {e}"))?;
        same(
            "identity kid",
            &kid_of(&pk),
            text(&j, &["person", "identity_kid_hex"])?,
        )?;
        let person = StandInIdentityPort
            .id_fp(&pk)
            .map_err(|e| format!("kat: {e:?}"))?;
        if person.as_str() != text(&j, &["person", "id_fp"])? {
            return Err("kat: id_fp of the identity pk does not match".into());
        }
        Ok(Self {
            person,
            identity_pk: pk,
            controller,
            controller_cid: uint(&j, &["controller", "cid"])?,
            device,
            device_kem,
            device_cid: uint(&j, &["device", "cid"])?,
        })
    }

    /// The node's view of the KAT Person: identity, Device and Controller keys.
    pub fn node_directory(&self) -> Directory {
        let mut d = Directory::default();
        d.add_key(&self.identity_pk, self.person.clone(), KeyRole::Identity);
        let device = KeyRole::Endpoint {
            cid: self.device_cid,
            kind: EndpointKind::Device,
        };
        d.add(&self.device, self.person.clone(), device);
        let controller = KeyRole::Endpoint {
            cid: self.controller_cid,
            kind: EndpointKind::Controller,
        };
        d.add(&self.controller, self.person.clone(), controller);
        d
    }
}
