//! The conformance gate: every fixture in `fixtures/sw-wire/` (C0's, mirrored byte for byte)
//! passes `sw_testkit::conformance::check` with the keys of `fixtures/sw-wire-kat.json`, and
//! at least one `invalid-*` fixture is present and refused. `--mirror <contract dir>` also
//! proves the mirror is exact (run it wherever the parent checkout is available).

use std::path::Path;

use sw_testkit::conformance::check;
use sw_testkit::kat::Kat;

pub fn run(root: &Path, args: &[String]) -> Result<String, String> {
    let dir = root.join("fixtures");
    let kat = Kat::parse(&read(&dir.join("sw-wire-kat.json"))?)?;
    let files = json_files(&dir.join("sw-wire"))?;
    let mut invalid = 0;
    for name in &files {
        check(name, &read(&dir.join("sw-wire").join(name))?, &kat)?;
        invalid += usize::from(name.starts_with("invalid-"));
    }
    if invalid == 0 {
        return Err("no invalid-* fixture, so refusal was never exercised".to_owned());
    }
    let mirror = match args {
        [flag, contract] if flag == "--mirror" => mirror(&dir, Path::new(contract))?,
        [] => "mirror not checked (pass --mirror <docs/contracts/batch-1116>)".to_owned(),
        _ => return Err("usage: xtask conformance [--mirror <contract dir>]".to_owned()),
    };
    Ok(format!(
        "conformance: PASS ({} fixtures, {invalid} refused as required; {mirror})",
        files.len()
    ))
}

fn mirror(ours: &Path, contract: &Path) -> Result<String, String> {
    let theirs = json_files(&contract.join("fixtures/sw-wire"))?;
    if theirs != json_files(&ours.join("sw-wire"))? {
        return Err("fixtures/sw-wire/ does not hold the contract's file set".to_owned());
    }
    let mut pairs: Vec<(String, String)> = theirs
        .iter()
        .map(|n| (format!("fixtures/sw-wire/{n}"), format!("sw-wire/{n}")))
        .collect();
    pairs.push(("sw-wire-kat.json".to_owned(), "sw-wire-kat.json".to_owned()));
    for (theirs, ours_rel) in &pairs {
        if read(&contract.join(theirs))? != read(&ours.join(ours_rel))? {
            return Err(format!(
                "fixtures/{ours_rel} differs from the contract's {theirs}"
            ));
        }
    }
    Ok(format!("mirror exact ({} files)", pairs.len()))
}

fn json_files(dir: &Path) -> Result<Vec<String>, String> {
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

fn read(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}
