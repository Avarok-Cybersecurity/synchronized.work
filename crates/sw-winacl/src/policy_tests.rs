#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

use super::*;

const ME: &str = "S-1-5-21-1004336348-1177238915-682003330-1001";
const OTHER: &str = "S-1-5-21-1004336348-1177238915-682003330-1002";

fn check(sddl: &str) -> Result<(), String> {
    check_owner_only(sddl, &[ME])
}

#[test]
fn what_this_crate_writes_is_owner_only() {
    for object in [Object::File, Object::Directory, Object::Pipe] {
        assert_eq!(check(&owner_only_sddl(object, ME)), Ok(()), "{object:?}");
    }
}

#[test]
fn what_windows_reads_back_is_owner_only() {
    // GetNamedSecurityInfo on a file this crate secured, then inside a secured directory.
    assert_eq!(check(&format!("O:{ME}D:PAI(A;;FA;;;{ME})")), Ok(()));
    assert_eq!(check(&format!("O:{ME}D:P(A;OICIID;FA;;;{ME})")), Ok(()));
    // A deny ACE only narrows access.
    assert_eq!(
        check(&format!("O:{ME}D:P(D;;FW;;;WD)(A;;FA;;;{ME})")),
        Ok(())
    );
    // A group section, if Windows adds one, changes nothing.
    assert_eq!(check(&format!("O:{ME}G:SYD:P(A;;FA;;;{ME})")), Ok(()));
}

#[test]
fn an_alias_for_this_user_counts_as_this_user() {
    // The built-in Administrator account prints as LA.
    assert_eq!(check_owner_only("O:LAD:P(A;;FA;;;LA)", &[ME, "LA"]), Ok(()));
    assert!(check_owner_only("O:LAD:P(A;;FA;;;LA)", &[ME]).is_err());
}

#[test]
fn a_grant_to_anyone_else_is_refused() {
    for who in ["WD", "BU", "AU", "SY", "BA", "CO", OTHER] {
        let sddl = format!("O:{ME}D:P(A;;FA;;;{ME})(A;;FR;;;{who})");
        let err = check(&sddl).expect_err(who);
        assert!(err.contains(who), "{err}");
    }
}

#[test]
fn an_inherited_dacl_is_refused() {
    // What a file under %LOCALAPPDATA% gets by default: SYSTEM, Administrators and the user.
    let default = format!("O:{ME}D:AI(A;ID;FA;;;SY)(A;ID;FA;;;BA)(A;ID;FA;;;{ME})");
    assert!(check(&default).is_err());
    // Even with only this user's ACE, a DACL that still inherits is not owner-only.
    let err = check(&format!("O:{ME}D:AI(A;ID;FA;;;{ME})")).unwrap_err();
    assert!(err.contains("protected"), "{err}");
}

#[test]
fn another_owner_is_refused() {
    let err = check(&format!("O:{OTHER}D:P(A;;FA;;;{ME})")).unwrap_err();
    assert!(err.contains("owned by"), "{err}");
    assert!(check(&format!("O:BAD:P(A;;FA;;;{ME})")).is_err());
}

#[test]
fn a_null_or_missing_dacl_is_refused() {
    assert!(check(&format!("O:{ME}D:NO_ACCESS_CONTROL")).is_err());
    assert!(check(&format!("O:{ME}")).is_err());
    assert!(check(&format!("D:P(A;;FA;;;{ME})")).is_err());
}

#[test]
fn an_ace_this_check_cannot_judge_is_refused() {
    let object = format!("O:{ME}D:P(OA;;FA;bf967aba-0de6-11d0-a285-00aa003049e2;;WD)");
    assert!(check(&object).is_err());
    let callback = format!("O:{ME}D:P(XA;;FA;;;WD;(@User.Title==\"PM\"))");
    assert!(check(&callback).is_err());
    assert!(check("not sddl").is_err());
}

#[test]
fn owner_of_reads_the_alias_windows_prints() {
    assert_eq!(owner_of("O:LAD:P(A;;FA;;;LA)"), Some("LA"));
    assert_eq!(owner_of(&format!("O:{ME}")), Some(ME));
    assert_eq!(owner_of("garbage"), None);
}
