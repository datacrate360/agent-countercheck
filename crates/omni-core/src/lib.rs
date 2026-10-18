//! omni-agent core: the pure parts of "is it really done?".
//!
//! - [`config`]: the check file (`.omni/checks.toml`) a project commits.
//! - [`protect`]: which paths a worker may not change (tests, the check file itself).
//! - [`receipt`]: the record of one verification and how its verdict is decided.
//!
//! Nothing here touches the file system, processes or the network; the `omni` binary does that.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

pub mod config;
pub mod protect;
pub mod receipt;

/// Hex SHA-256 of some bytes.
pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for b in digest {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn sha256_of_empty_input() {
        assert_eq!(
            super::sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }
}
