//! Edit a file's permissions (its DACL) in SDDL text form: add one allow entry for a SID, or
//! remove every entry for it. Pure string work, so it is tested on every OS.
//!
//! The text comes from `ConvertSecurityDescriptorToStringSecurityDescriptorW` with only the DACL
//! asked for, e.g. `D:PAI(A;OICIID;FA;;;SY)(A;OICIID;FA;;;S-1-5-21-1-2-3-1001)`. Anything we
//! don't understand is an error, never a guess: the caller then doesn't touch the file.

/// Longest DACL text accepted (a file with thousands of entries is not ours to edit).
const MAX_LEN: usize = 256 * 1024;

/// A DACL split into its header (`D:` plus flags like `PAI`), its entries without the outer
/// parentheses, and whatever follows them (normally nothing).
#[derive(Debug, PartialEq, Eq)]
struct Dacl<'a> {
    head: &'a str,
    aces: Vec<&'a str>,
    tail: &'a str,
}

fn parse(sddl: &str) -> Result<Dacl<'_>, String> {
    if sddl.len() > MAX_LEN {
        return Err("the permissions are too long to edit".to_string());
    }
    let body = sddl
        .strip_prefix("D:")
        .ok_or_else(|| format!("unexpected permissions text: {}", short(sddl)))?;
    // Flags (P, AI, AR) up to the first entry.
    let first = body.find('(').unwrap_or(body.len());
    let flags = body.get(..first).unwrap_or("");
    if !flags.chars().all(|c| matches!(c, 'P' | 'A' | 'I' | 'R')) {
        return Err(format!("unexpected permissions text: {}", short(sddl)));
    }
    let head = sddl.get(..2 + first).unwrap_or("D:");
    let mut aces = Vec::new();
    let mut rest = body.get(first..).unwrap_or("");
    while rest.starts_with('(') {
        // Conditional entries nest parentheses: find the matching one.
        let mut depth = 0usize;
        let mut end = None;
        for (i, c) in rest.char_indices() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        end = Some(i);
                        break;
                    }
                }
                _ => {}
            }
        }
        let end = end.ok_or_else(|| format!("unbalanced permissions text: {}", short(sddl)))?;
        aces.push(rest.get(1..end).unwrap_or(""));
        rest = rest.get(end + 1..).unwrap_or("");
    }
    if !rest.is_empty() {
        return Err(format!("unexpected permissions text: {}", short(sddl)));
    }
    Ok(Dacl {
        head,
        aces,
        tail: rest,
    })
}

fn short(s: &str) -> String {
    s.chars().take(120).collect()
}

/// The fields of one entry: type;flags;rights;object;inherit object;trustee[;condition].
fn fields(ace: &str) -> Vec<&str> {
    ace.splitn(7, ';').collect()
}

fn trustee_is(ace: &str, sid: &str) -> bool {
    fields(ace)
        .get(5)
        .is_some_and(|t| t.eq_ignore_ascii_case(sid))
}

fn inherited(ace: &str) -> bool {
    fields(ace).get(1).is_some_and(|f| f.contains("ID"))
}

fn join(d: &Dacl<'_>, aces: &[String]) -> String {
    let mut out = String::from(d.head);
    for a in aces {
        out.push('(');
        out.push_str(a);
        out.push(')');
    }
    out.push_str(d.tail);
    out
}

/// A SID in string form, as we write it into an entry: `S-` and digits and dashes only.
fn check_sid(sid: &str) -> Result<(), String> {
    let ok = sid.len() <= 184
        && sid.starts_with("S-1-")
        && sid.get(4..).is_some_and(|r| {
            r.split('-')
                .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
        });
    if ok {
        Ok(())
    } else {
        Err(format!("not a SID: {}", short(sid)))
    }
}

/// `sddl` with one entry added that allows `sid` full control (`FA`), inherited by everything
/// created inside when `inherit` is set (a folder). Placed after the explicit entries and before
/// the inherited ones (the order Windows keeps), so an explicit deny still wins. A DACL that is
/// absent (`D:NO_ACCESS_CONTROL`: anyone may do anything) is returned as is.
pub fn add_full_control(sddl: &str, sid: &str, inherit: bool) -> Result<String, String> {
    check_sid(sid)?;
    if sddl == "D:NO_ACCESS_CONTROL" {
        return Ok(sddl.to_string());
    }
    let d = parse(sddl)?;
    let flags = if inherit { "OICI" } else { "" };
    let entry = format!("A;{flags};FA;;;{sid}");
    if d.aces.iter().any(|a| a.eq_ignore_ascii_case(&entry)) {
        return Ok(sddl.to_string());
    }
    let at = d
        .aces
        .iter()
        .position(|a| inherited(a))
        .unwrap_or(d.aces.len());
    let mut aces: Vec<String> = d.aces.iter().map(|a| a.to_string()).collect();
    aces.insert(at, entry);
    Ok(join(&d, &aces))
}

/// `sddl` without any entry for `sid` (explicit or inherited), and whether anything was removed.
pub fn remove_sid(sddl: &str, sid: &str) -> Result<(String, bool), String> {
    check_sid(sid)?;
    if sddl == "D:NO_ACCESS_CONTROL" {
        return Ok((sddl.to_string(), false));
    }
    let d = parse(sddl)?;
    let kept: Vec<String> = d
        .aces
        .iter()
        .filter(|a| !trustee_is(a, sid))
        .map(|a| a.to_string())
        .collect();
    let removed = kept.len() != d.aces.len();
    Ok((join(&d, &kept), removed))
}

