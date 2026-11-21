//! The owner-only rule, over SDDL text (pure: no Win32 call, tested on every platform).
//!
//! An object is owner-only when this user owns it, its DACL is protected (nothing inherited
//! from the parent can widen it later), and every access-allowed ACE names this user. Deny ACEs
//! only narrow access, so they are accepted; any other ACE kind (object, callback, conditional
//! or audit ACEs) is refused, because this rule cannot say what it grants.

/// The SDDL this crate writes: owner and protected DACL with one ACE for `sid`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Object {
    /// A file: full file access.
    File,
    /// A directory: full access, inherited by every file and directory created in it.
    Directory,
    /// A named pipe: generic all.
    Pipe,
}

/// The owner-only security descriptor for `object`, owned by and granting only `sid`.
pub fn owner_only_sddl(object: Object, sid: &str) -> String {
    let ace = match object {
        Object::File => "A;;FA;;;",
        Object::Directory => "A;OICI;FA;;;",
        Object::Pipe => "A;;GA;;;",
    };
    format!("O:{sid}D:P({ace}{sid})")
}

/// The owner and DACL sections of an SDDL string (`O:…G:…D:…S:…`, any of them absent).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Sections<'a> {
    owner: Option<&'a str>,
    dacl: Option<&'a str>,
}

fn sections(sddl: &str) -> Result<Sections<'_>, String> {
    let mut owner = None;
    let mut dacl = None;
    let mut rest = sddl;
    while !rest.is_empty() {
        let (key, body) = match rest.as_bytes() {
            [k @ (b'O' | b'G' | b'D' | b'S'), b':', ..] => (*k, &rest[2..]),
            _ => return Err(format!("not an SDDL section: {rest:?}")),
        };
        // A section runs to the next `X:` at parenthesis depth 0 (ACE bodies hold no section).
        let mut depth = 0usize;
        let mut end = body.len();
        for (i, c) in body.char_indices() {
            match c {
                '(' => depth += 1,
                ')' => depth = depth.saturating_sub(1),
                ':' if depth == 0 && i > 0 => {
                    end = i - 1;
                    break;
                }
                _ => {}
            }
        }
        let value = &body[..end];
        match key {
            b'O' => owner = Some(value),
            b'D' => dacl = Some(value),
            _ => {}
        }
        rest = &body[end..];
    }
    Ok(Sections { owner, dacl })
}

/// Why `sddl` is not owner-only for the user called `me` (their SID string, plus the SDDL alias
/// Windows prints for it, if any), or `Ok` when it is.
pub fn check_owner_only(sddl: &str, me: &[&str]) -> Result<(), String> {
    let s = sections(sddl)?;
    let owner = s.owner.ok_or("no owner in the security descriptor")?;
    if !me.contains(&owner) {
        return Err(format!("owned by {owner}, not by this user"));
    }
    let dacl = s.dacl.ok_or("no DACL: everyone has full access")?;
    let (flags, aces) = dacl.split_at(dacl.find('(').unwrap_or(dacl.len()));
    if flags.contains("NO_ACCESS_CONTROL") {
        return Err("a NULL DACL: everyone has full access".into());
    }
    if flags.replace("AI", "").replace("AR", "") != "P" {
        return Err(format!(
            "the DACL inherits from its parent (flags {flags:?}); expected protected"
        ));
    }
    let inner = aces
        .strip_prefix('(')
        .and_then(|a| a.strip_suffix(')'))
        .unwrap_or(aces);
    for ace in inner.split(")(").filter(|a| !a.is_empty()) {
        let fields: Vec<&str> = ace.split(';').collect();
        match fields.as_slice() {
            ["D", _, _, _, _, _] => {}
            ["A", _, _, _, _, trustee] if me.contains(trustee) => {}
            ["A", _, rights, _, _, trustee] => {
                return Err(format!(
                    "grants {rights} to {trustee}, not only to this user"
                ));
            }
            _ => return Err(format!("an ACE this check cannot judge: ({ace})")),
        }
    }
    Ok(())
}

/// The owner field of an SDDL string (`O:<owner>`), used to learn the alias Windows prints for
/// this user's SID by round-tripping `O:<sid>` through the OS.
pub fn owner_of(sddl: &str) -> Option<&str> {
    sections(sddl).ok()?.owner
}

#[cfg(test)]
#[path = "policy_tests.rs"]
mod tests;
