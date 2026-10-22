# Windows sandbox evidence (not shipped)

A standalone program, **not a member of the Cargo workspace and not part of the `countercheck` binary**, that CI's
`AppContainer evidence (windows)` job runs on `windows-latest` to measure what an AppContainer allows: the internet,
localhost within one process and between two processes of the same container, Git for Windows' `sh`, a Python
outside Program Files, and with or without the (admin-only) loopback exemption, both as the runner's admin account
and as a fresh standard user. Its results are summarized in
[ADR-0003](../../docs/decisions/0003-windows-sandbox.md).

It contains `unsafe` (the bare `CreateAppContainerProfile` + `CreateProcessW` calls). That is acceptable here only
because nothing here ships; the workspace's `unsafe_code = "forbid"` does not apply to it (it has its own
`[workspace]`). Run it on Windows with `cargo run --manifest-path eval/windows-evidence/Cargo.toml`.
