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
| **AppContainer** (Edge, Store apps) | confined | blocked without `internetClient` (measured) | **works within one container** (measured as admin, no exemption); to other processes it needs the loopback exemption, which a standard user could add for their own container (measured) | profile: no | no: only folders that grant `ALL APPLICATION PACKAGES` or the container's SID (System32, Program Files); measured: Python in `C:\hostedtoolcache` can't be started; also `%LOCALAPPDATA%\Programs\Python`, the repository's `node_modules` |
| **Write-restricted token** (`CreateRestrictedToken(WRITE_RESTRICTED)`, Chromium's GPU process, Windows services) | confined to objects that grant a restricting SID | not blocked | works | no | yes, unchanged |
| Low integrity level | confined, except folders labelled low (`AppData\LocalLow`, `%TEMP%\Low`) | not blocked | works | no | yes |
| Windows Filtering Platform / firewall rules | - | blocked per program or SID | works | **yes** | - |
| Job Object (already used) | - | - | - | no | - |

The AppContainer row was measured on the CI runner (`windows-latest`) by the `AppContainer evidence (windows)` job
(`eval/windows-evidence`: a standalone program outside the workspace, never shipped). See "Evidence" below.

## Decision

**Writes: a write-restricted token, for `pwsh` and `cmd` checks. Process tree: the Job Object, as before. Internet:
not blocked, and the receipt says so. Git for Windows' `sh`: can't be confined (below), so it is decided per check: in a
run with `sh` checks, the `pwsh`/`cmd` checks still run in the sandbox, each `sh` check runs without it and carries
`"sandboxed": false` and `"unsandboxed_reason"` on the receipt, and the run's label becomes `windows-restricted-token,
partial: sh checks ran unsandboxed (Git for Windows can't run in a restricted token); ...`. A whole-run fallback was
rejected: `sh` is the default shell, so it would leave most Windows projects with no sandbox at all.
`--sandbox windows` refuses a run with `sh` checks.** Without `sh` checks the label is `windows-restricted-token (writes only in the check folder; internet NOT blocked)`,
the same honest shape as Linux without user namespaces.

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

Why not AppContainer now, although it also blocks the internet. I expected it to block localhost between processes
(Microsoft's UWP documentation says so); the CI evidence shows it does not **within one container** (a server and
a client the check starts are both inside), so the internet could be blocked with localhost kept. But it can't read
the user's own toolchain: measured, the runner's Python in `C:\hostedtoolcache` can't even be started, and the same
holds for a Python in `%LOCALAPPDATA%` or the repository's linked `node_modules`, unless we add permission entries to
those folders (changing files outside the check, recursively, on every run or once for good). Git for Windows' `sh`
fails in it too (measured, same MSYS error). A sandbox that fails honest work would make people turn it off, so this
PR ships the restricted token and leaves an AppContainer mode as the next step (below).

## Why Git for Windows' `sh` can't run in it

Measured on CI (second run): with every grant above, MSYS `bash` dies at start with `CreateFileMapping
<user SID>.1, Win32 error 5` or `couldn't create signal pipe, Win32 error 5`. MSYS gives its per-user shared memory
and its signal pipes explicit permissions naming only the user, `Administrators` and `SYSTEM`, and every MSYS
process of the user shares them. A write-restricted process can open them only if one of those SIDs is a restricting
SID, and each of them also has write on everything in the user's profile, so the restriction would mean nothing. A
low integrity level fails the same way on objects a normal Git Bash created, and an AppContainer can't open them
because they don't name the container. So `sh` checks are not confined, and the receipt never claims they are.

Without that limit (a follow-up needing a one-time admin setup, as OpenAI's Codex does on Windows): run checks as a
dedicated local user created once by an elevated `countercheck setup`, with ACLs giving that user write only in
the check folder, and a Windows Firewall rule for that user blocking everything but loopback. MSYS then works (its
objects belong to that user), and the internet can be blocked with localhost kept.

## The `unsafe` question

The workspace forbids `unsafe`. No safe crate covers restricted tokens and per-file DACL edits:

- `win32job` (already used) and `junction` cover only their own APIs.
- `rappct` (AppContainer/LPAC, MIT) covers AppContainer launch and ACL grants, but has no restricted-token launch
  and no ACL revoke, has one author, one year of history, and `unwrap()` in its launch path. Tried for the CI evidence,
  it failed every launch on the runner (`CreateProcessW` error 6 with null stdio, 203 otherwise); not used.
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

CI on `windows-latest` (Windows Server, Git 2.56.0.windows.2), PR #29:

- **Restricted token, first run**: MSYS `bash` died at start (`NtCreateDirectoryObject(\BaseNamedObjects\msys-2.0S5-…):
  0xC0000022`), PowerShell's .NET failed to load `BCrypt.dll`, `cmd`'s `>NUL` was refused: fixed by `Everyone` and
  `RESTRICTED` among the restricting SIDs.
- **Restricted token, second run**: `pwsh` and `cmd` checks pass; MSYS `bash` still dies (`CreateFileMapping <user
  SID>.1, Win32 error 5` / `couldn't create signal pipe, Win32 error 5`): the per-check `sh` rule above.
- **Restricted token, final**: [CI run 39716343023, job 119619357141](https://github.com/datacrate360/agent-countercheck/actions/runs/39716343023/job/119619357141)
  (commit `c567ac4`, `windows-latest`): the probe printed `restricted=1`; `tests\verify.rs` ran 99 tests, 99 passed,
  0 failed, 0 ignored, among them every Windows sandbox test (writes refused outside with the hint, `writes` opens
  exactly its folder and is undone, localhost between processes works, the internet is reachable as the label says,
  detached and timed-out processes are stopped, `sh` checks marked unsandboxed while `cmd` stays confined, a failed
  setup fails the check); the winsandbox, core, ui and unit test binaries passed too. Later commits only add a
  `--nocapture` CI step that prints each sandbox test's probe result, silence the probe's own refused write, and add
  a test for that.
- **AppContainer** (`eval/windows-evidence`, as the runner's admin account), PowerShell probes inside the container:

  | Setup | internet 1.1.1.1:443 | localhost, same process | localhost, another process (same container) | Git `sh` | Python in `C:\hostedtoolcache` |
  |---|---|---|---|---|---|
  | no capabilities | blocked | ok | ok | fails (MSYS `0xC0000022`) | can't be started |
  | `internetClient` + `privateNetworkClientServer` | ok | ok | ok | fails | can't be started |
  | no capabilities + loopback exemption | blocked | ok | ok | fails | can't be started |

  The same probe as a fresh standard (non-admin) user (`net session` fails), [CI job](https://github.com/datacrate360/agent-countercheck/actions/runs/39716343023/job/119619357282):
  the container profile is created without admin; Git `sh` fails the same way and Python in `C:\hostedtoolcache`
  can't be started; **adding the loopback exemption succeeded** (`CheckNetIsolation LoopbackExempt -a` printed "OK."
  and listed the container afterwards), so it is not admin-only for a profile the user owns. The network probes are
  **inconclusive** as that user: PowerShell ran in ConstrainedLanguage mode inside the container ("Only core types are
  supported in this language mode"), so they never reached the network. Measuring them needs a probe that isn't
  PowerShell (an AppContainer mode would need that work anyway).

## Next step (not in this PR)

1. **An AppContainer mode** for projects whose toolchain is readable from a container (Program Files, or folders the
   user grants once): internet blocked, localhost within the check kept, no admin. Needs a decision on granting the
   container's SID read access to toolchain folders.
2. **A one-time admin setup** for everything else (as OpenAI's Codex does on Windows): checks run as a dedicated local
   user, with write only in the check folder and a firewall rule for that user that allows only loopback. MSYS works
   then (its objects belong to that user), so `sh` checks could be sandboxed too.
