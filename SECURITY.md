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

- The macOS sandbox is Seatbelt (`sandbox-exec`); other platforms have no sandbox yet.
- Folders listed under `link` come from the worker's tree and are not part of the checked snapshot; receipts say so.
- Receipts are sealed with a hash, not yet signed.
