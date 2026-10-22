//! countercheck's Windows check sandbox: the Win32 calls it needs, behind a safe API.
//!
//! The rest of the workspace forbids `unsafe`. This crate is the one exception, kept small so it
//! can be reviewed line by line: no published crate offers these calls with a safe API that we
//! would trust as a referee's security boundary (see docs/decisions/0003-windows-sandbox.md).
//!
//! What it does, unprivileged (no admin, no Developer Mode):
//! - [`sddl`] (pure, every OS): add and remove one allow entry in a file's permissions, written
//!   in Windows' SDDL text form, so the logic is unit-tested off Windows too.
//! - `sys` (Windows): read and write a file's permissions (`GetFileSecurityW`/`SetFileSecurityW`,
//!   which change one object only, never a whole tree, and never follow into other folders on
//!   their own), and start a process with a **write-restricted token**
//!   (`CreateRestrictedToken(WRITE_RESTRICTED)` + `CreateProcessAsUserW`): every write access
//!   check must then also pass for one of a short list of SIDs, so the process can write only
//!   where its run's own SID was granted, while it reads everything the user can.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]
#![cfg_attr(not(windows), forbid(unsafe_code))]

pub mod sddl;

#[cfg(windows)]
mod sys;
#[cfg(windows)]
pub use sys::{Child, Restricted, current_sids, file_dacl, set_file_dacl, spawn_restricted};
