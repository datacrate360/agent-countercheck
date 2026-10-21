//! The Linux check sandbox: `countercheck sandbox-exec` (hidden), started by the runner for every
//! check, sets itself up and then runs the check.
//!
//! In order, all without root and without `unsafe` in this crate:
//! 1. **Subreaper** (`PR_SET_CHILD_SUBREAPER`): a process the check orphans, even one that called
//!    `setsid` or forked twice, is re-parented to this helper instead of `init`, so it can be found
//!    and killed when the check ends.
//! 2. **Namespaces**, when the system allows unprivileged user namespaces: a private network
//!    namespace with only loopback up (no internet, localhost works, like the macOS sandbox),
//!    and a private PID namespace in which the check is PID 1, so everything it started dies with
//!    it. Skipped for the network part when the check has `network = true`.
//! 3. **Landlock**: writes only in the check folder, its temp dir, `writes = [...]` locations,
//!    `/dev/null` and `/dev/shm`; signals and abstract UNIX sockets can't reach outside processes
//!    (where the kernel supports it).
//! 4. Run the check with its timeout, then kill whatever is left.
//!
//! What each system actually gives is reported by `--probe`, and the receipt's sandbox label says
//! it; nothing here is claimed that wasn't enforced. If a step the probe promised fails, the check
//! fails (exit 125) rather than running with less protection than the receipt says.

use landlock::{
    ABI, Access, AccessFs, CompatLevel, Compatible, Ruleset, RulesetAttr, RulesetCreatedAttr,
    RulesetStatus, Scope, path_beneath_rules,
};
use nix::sched::{CloneFlags, unshare};
use nix::sys::signal::{Signal, kill};
use nix::sys::socket::{
    AddressFamily, MsgFlags, NetlinkAddr, SockFlag, SockProtocol, SockType, recv, sendto, socket,
};
use nix::sys::wait::{WaitPidFlag, waitpid};
use nix::unistd::Pid;
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;
use wait_timeout::ChildExt;

/// Exit code when the sandbox itself couldn't be set up (the check didn't run).
pub const SETUP_FAILED: u8 = 125;
/// Exit code when the check ran out of time (the runner reports it as a timeout).
pub const TIMED_OUT: u8 = 124;
/// Printed (to the check's output) when the check ran out of time.
pub const TIMEOUT_LINE: &str = "[countercheck] the check timed out";

/// The ABI we ask Landlock for; older kernels give a subset (reported as partial).
const LANDLOCK_ABI: ABI = ABI::V6;

/// What the sandbox can do on this system (from `--probe`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Caps {
    /// Landlock confines writes.
    pub landlock: bool,
    /// User + network + PID namespaces work.
    pub namespaces: bool,
}

/// Options for one sandboxed run.
pub struct Run {
    pub work: PathBuf,
    pub tmp: PathBuf,
    pub writes: Vec<PathBuf>,
    pub network: bool,
    pub timeout_secs: u64,
    /// Use namespaces (the probe found them working).
    pub namespaces: bool,
    pub program: Vec<String>,
}

/// Map our uid/gid into a new user namespace, so we keep our identity and get the capabilities
/// needed to set up the network namespace.
fn enter_namespaces(network: bool) -> Result<(), String> {
    let uid = nix::unistd::getuid();
    let gid = nix::unistd::getgid();
    let mut flags = CloneFlags::CLONE_NEWUSER | CloneFlags::CLONE_NEWPID;
    if !network {
        flags |= CloneFlags::CLONE_NEWNET;
    }
    unshare(flags).map_err(|e| format!("unshare: {e}"))?;
    std::fs::write("/proc/self/setgroups", "deny").map_err(|e| format!("setgroups: {e}"))?;
    std::fs::write("/proc/self/uid_map", format!("{uid} {uid} 1\n"))
        .map_err(|e| format!("uid_map: {e}"))?;
    std::fs::write("/proc/self/gid_map", format!("{gid} {gid} 1\n"))
        .map_err(|e| format!("gid_map: {e}"))?;
    if !network {
        loopback_up()?;
    }
    Ok(())
}

