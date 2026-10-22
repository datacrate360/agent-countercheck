# ADR-0003: The Windows check sandbox

Status: proposed (draft PR, for the maintainer to decide), 2026-10-22.

## The goal

`countercheck verify` on Windows 10/11 should give the guarantees macOS (Seatbelt) and Linux (Landlock + namespaces)
give, without admin rights:

1. writes only inside the check folder, its temp dir and `writes = [...]` locations;
2. no internet, with localhost still working (web tests talk over localhost), unless the check has `network = true`;
3. everything the check started stops when it ends.

## What Windows offers without admin

| Mechanism | Writes | Internet | Localhost between processes | Admin? | Reads the user's toolchain |
|---|---|---|---|---|---|
| **AppContainer** (Edge, Store apps) | confined | blocked without `internetClient` | **blocked** (any capability set) | profile: no. Loopback exemption: **yes** | no: only folders that grant `ALL APPLICATION PACKAGES` or the container's SID (System32, Program Files); not `C:\hostedtoolcache`, `%LOCALAPPDATA%\Programs\Python`, the repository's `node_modules` |
| **Write-restricted token** (`CreateRestrictedToken(WRITE_RESTRICTED)`, Chromium's GPU process, Windows services) | confined to objects that grant a restricting SID | not blocked | works | no | yes, unchanged |
| Low integrity level | confined, except folders labelled low (`AppData\LocalLow`, `%TEMP%\Low`) | not blocked | works | no | yes |
| Windows Filtering Platform / firewall rules | - | blocked per program or SID | works | **yes** | - |
| Job Object (already used) | - | - | - | no | - |

The AppContainer column was measured on the CI runner (`windows-latest`) by the `appcontainer-evidence` job
(`.github/appcontainer-probe`, which uses the third-party `rappct` crate only there). See "Evidence" below.

## Decision

**Writes: a write-restricted token. Process tree: the Job Object, as before. Internet: not blocked, and the receipt
says so.** The label is `windows-restricted-token (writes only in the check folder; internet NOT blocked: Windows
needs admin rights to block it and keep localhost)`, the same honest shape as Linux without user namespaces.

How it works (`apps/countercheck/src/windows_sandbox.rs`, `countercheck sandbox-exec`, mirroring `linux_sandbox.rs`):

1. One random SID per `verify` run, shaped like a local account no machine has (`S-1-5-21-a-b-c-rid`).
2. `verify` grants that SID full control on the check folder and its temp dir while they are still empty, as an
   inheritable entry, so every file copied in inherits it and no tree is walked.
3. Per check, the helper grants the SID on each `writes` location (created if missing) and everything already in it,
   walking the tree itself and never entering or changing a symlink or junction; afterwards it removes every entry for
   the SID again, including on what the check created there.
4. The check runs with this user's token, restricted: all privileges but traverse removed, and `WRITE_RESTRICTED`
   with the restricting SIDs {run SID, logon-session SID, `Everyone`, `RESTRICTED`}. Every write access check must
   then pass both for the user's normal groups and for one restricting SID; only the check folder, temp dir and
   `writes` grant the run SID. `Everyone` and `RESTRICTED` are there, as in Chromium's restricted tokens and Windows'
   write-restricted services, because system objects are granted to them: without them the first CI run showed
   MSYS `bash` dying at start (`NtCreateDirectoryObject(\BaseNamedObjects\msys-2.0S5-…): 0xC0000022`), .NET failing
   to load `BCrypt.dll`, and `cmd`'s `>NUL` refused. On disk, almost nothing grants them write.
   The token's default DACL lists the user, the run SID and the logon SID, so the check can still open what it
   creates (pipes, its child processes). Only stdin/stdout/stderr are inherited (`PROC_THREAD_ATTRIBUTE_HANDLE_LIST`).
5. The check starts suspended, joins a kill-on-close Job Object, then runs with the check's timeout; the job is closed
   when it ends, stopping everything it started.
6. `--probe` makes a restricted process write once where it may and once where it may not; `--sandbox auto` uses the
   sandbox only when that refused write really was refused. If any setup step fails at run time, the check fails
   (exit 125) and never runs with less than the receipt says.

Why not AppContainer, which also blocks the internet: without admin it breaks two things the other platforms keep.
Localhost between processes is blocked (a test server and the browser or test client that talks to it are separate
processes); the loopback exemption (`CheckNetIsolation LoopbackExempt`, `NetworkIsolationSetAppContainerConfig`) is a
machine-wide firewall setting that needs admin. And the check can't read the user's own toolchain (a Python in
`%LOCALAPPDATA%`, the repository's linked `node_modules`) unless we add permission entries to those folders, which
changes files outside the check on every run. A sandbox that fails honest work would make people turn it off.

## The `unsafe` question

The workspace forbids `unsafe`. No safe crate covers restricted tokens and per-file DACL edits:

- `win32job` (already used) and `junction` cover only their own APIs.
- `rappct` (AppContainer/LPAC, MIT) covers AppContainer launch and ACL grants, but has no restricted-token launch
  and no ACL revoke, has one author, one year of history, and `unwrap()` in its launch path. It is used only in the
  CI evidence job, not shipped.
- The `windows` / `windows-sys` crates are bindings; every call is `unsafe`.

So the FFI lives in **one small crate, `crates/countercheck-winsandbox`**, built on Microsoft's `windows-sys`
bindings. It is the only crate that allows `unsafe`, its lints deny `unsafe` blocks without a `SAFETY:` comment, and
the logic that can be wrong without being unsafe (editing permission text) is a pure module tested on every OS. What
it calls, and why each is sound, is commented at each block. The maintainer decides whether this exception is
acceptable; the alternative is no Windows sandbox.

## Known limits (stated in SECURITY.md)

- **Internet not blocked** (see above). Run inside a VM or a Windows container with networking off when that matters.
- **Places anyone may write**: a write-restricted process can still write where a file or folder grants write to
  `Everyone` or `RESTRICTED` (rare; a sloppy installer's folder in `C:\ProgramData`, for example) or has no DACL at
  all; and drives without permissions (FAT/exFAT USB sticks, some network shares) don't restrict writes.
- **Processes started through a Windows service** (Task Scheduler, WMI, a COM server) run outside the Job Object.
- Leftover permission entries: if the helper itself is killed, an entry for a SID no token will ever carry again can
  stay on a `writes` folder (harmless, but visible in `icacls`).

## Evidence

See the PR description for the CI runs.
