# ADR-0004: Windows full sandbox, an opt-in admin setup

Status: accepted (2026-10-22, by the maintainer), shipped as **experimental** until tried on real Windows machines
beyond CI. Follows [ADR-0003](0003-windows-sandbox.md).

## The problem

v0.8.0's Windows sandbox (ADR-0003) runs as the developer's own account with a write-restricted token. It confines
writes for `pwsh`/`cmd` checks, but:

1. **Git for Windows' `sh` can't run in it** (MSYS secures its shared memory and signal pipes to the user account), so
   `sh` checks, the default shell, run unsandboxed.
2. **The internet isn't blocked**: the only unprivileged way (an AppContainer) can't run Git's `sh`, most toolchains or
   unrestricted PowerShell (CI evidence in ADR-0003); a firewall rule needs admin rights.

Both have the same cause: the check runs as the developer. A firewall rule can't single out "the check" from "the
developer", and MSYS objects belong to the developer.

## Decision

**An opt-in, one-time admin setup creates a dedicated low-rights account; checks then run as that account, inside the
existing write-restricted token and Job Object.** Without the setup, nothing changes (v0.8.0's partial sandbox).

```text
countercheck windows-setup            # once, as admin
countercheck windows-setup --remove   # undo everything
countercheck windows-setup --allow-read C:\Users\me\AppData\Local\Programs\Python   # optional, see below
```

### Setup (admin, once; PowerShell's built-in cmdlets, no new unsafe code)

1. A local account `countercheck-runner`: random 32-character password, never expires, can't change it, member of
   `Users` only (never admin), hidden from the sign-in screen.
2. A Windows Firewall rule, outbound, **block**, for that account only (`-LocalUser`), for every remote address except
   loopback (`127.0.0.0/8`, `::1`), so localhost keeps working.
3. The account's profile is created once (its first sign-in), so the first check isn't slow.
4. `%ProgramData%\countercheck\runner.json`: account name, SID, profile path, firewall rule name, and the password
   encrypted with machine-scope DPAPI. The folder's permissions are reset to Administrators + SYSTEM full control,
   Users read, so a non-admin (the agent) can't change it.
5. `--allow-read DIR`: give the account read access to a toolchain outside Program Files (a per-user Python, a
   repository's `node_modules`/`.venv` target), because it can't read the developer's profile.

The password is **not** a security boundary against the agent: any local process can decrypt machine-scope DPAPI. It
only lets countercheck start processes as the account; an agent that used it would get an account with *fewer* rights
than its own and no internet. What the agent can't do without admin is remove the firewall rule or change the config.

### Each check (no admin)

1. `verify` reads `runner.json`, grants the account full control on the check folder and its temp dir (as v0.8.0
   does for the run's SID) and copies the countercheck binary into a `bin` folder the account may read and run but
   not write (so an earlier check can't swap it).
2. The helper (`sandbox-exec`, as the developer) grants the account on `writes` locations, decrypts the password and
   starts **the same helper as the account** with `CreateProcessWithLogonW` (the documented way for a non-admin to
   start a process as another user: Windows' Secondary Logon service), suspended, in a kill-on-close Job Object, with
   the check's environment (profile variables pointed at the account's profile) and its stdout/stderr.
3. That second helper runs the check exactly like v0.8.0, with the account's token write-restricted to {account SID,
   logon SID, Everyone, RESTRICTED}: writes only where the account was granted (check folder, temp dir, `writes`, its
   own dedicated profile). Because MSYS's objects now belong to the account, **Git's `sh` runs sandboxed**.
4. The firewall rule blocks the account's internet; loopback is excluded, so web tests keep working.
5. Timeouts, exit codes and grant removal work as in v0.8.0; closing the outer job stops everything.

### `network = true` and the receipt

- A check with `network = true` runs as the developer in v0.8.0's mode (restricted token; `sh` unsandboxed), marked on
  the receipt per check. (A second account without the firewall rule would sandbox it fully; a follow-up.)
- Label: `windows-runner (no internet, localhost only; writes only in the check folder; checks run as
  countercheck-runner)`. `--sandbox auto` picks it only when a probe passes: the account starts, a refused write is
  refused, and the firewall rule exists, is enabled and blocks outbound. Otherwise it falls back to v0.8.0's mode and
  says why. If a step fails at run time, the check fails (exit 125).

### New unsafe code

Two Win32 calls, in `crates/countercheck-winsandbox` under ADR-0003's rules (one operation per block, `SAFETY`
comments, safe public API): `CreateProcessWithLogonW` and `CryptUnprotectData`. Setup uses PowerShell cmdlets
(`New-LocalUser`, `New-NetFirewallRule`, `ProtectedData`), no FFI.

## Alternatives rejected

| Option | Why not |
|---|---|
| AppContainer mode | Can't run Git's `sh`, most toolchains or unrestricted PowerShell (ADR-0003 evidence) |
| A Windows service (SYSTEM) that starts checks as the account, no stored password | A privileged service accepting requests from unprivileged users is a local privilege-escalation surface; far more to secure than a low-rights password |
| Run as the account without a restricted token | `Users` can write in `C:\ProgramData`, `C:\Users\Public` and new folders under `C:\`; the restricted token removes that |
| Windows Sandbox (the built-in VM) | Needs Pro/Enterprise and the feature enabled, starts slowly, and has none of the developer's toolchains |

## Limits

- Admin needed once; in enterprises, IT would deploy it (backlog, ROADMAP "Enterprise").
- Toolchains and linked folders in the developer's profile need `--allow-read` once.
- `network = true` checks run in v0.8.0's mode (above).
- Policies that forbid extra local accounts (some enterprises) rule it out; v0.8.0's mode remains.
- Processes started through a Windows service (Task Scheduler, WMI) still run outside the Job Object.

## Tests it needs

CI (`windows-latest` runs as admin): run the existing suite, then `windows-setup`, then runner tests:
`sh` and `cmd` checks write inside and are refused outside; `sh` runs sandboxed (no `sandboxed: false`); internet
refused, localhost works; a `network = true` check reaches the internet and is marked; a background process is
stopped; the probe falls back when the firewall rule is removed; then `windows-setup --remove` leaves no account,
rule or config. Unit tests: config parsing, the environment block, the password generator.

## Follow-ups (from the approval)

1. **No stored password.** The password is harmless (the account has fewer rights than the agent and no internet),
   but "a password local users can read" is what an enterprise security review notices first. Options: a per-machine
   service that starts checks as the account by S4U logon (no password, but a privileged component to secure), or
   IT-managed credentials in the enterprise packaging (ROADMAP "Enterprise").
2. **`network = true` checks fully sandboxed.** Today they run as the developer in v0.8.0's mode. A second account
   without the firewall rule (`countercheck-runner-net`) would give them the same write confinement and `sh` support.
3. **Try it on real Windows 11 machines** (a non-admin developer after an admin ran the setup, a per-user Python with
   `--allow-read`, a repository `.venv`) before dropping "experimental".
