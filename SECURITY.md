# Security policy

Countercheck runs other people's check commands and decides whether AI work is accepted, so security problems matter
twice: a bug can let unsafe code run, or let a false "done" through.

## Reporting a vulnerability

Report privately through GitHub: **Security → Report a vulnerability** on
[datacrate360/agent-countercheck](https://github.com/datacrate360/agent-countercheck/security). Please include steps to
reproduce and the version (`countercheck --version`). Don't open a public issue for a security problem.

We aim to reply within a few days and to credit reporters who want credit.

## In scope

- Escaping the check sandbox (network access, or writes outside the check folder).
- A worker changing what is checked without the receipt saying so (bypassing the base-revision lock or protected
  files).
- Forging or editing a receipt without the seal breaking.
- Path traversal or symlink tricks in the snapshot.

## Known limits (not vulnerabilities)

- The macOS sandbox is Seatbelt (`sandbox-exec`); the Linux sandbox is Landlock plus user, network and PID namespaces
  (`countercheck sandbox-exec`). Where a Linux system blocks unprivileged user namespaces, checks can reach the
  internet (writes stay confined) and the receipt says so.
- The Windows sandbox is a write-restricted token plus a Job Object (`countercheck sandbox-exec`,
  [ADR-0003](docs/decisions/0003-windows-sandbox.md)). It confines writes but **does not block the internet** (the only unprivileged way, an AppContainer, can't run most toolchains; the
  receipt says so), and only `shell = "pwsh"`/`"cmd"` checks can run in it: Git for
  Windows' `sh` (MSYS) can't start under it, so `sh` checks run unsandboxed: each is marked `"sandboxed": false`
  with the reason, and the run's label says `partial: sh checks ran unsandboxed`. Not covered: drives without permissions (FAT/exFAT, some network
  shares), files and folders that grant write to `Everyone` or `RESTRICTED` (or have no DACL), and processes started through a Windows service
  (Task Scheduler, WMI, COM), which run outside the Job Object.
- With `countercheck windows-setup` (opt-in, admin once; ADR-0004), Windows checks run as a low-rights local account,
  `countercheck-runner`, inside the same restricted token and Job Object, and a firewall rule blocks that account's
  internet (not loopback). Its password is stored with machine-scope DPAPI in `%ProgramData%\countercheck` (admins can
  change it, users can read it): it is not a secret from the agent, only the means to start processes as an account
  with fewer rights than the agent's own. A check with `network = true` runs as the developer.
- `crates/countercheck-winsandbox` is the only crate allowed to use `unsafe` (Win32 calls with no safe crate); every
  block carries a `SAFETY:` note, and a bug there is in scope.
- Folders listed under `link` come from the worker's tree and are not part of the checked snapshot; receipts say so.
  Countercheck refuses to vouch for a run when one points back into the project (an editable install) and, in a
  hooked session, when one changed since the session started; a change made before the session, or between runs of
  a hand-run `verify`, is not detected. Run the checks in CI with a fresh environment for a guarantee.
- Receipts are sealed with a hash, not yet signed.