/// Bring `lo` up in the new network namespace with one rtnetlink RTM_NEWLINK message
/// (`ip link set lo up`), so localhost works while everything else is unreachable.
fn loopback_up() -> Result<(), String> {
    const RTM_NEWLINK: u16 = 16;
    const NLM_F_REQUEST: u16 = 1;
    const NLM_F_ACK: u16 = 4;
    const NLMSG_ERROR: u16 = 2;
    const IFF_UP: u32 = 1;
    let fd = socket(
        AddressFamily::Netlink,
        SockType::Raw,
        SockFlag::empty(),
        SockProtocol::NetlinkRoute,
    )
    .map_err(|e| format!("netlink socket: {e}"))?;
    let mut msg = Vec::with_capacity(32);
    msg.extend_from_slice(&32u32.to_ne_bytes()); // nlmsg_len
    msg.extend_from_slice(&RTM_NEWLINK.to_ne_bytes()); // nlmsg_type
    msg.extend_from_slice(&(NLM_F_REQUEST | NLM_F_ACK).to_ne_bytes()); // nlmsg_flags
    msg.extend_from_slice(&1u32.to_ne_bytes()); // nlmsg_seq
    msg.extend_from_slice(&0u32.to_ne_bytes()); // nlmsg_pid
    msg.push(0); // ifi_family = AF_UNSPEC
    msg.push(0); // padding
    msg.extend_from_slice(&0u16.to_ne_bytes()); // ifi_type
    msg.extend_from_slice(&1i32.to_ne_bytes()); // ifi_index: lo is 1 in a new namespace
    msg.extend_from_slice(&IFF_UP.to_ne_bytes()); // ifi_flags
    msg.extend_from_slice(&IFF_UP.to_ne_bytes()); // ifi_change
    sendto(
        fd.as_raw_fd(),
        &msg,
        &NetlinkAddr::new(0, 0),
        MsgFlags::empty(),
    )
    .map_err(|e| format!("netlink send: {e}"))?;
    let mut buf = [0u8; 256];
    let n = recv(fd.as_raw_fd(), &mut buf, MsgFlags::empty())
        .map_err(|e| format!("netlink recv: {e}"))?;
    let reply = buf.get(..n).unwrap_or(&[]);
    let kind = reply
        .get(4..6)
        .and_then(|b| <[u8; 2]>::try_from(b).ok())
        .map_or(0, u16::from_ne_bytes);
    let error = reply
        .get(16..20)
        .and_then(|b| <[u8; 4]>::try_from(b).ok())
        .map_or(-1, i32::from_ne_bytes);
    if kind != NLMSG_ERROR || error != 0 {
        return Err(format!(
            "could not bring loopback up (netlink error {error})"
        ));
    }
    Ok(())
}

/// Confine writes (and, where supported, signals and abstract sockets) to the check.
fn landlock(run: &Run) -> Result<RulesetStatus, String> {
    let write = AccessFs::from_write(LANDLOCK_ABI);
    let mut allowed: Vec<PathBuf> = vec![run.work.clone(), run.tmp.clone()];
    allowed.extend(run.writes.iter().cloned());
    allowed.push(PathBuf::from("/dev/null"));
    // POSIX shared memory and semaphores (Python multiprocessing) live here; it's a tmpfs.
    allowed.push(PathBuf::from("/dev/shm"));
    let ruleset = Ruleset::default()
        .set_compatibility(CompatLevel::BestEffort)
        .handle_access(write)
        .map_err(|e| format!("landlock: {e}"))?
        .scope(Scope::from_all(LANDLOCK_ABI))
        .map_err(|e| format!("landlock: {e}"))?
        .create()
        .map_err(|e| format!("landlock: {e}"))?
        .add_rules(path_beneath_rules(
            allowed.iter().filter(|p| p.exists()),
            write,
        ))
        .map_err(|e| format!("landlock: {e}"))?;
    let status = ruleset
        .restrict_self()
        .map_err(|e| format!("landlock: {e}"))?;
    Ok(status.ruleset)
}

/// Every process below `me`, from `/proc` (`ppid` chains).
fn descendants(me: i32) -> Vec<i32> {
    let mut parent = std::collections::HashMap::new();
    let Ok(dir) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    for e in dir.flatten() {
        let Some(pid) = e.file_name().to_str().and_then(|s| s.parse::<i32>().ok()) else {
            continue;
        };
        let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
            continue;
        };
        // `pid (comm) state ppid ...`; comm may contain spaces and parentheses.
        let ppid = stat
            .rsplit_once(')')
            .and_then(|(_, rest)| rest.split_whitespace().nth(1))
            .and_then(|s| s.parse::<i32>().ok());
        if let Some(ppid) = ppid {
            parent.insert(pid, ppid);
        }
    }
    let mut out = Vec::new();
    let mut frontier = vec![me];
    while let Some(p) = frontier.pop() {
        for (&child, &pp) in &parent {
            if pp == p && child != me && !out.contains(&child) {
                out.push(child);
                frontier.push(child);
            }
        }
    }
    out
}