/// The SID that marks one sandboxed run, from four random numbers: shaped like a local account
/// (`S-1-5-21-a-b-c-rid`) that no machine or domain has, so no other token ever carries it.
pub fn run_sid(random: [u32; 4]) -> String {
    let [a, b, c, d] = random;
    format!("S-1-5-21-{a}-{b}-{c}-{}", 1000 + d % 1_000_000)
}

/// A token's default permissions for what its processes create (pipes, child processes,
/// threads): full control for each SID. A write-restricted process can open its own objects
/// only if one of its restricting SIDs is in here.
pub fn default_dacl(sids: &[&str]) -> Result<String, String> {
    let mut out = String::from("D:");
    for s in sids {
        check_sid(s)?;
        out.push_str(&format!("(A;;GA;;;{s})"));
    }
    out.push_str("(A;;GA;;;SY)");
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUN: &str = "S-1-5-21-11-22-33-1044";

    #[test]
    fn an_entry_is_added_after_the_explicit_ones_and_before_the_inherited() {
        let s = "D:AI(D;;WD;;;S-1-5-21-9-9-9-1000)(A;;FA;;;BA)(A;OICIID;FA;;;SY)(A;OICIID;FA;;;S-1-5-21-1-2-3-1001)";
        let out = add_full_control(s, RUN, true).unwrap();
        assert_eq!(
            out,
            format!(
                "D:AI(D;;WD;;;S-1-5-21-9-9-9-1000)(A;;FA;;;BA)(A;OICI;FA;;;{RUN})(A;OICIID;FA;;;SY)(A;OICIID;FA;;;S-1-5-21-1-2-3-1001)"
            )
        );
        // Adding it twice changes nothing; a file gets no inheritance flags.
        assert_eq!(add_full_control(&out, RUN, true).unwrap(), out);
        assert_eq!(
            add_full_control("D:(A;;FA;;;SY)", RUN, false).unwrap(),
            format!("D:(A;;FA;;;SY)(A;;FA;;;{RUN})")
        );
        assert_eq!(
            add_full_control("D:P", RUN, false).unwrap(),
            format!("D:P(A;;FA;;;{RUN})")
        );
    }

    #[test]
    fn every_entry_for_the_sid_is_removed_and_nothing_else() {
        let s = format!(
            "D:PAI(A;OICI;FA;;;{RUN})(A;;0x1200a9;;;WD)(A;OICIID;FA;;;{RUN})(XA;;FX;;;S-1-1-0;(WIN://SYSAPPID Contains \"x\"))"
        );
        let (out, removed) = remove_sid(&s, RUN).unwrap();
        assert!(removed);
        assert_eq!(
            out,
            "D:PAI(A;;0x1200a9;;;WD)(XA;;FX;;;S-1-1-0;(WIN://SYSAPPID Contains \"x\"))"
        );
        let (again, removed) = remove_sid(&out, RUN).unwrap();
        assert!(!removed);
        assert_eq!(again, out);
        // A SID that only starts the same is another SID.
        let other = format!("D:(A;;FA;;;{RUN}5)");
        assert_eq!(remove_sid(&other, RUN).unwrap(), (other.clone(), false));
    }

    #[test]
    fn what_is_not_understood_is_refused() {
        for bad in [
            "O:BAD:(A;;FA;;;SY)",
            "D:(A;;FA;;;SY",
            "D:(A;;FA;;;SY)S:(AU;;FA;;;WD)",
            "D:XYZ(A;;FA;;;SY)",
            "",
        ] {
            assert!(add_full_control(bad, RUN, true).is_err(), "{bad}");
            assert!(remove_sid(bad, RUN).is_err(), "{bad}");
        }
        for sid in ["", "S-1-", "S-1-5-)(A;;FA;;;WD", "WD", "S-1-5--1"] {
            assert!(add_full_control("D:", sid, true).is_err(), "{sid}");
        }
        // No DACL means no restriction at all: left alone.
        assert_eq!(
            add_full_control("D:NO_ACCESS_CONTROL", RUN, true).unwrap(),
            "D:NO_ACCESS_CONTROL"
        );
    }

    #[test]
    fn run_sids_look_like_local_accounts_and_differ() {
        let a = run_sid([1, 2, 3, 4]);
        assert_eq!(a, "S-1-5-21-1-2-3-1004");
        assert!(check_sid(&a).is_ok());
        assert_ne!(a, run_sid([1, 2, 3, 5]));
        assert!(check_sid(&run_sid([u32::MAX; 4])).is_ok());
    }

    #[test]
    fn the_default_dacl_lists_each_sid_and_system() {
        assert_eq!(
            default_dacl(&[RUN, "S-1-5-5-0-1234"]).unwrap(),
            format!("D:(A;;GA;;;{RUN})(A;;GA;;;S-1-5-5-0-1234)(A;;GA;;;SY)")
        );
        assert!(default_dacl(&["BA)(A;;GA;;;WD"]).is_err());
    }
}
