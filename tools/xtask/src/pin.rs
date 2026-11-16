//! `fixtures/contract.pin`: the parent revision the mirror was taken from and the SHA3-256 of
//! every mirrored file, so CI proves the mirror unchanged without the (private) parent
//! checkout. Lines are `parent <rev>` and then `<sha3 hex>  <path in the contract dir>`.

use std::path::{Path, PathBuf};

const PIN: &str = "contract.pin";
/// Mirrored files outside `fixtures/sw-wire/`, by their path in the contract dir.
const LOOSE: [&str; 2] = ["sign-in-kat.json", "sw-wire-kat.json"];

/// Every mirrored file as (path in the contract dir, path under our `fixtures/`).
fn mirrored(ours: &Path) -> Result<Vec<(String, PathBuf)>, String> {
    let mut out: Vec<(String, PathBuf)> = json_files(&ours.join("sw-wire"))?
        .into_iter()
        .map(|n| {
            (
                format!("fixtures/sw-wire/{n}"),
                ours.join("sw-wire").join(n),
            )
        })
        .collect();
    out.extend(LOOSE.iter().map(|n| ((*n).to_owned(), ours.join(n))));
    Ok(out)
}

fn digest_lines(ours: &Path) -> Result<Vec<String>, String> {
    mirrored(ours)?
        .into_iter()
        .map(|(theirs, path)| {
            let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            let hex: String = sw_crypto::sha3_256(&bytes)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect();
            Ok(format!("{hex}  {theirs}"))
        })
        .collect()
}

/// Every mirrored file has the digest the pin records, and the pin names no other file.
pub fn verify(ours: &Path) -> Result<String, String> {
    let pin = read(&ours.join(PIN))?;
    let mut lines = pin.lines().filter(|l| !l.starts_with('#'));
    let rev = lines
        .next()
        .and_then(|l| l.strip_prefix("parent "))
        .ok_or("contract.pin: first line must be `parent <rev>`")?;
    let pinned: Vec<&str> = lines.collect();
    let now = digest_lines(ours)?;
    if pinned != now {
        return Err(format!(
            "fixtures differ from contract.pin (parent {rev}): re-mirror from the contract and \
             run `xtask conformance --repin <contract dir> <rev>`"
        ));
    }
    Ok(format!(
        "{} files match contract.pin (parent {rev})",
        now.len()
    ))
}

pub fn write(ours: &Path, rev: &str) -> Result<String, String> {
    let lines = digest_lines(ours)?;
    let text = format!(
        "# Mirror of docs/contracts/batch-1116 at this parent revision; SHA3-256 per file.\n\
         parent {rev}\n{}\n",
        lines.join("\n")
    );
    let path = ours.join(PIN);
    std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(format!("contract.pin rewritten for parent {rev}"))
}

/// The mirror is exact: the same `fixtures/sw-wire/` file set and identical bytes.
pub fn mirror(ours: &Path, contract: &Path) -> Result<String, String> {
    if json_files(&contract.join("fixtures/sw-wire"))? != json_files(&ours.join("sw-wire"))? {
        return Err("fixtures/sw-wire/ does not hold the contract's file set".to_owned());
    }
    let files = mirrored(ours)?;
    for (theirs, path) in &files {
        if read(&contract.join(theirs))? != read(path)? {
            return Err(format!(
                "{} differs from the contract's {theirs}",
                path.display()
            ));
        }
    }
    Ok(format!("mirror exact ({} files)", files.len()))
}

pub fn json_files(dir: &Path) -> Result<Vec<String>, String> {
    let mut names = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let name = entry
            .map_err(|e| e.to_string())?
            .file_name()
            .to_string_lossy()
            .into_owned();
        if !name.ends_with(".json") {
            return Err(format!("{}/{name} is not a JSON fixture", dir.display()));
        }
        names.push(name);
    }
    names.sort();
    Ok(names)
}

pub fn read(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}