/// Kill everything the check left behind: stop all first (so nothing forks meanwhile), then kill,
/// reap, and repeat until nothing is left.
fn kill_leftovers() {
    let me = std::process::id() as i32;
    for _ in 0..20 {
        let left = descendants(me);
        if left.is_empty() {
            break;
        }
        for &p in &left {
            let _ = kill(Pid::from_raw(p), Signal::SIGSTOP);
        }
        for &p in &left {
            let _ = kill(Pid::from_raw(p), Signal::SIGKILL);
        }
        while let Ok(s) = waitpid(None, Some(WaitPidFlag::WNOHANG)) {
            if s == nix::sys::wait::WaitStatus::StillAlive {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Set everything up in this process (it is about to exit), for `--probe` and for real runs.
fn setup(run: &Run) -> Result<RulesetStatus, String> {
    nix::sys::prctl::set_child_subreaper(true).map_err(|e| format!("subreaper: {e}"))?;
    if run.namespaces {
        enter_namespaces(run.network)?;
    }
    landlock(run)
}

/// `countercheck sandbox-exec --probe`: what works here, as `landlock=<0|1> namespaces=<0|1>`.
pub fn probe_main() -> u8 {
    // Namespaces first, in a throwaway child of this process (they can't be undone).
    let tmp = std::env::temp_dir();
    let mut run = Run {
        work: tmp.clone(),
        tmp,
        writes: Vec::new(),
        network: false,
        timeout_secs: 5,
        namespaces: true,
        program: vec!["true".to_string()],
    };
    let namespaces = enter_namespaces(false).is_ok();
    run.namespaces = false;
    let landlock = matches!(
        landlock(&run),
        Ok(RulesetStatus::FullyEnforced | RulesetStatus::PartiallyEnforced)
    );
    println!(
        "landlock={} namespaces={}",
        u8::from(landlock),
        u8::from(namespaces)
    );
    0
}

/// Parse `--probe` output.
pub fn parse_probe(out: &str) -> Caps {
    Caps {
        landlock: out.contains("landlock=1"),
        namespaces: out.contains("namespaces=1"),
    }
}

/// What this system's sandbox can do, probed once per process with the given binary.
pub fn probe(exe: &std::path::Path) -> Caps {
    static CAPS: std::sync::OnceLock<Caps> = std::sync::OnceLock::new();
    *CAPS.get_or_init(|| {
        Command::new(exe)
            .args(["sandbox-exec", "--probe"])
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| parse_probe(&String::from_utf8_lossy(&o.stdout)))
            .unwrap_or(Caps {
                landlock: false,
                namespaces: false,
            })
    })
}

/// `countercheck sandbox-exec ... -- <program> <args>`: set up, run, clean up, pass the exit code on.
pub fn run_main(run: Run) -> u8 {
    match setup(&run) {
        Ok(RulesetStatus::FullyEnforced | RulesetStatus::PartiallyEnforced) => {}
        Ok(RulesetStatus::NotEnforced) => {
            eprintln!(
                "[countercheck] the Linux sandbox could not confine writes here (Landlock not enforced)"
            );
            return SETUP_FAILED;
        }
        Err(e) => {
            eprintln!("[countercheck] the Linux sandbox could not start: {e}");
            return SETUP_FAILED;
        }
    }
    let Some((program, args)) = run.program.split_first() else {
        eprintln!("[countercheck] sandbox-exec: no command");
        return SETUP_FAILED;
    };
    let mut child = match Command::new(program).args(args).spawn() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[countercheck] could not start {program}: {e}");
            return SETUP_FAILED;
        }
    };
    let status = child.wait_timeout(Duration::from_secs(run.timeout_secs));
    let code = match status {
        Ok(Some(s)) => {
            use std::os::unix::process::ExitStatusExt;
            s.code()
                .or_else(|| s.signal().map(|sig| 128 + sig))
                .unwrap_or(1)
        }
        _ => {
            let _ = child.kill();
            let _ = child.wait();
            eprintln!("{TIMEOUT_LINE} after {} s", run.timeout_secs);
            i32::from(TIMED_OUT)
        }
    };
    // In a PID namespace everything already died with the check (its PID 1); otherwise the
    // subreaper has collected the orphans.
    kill_leftovers();
    u8::try_from(code.clamp(0, 255)).unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_output_is_parsed() {
        assert_eq!(
            parse_probe("landlock=1 namespaces=0\n"),
            Caps {
                landlock: true,
                namespaces: false
            }
        );
        assert_eq!(
            parse_probe(""),
            Caps {
                landlock: false,
                namespaces: false
            }
        );
    }

    #[test]
    fn descendants_of_a_missing_process_are_none() {
        assert!(descendants(i32::MAX).is_empty());
    }
}
