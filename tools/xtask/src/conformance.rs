//! The conformance gate: every fixture in `fixtures/sw-wire/` (C0's, mirrored byte for byte)
//! passes `sw_testkit::conformance::check` with the keys of `fixtures/sw-wire-kat.json` and
//! `fixtures/sign-in-kat.json`, at least one `invalid-*` fixture is present and refused, the
//! KAT Device enrols itself by the published sign-in assertion, and every mirrored file still
//! has the digest `fixtures/contract.pin` records. `--mirror <contract dir>` also proves the
//! mirror against the parent checkout; `--repin <contract dir> <parent rev>` rewrites the pin
//! after proving it.

use std::path::Path;

use sw_testkit::conformance::{check, check_sign_in_enrol};
use sw_testkit::kat::Kat;
use sw_testkit::sign_in_kat::SignInKat;

use crate::pin;

pub fn run(root: &Path, args: &[String]) -> Result<String, String> {
    let dir = root.join("fixtures");
    let kat = Kat::parse(&pin::read(&dir.join("sw-wire-kat.json"))?)?;
    let sign_in = SignInKat::parse(&pin::read(&dir.join("sign-in-kat.json"))?)?;
    let files = pin::json_files(&dir.join("sw-wire"))?;
    let mut invalid = 0;
    for name in &files {
        check(
            name,
            &pin::read(&dir.join("sw-wire").join(name))?,
            &kat,
            &sign_in,
        )?;
        invalid += usize::from(name.starts_with("invalid-"));
    }
    if invalid == 0 {
        return Err("no invalid-* fixture, so refusal was never exercised".to_owned());
    }
    check_sign_in_enrol(&kat, &sign_in)?;
    let mirror = match args {
        [flag, contract] if flag == "--mirror" => pin::mirror(&dir, Path::new(contract))?,
        [flag, contract, rev] if flag == "--repin" => {
            pin::mirror(&dir, Path::new(contract))?;
            pin::write(&dir, rev)?
        }
        [] => "parent checkout not compared (pass --mirror <docs/contracts/batch-1116>)".to_owned(),
        _ => {
            return Err(
                "usage: xtask conformance [--mirror <contract dir> | --repin <contract dir> <rev>]"
                    .to_owned(),
            );
        }
    };
    let pinned = pin::verify(&dir)?;
    Ok(format!(
        "conformance: PASS ({} fixtures, {invalid} refused as required; enrol by assertion \
         accepted; {pinned}; {mirror})",
        files.len()
    ))
}
