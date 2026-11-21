//! `sw-release-sign keygen --private-key F --public-key F`
//! `sw-release-sign check-key --public-key F`
//! `sw-release-sign sign --private-key F --tag T FILE...`   writes FILE.mldsa.sig
//! `sw-release-sign verify --public-key F --tag T FILE...`  reads FILE.mldsa.sig

use std::path::Path;
use std::process::ExitCode;

use data_encoding::{BASE64URL_NOPAD, HEXLOWER};
use sw_release_sign::{private_key, public_key, sign, verify};
use zeroize::Zeroizing;

fn read(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))
}

fn name_of(file: &str) -> Result<String, String> {
    Path::new(file)
        .file_name()
        .and_then(|n| n.to_str())
        .map(str::to_owned)
        .ok_or_else(|| format!("{file}: no file name"))
}

fn flag<'a>(args: &'a [String], name: &str) -> Result<&'a str, String> {
    let at = args
        .iter()
        .position(|a| a == name)
        .ok_or_else(|| format!("{name} is required"))?;
    args.get(at + 1)
        .map(String::as_str)
        .ok_or_else(|| format!("{name} needs a value"))
}

/// Everything after the flags (`--x v` pairs) is a file.
fn files(args: &[String]) -> Vec<&str> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if args[i].starts_with("--") {
            i += 2;
        } else {
            out.push(args[i].as_str());
            i += 1;
        }
    }
    out
}

fn write_private(path: &str, text: &str) -> Result<(), String> {
    use std::io::Write as _;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut f = options.open(path).map_err(|e| format!("{path}: {e}"))?;
    f.write_all(text.as_bytes())
        .map_err(|e| format!("{path}: {e}"))
}

fn run(args: &[String]) -> Result<(), String> {
    let rest = args.get(1..).unwrap_or_default();
    match args.first().map(String::as_str) {
        Some("keygen") => {
            let mut seed = Zeroizing::new([0u8; 32]);
            getrandom::fill(seed.as_mut()).map_err(|e| format!("the OS CSPRNG: {e}"))?;
            let key = sw_crypto::SigningKey::from_seed(&seed);
            write_private(
                flag(rest, "--private-key")?,
                &format!("{}\n", HEXLOWER.encode(seed.as_ref())),
            )?;
            let public = format!("{}\n", BASE64URL_NOPAD.encode(key.public_key()));
            let path = flag(rest, "--public-key")?;
            std::fs::write(path, public).map_err(|e| format!("{path}: {e}"))
        }
        Some("check-key") => public_key(&read(flag(rest, "--public-key")?)?).map(drop),
        Some("sign") => {
            let key = private_key(&Zeroizing::new(read(flag(rest, "--private-key")?)?))?;
            let tag = flag(rest, "--tag")?;
            for file in files(rest) {
                let bytes = std::fs::read(file).map_err(|e| format!("{file}: {e}"))?;
                let sig = sign(&key, tag, &name_of(file)?, &bytes)?;
                let out = format!("{file}.mldsa.sig");
                std::fs::write(&out, sig).map_err(|e| format!("{out}: {e}"))?;
                println!("signed {file}");
            }
            Ok(())
        }
        Some("verify") => {
            let pk = public_key(&read(flag(rest, "--public-key")?)?)?;
            let tag = flag(rest, "--tag")?;
            let all = files(rest);
            if all.is_empty() {
                return Err("no files to verify".into());
            }
            for file in all {
                let bytes = std::fs::read(file).map_err(|e| format!("{file}: {e}"))?;
                let sig = read(&format!("{file}.mldsa.sig"))?;
                verify(&pk, tag, &name_of(file)?, &bytes, &sig)?;
                println!("verified {file}");
            }
            Ok(())
        }
        _ => Err(
            "usage: sw-release-sign keygen|check-key|sign|verify (see the source header)".into(),
        ),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("sw-release-sign: {e}");
            ExitCode::FAILURE
        }
    }
}
