//! No released binary carries its build machine's home directory (`/Users/<name>/`,
//! `/home/<name>/`, `C:\Users\<name>\`, either slash). rustc embeds source paths (panic
//! locations, `file!()`) unless the build maps them with `--remap-path-prefix`; the release
//! workflow does, and this gate proves it on the bytes it ships. The parent repo's
//! scripts/check-no-home-paths-shipped.mjs is the same rule.
//!
//!   xtask home-paths [--allow-under <dir>]... <file>...
//!
//! `--allow-under` skips a whole path under `<dir>`: a directory a C build compiled in, which no
//! rustc flag reaches (vendored OpenSSL's ENGINESDIR and MODULESDIR, under the target dir).

use std::path::Path;

/// The distinct home prefixes in `bytes` (each byte read as one latin-1 character), skipping any
/// whole path that starts with one of `allow_under`.
pub fn home_paths_in(bytes: &[u8], allow_under: &[String]) -> Vec<String> {
    let text: Vec<char> = bytes.iter().map(|&b| char::from(b)).collect();
    let mut found = Vec::new();
    let mut i = 0;
    while i < text.len() {
        match prefix_at(&text, i) {
            Some(end) => {
                let path_end = (end..text.len())
                    .find(|&j| matches!(text[j], '\0' | '"' | '\'') || text[j].is_whitespace())
                    .unwrap_or(text.len());
                let path: String = text[i..path_end].iter().collect();
                if !allow_under.iter().any(|dir| path.starts_with(dir.as_str())) {
                    found.push(text[i..end].iter().collect());
                }
                i = path_end;
            }
            None => i += 1,
        }
    }
    found.sort();
    found.dedup();
    found
}

fn starts(text: &[char], at: usize, s: &str) -> bool {
    s.chars()
        .enumerate()
        .all(|(k, c)| text.get(at + k) == Some(&c))
}

fn is_sep(c: Option<&char>) -> bool {
    matches!(c, Some('/' | '\\'))
}

/// The end of a home prefix (just past the separator after the user name) starting at `i`.
fn prefix_at(text: &[char], i: usize) -> Option<usize> {
    let name_start = if starts(text, i, "/Users/") {
        i + 7
    } else if starts(text, i, "/home/") {
        i + 6
    } else if text[i].is_ascii_alphabetic()
        && (i == 0 || !(text[i - 1].is_ascii_alphanumeric() || text[i - 1] == '_'))
        && text.get(i + 1) == Some(&':')
        && is_sep(text.get(i + 2))
        && starts(text, i + 3, "Users")
        && is_sep(text.get(i + 8))
    {
        i + 9
    } else {
        return None;
    };
    let name_len = text[name_start..]
        .iter()
        .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        .count();
    let sep = name_start + name_len;
    (name_len > 0 && is_sep(text.get(sep))).then_some(sep + 1)
}

pub fn run(args: &[String]) -> Result<String, String> {
    let mut allow_under = Vec::new();
    let mut files = Vec::new();
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        if arg == "--allow-under" {
            allow_under.push(it.next().ok_or("--allow-under takes a directory")?.clone());
        } else {
            files.push(arg);
        }
    }
    if files.is_empty() {
        return Err("home-paths: no file given; a scan of nothing proves nothing".into());
    }
    let mut problems = Vec::new();
    for file in &files {
        let bytes = std::fs::read(Path::new(file)).map_err(|e| format!("{file}: {e}"))?;
        let found = home_paths_in(&bytes, &allow_under);
        if !found.is_empty() {
            problems.push(format!("{file}: {}", found.join(", ")));
        }
    }
    if problems.is_empty() {
        Ok(format!(
            "home-paths: PASS ({} file(s), no build-machine home directory)",
            files.len()
        ))
    } else {
        Err(format!(
            "home-paths: build-machine home directories (build with --remap-path-prefix):\n  {}",
            problems.join("\n  ")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::home_paths_in;

    fn found(text: &str) -> Vec<String> {
        home_paths_in(text.as_bytes(), &[])
    }

    #[test]
    fn every_home_form_is_found() {
        assert_eq!(
            found("x\0/Users/runner/.cargo/registry/a.rs\0"),
            ["/Users/runner/"]
        );
        assert_eq!(found("/home/runner/work/x.rs"), ["/home/runner/"]);
        assert_eq!(
            found(r"panic at C:\Users\runneradmin\.cargo\registry\src\lib.rs"),
            [r"C:\Users\runneradmin\"]
        );
        assert_eq!(found("d:/Users/ada/x"), ["d:/Users/ada/"]);
        assert_eq!(
            found("/Users/a/x /Users/a/y /Users/b/z"),
            ["/Users/a/", "/Users/b/"]
        );
    }

    #[test]
    fn mapped_and_unrelated_paths_are_not() {
        for clean in [
            "/cargo/registry/src/x.rs",
            "/sw/crates/sw-mcp/src/main.rs",
            r"D:\a\synchronized.work\synchronized.work\target\x.rs",
            "/rustc/0123abcd/library/std/src/panicking.rs",
            r"C:\Users\",
            "XC:\\Users\\ada\\",
            "/Users/",
        ] {
            assert!(found(clean).is_empty(), "{clean}");
        }
    }

    #[test]
    fn a_path_under_an_allowed_directory_is_skipped_and_only_that() {
        let dir = "/Users/runner/work/sw/target/release/build/openssl-sys-".to_owned();
        let bytes = b"/Users/runner/work/sw/target/release/build/openssl-sys-1/out/engines-3\0";
        assert!(home_paths_in(bytes, std::slice::from_ref(&dir)).is_empty());
        let mixed =
            b"/Users/runner/work/sw/target/release/build/openssl-sys-1/x\0/Users/runner/.cargo/y";
        assert_eq!(home_paths_in(mixed, &[dir]), ["/Users/runner/"]);
    }
}
