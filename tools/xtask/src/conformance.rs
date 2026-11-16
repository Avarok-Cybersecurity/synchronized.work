//! The conformance gate: the committed fixtures are exactly what the generator produces, the
//! MANIFEST digests match, and every fixture passes `sw_testkit::conformance::check`.

use std::collections::BTreeSet;
use std::path::Path;

use sw_testkit::conformance::{check, diag_name, diagnostic, manifest};
use sw_testkit::fixtures;

const MANIFEST: &str = "MANIFEST";

pub fn generate(root: &Path) -> Result<String, String> {
    let dir = root.join("fixtures");
    let all = fixtures::all();
    for (name, bytes) in &all {
        write(&dir.join(name), bytes)?;
        write(
            &dir.join(diag_name(name)),
            format!("{}\n", diagnostic(bytes)?).as_bytes(),
        )?;
    }
    write(&dir.join(MANIFEST), manifest(&all).as_bytes())?;
    Ok(format!(
        "gen-fixtures: wrote {} fixtures to {}",
        all.len(),
        dir.display()
    ))
}

pub fn run(root: &Path) -> Result<String, String> {
    let dir = root.join("fixtures");
    let expected = fixtures::all();
    let mut on_disk = Vec::new();
    let mut names = BTreeSet::new();
    for entry in std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let name = entry.map_err(|e| e.to_string())?.file_name();
        names.insert(name.to_string_lossy().into_owned());
    }
    let mut known: BTreeSet<String> = [MANIFEST.to_owned()].into();
    for (name, generated) in &expected {
        known.insert((*name).to_owned());
        known.insert(diag_name(name));
        let bytes = read(&dir.join(name))?;
        let diag = String::from_utf8(read(&dir.join(diag_name(name)))?)
            .map_err(|_| format!("{} is not UTF-8", diag_name(name)))?;
        check(name, &bytes, &diag)?;
        if &bytes != generated {
            return Err(format!("{name} differs from what the generator produces"));
        }
        on_disk.push((*name, bytes));
    }
    let stray: Vec<&String> = names.difference(&known).collect();
    if !stray.is_empty() {
        return Err(format!(
            "fixtures/ holds files no generator produced: {stray:?}"
        ));
    }
    let committed = String::from_utf8(read(&dir.join(MANIFEST))?).map_err(|e| e.to_string())?;
    if committed != manifest(&on_disk) {
        return Err("fixtures/MANIFEST digests do not match the fixtures".to_owned());
    }
    Ok(format!("conformance: PASS ({} fixtures)", expected.len()))
}

fn read(path: &Path) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()))
}
