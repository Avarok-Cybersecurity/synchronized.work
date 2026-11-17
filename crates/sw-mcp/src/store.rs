//! Loading and saving the Device's files (the `KeyStore` of design §5.4, file kind): keys, the
//! signed allowlist and the replay log. Every reader refuses what it cannot fully parse.

use ciborium::value::Value;
use sw_protocol::cbor;

use crate::gate::ReplayEntry;
use crate::home::{Home, read_private, write_private};
use crate::keys::DeviceKeys;
use crate::trust::TrustList;

pub fn load_or_create_keys(
    home: &Home,
    entropy: &mut dyn sw_crypto::Entropy,
) -> Result<DeviceKeys, String> {
    let path = home.device_keys();
    match read_private(&path).map_err(|e| e.to_string())? {
        Some(bytes) => DeviceKeys::from_bytes(&bytes).map_err(|e| e.to_string()),
        None => {
            let keys = DeviceKeys::generate(entropy);
            let bytes = keys.to_bytes().map_err(|e| e.to_string())?;
            write_private(&path, &bytes).map_err(|e| e.to_string())?;
            log::info!("generated this Device's signing and sealing keys and Citadel password");
            Ok(keys)
        }
    }
}

/// Loads the keys without ever creating them (for the CLI, which must not race the daemon).
pub fn load_keys(home: &Home) -> Result<DeviceKeys, String> {
    let bytes = read_private(&home.device_keys())
        .map_err(|e| e.to_string())?
        .ok_or("no Device keys yet; run `sw-mcp daemon` once first")?;
    DeviceKeys::from_bytes(&bytes).map_err(|e| e.to_string())
}

pub fn load_trust(home: &Home, keys: &DeviceKeys) -> Result<TrustList, String> {
    match read_private(&home.trusted_controllers()).map_err(|e| e.to_string())? {
        Some(bytes) => {
            TrustList::from_bytes(&bytes, keys.sign.public_key()).map_err(|e| e.to_string())
        }
        None => Ok(TrustList::default()),
    }
}

pub fn save_trust(home: &Home, keys: &DeviceKeys, trust: &TrustList) -> Result<(), String> {
    let bytes = trust.to_bytes(&keys.sign).map_err(|e| e.to_string())?;
    write_private(&home.trusted_controllers(), &bytes).map_err(|e| e.to_string())
}

/// `replay.cbor` = `[[kid, nonce, seq, exp], …]`.
pub fn load_replay_log(home: &Home) -> Result<Vec<ReplayEntry>, String> {
    let Some(bytes) = read_private(&home.replay_log()).map_err(|e| e.to_string())? else {
        return Ok(Vec::new());
    };
    let bad = || "replay.cbor is not a list of [kid, nonce, seq, exp]".to_owned();
    let Ok(Value::Array(items)) = cbor::decode(&bytes) else {
        return Err(bad());
    };
    items
        .into_iter()
        .map(|item| match item {
            Value::Array(f) => match f.as_slice() {
                [
                    Value::Bytes(kid),
                    Value::Bytes(nonce),
                    Value::Integer(seq),
                    Value::Integer(exp),
                ] => Ok(ReplayEntry {
                    kid: kid.clone().try_into().map_err(|_| bad())?,
                    nonce: nonce.clone().try_into().map_err(|_| bad())?,
                    seq: u64::try_from(*seq).map_err(|_| bad())?,
                    exp: u64::try_from(*exp).map_err(|_| bad())?,
                }),
                _ => Err(bad()),
            },
            _ => Err(bad()),
        })
        .collect()
}

pub fn save_replay_log(home: &Home, log: &[ReplayEntry]) -> Result<(), String> {
    let items = log
        .iter()
        .map(|e| {
            Value::Array(vec![
                Value::Bytes(e.kid.to_vec()),
                Value::Bytes(e.nonce.to_vec()),
                Value::from(e.seq),
                Value::from(e.exp),
            ])
        })
        .collect();
    let bytes = cbor::encode(&Value::Array(items)).map_err(|e| format!("{e:?}"))?;
    write_private(&home.replay_log(), &bytes).map_err(|e| e.to_string())
}
