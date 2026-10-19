//! `countercheck`: checks whether an AI worker's "done" is really done, and writes a receipt.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

mod git;
mod hook;
mod runner;
mod scan;
mod snapshot;
mod verify;

use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser)]
#[command(
    name = "countercheck",
    version,
    about = "Proof that AI work really happened: re-runs your checks in a clean copy the worker can't edit, and writes a receipt."
)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Clone, Copy, ValueEnum)]
enum SandboxArg {
    /// The macOS sandbox when it can start here, otherwise none (the receipt says which).
    Auto,
    /// No network, writes only inside the check folder (macOS).
    Seatbelt,
    /// Run checks as a normal process.
    None,
}

#[derive(Subcommand)]
enum Cmd {
    /// Verify the current work against the checks committed at the base revision.
    Verify {
        /// Repository to verify (any folder inside it).
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// The revision before the worker's change. Use your main branch for committed work.
        #[arg(long, default_value = "HEAD")]
        base: String,
        /// What the worker said it did, e.g. "fixed the bug, all tests pass".
        #[arg(long)]
        claim: Option<String>,
        /// Which agent did the work (claude-code, codex, hermes...).
        #[arg(long)]
        agent: Option<String>,
        /// Which model did the work.
        #[arg(long)]
        model: Option<String>,
        /// Cost of the work in US dollars, if the caller knows it.
        #[arg(long)]
        cost_usd: Option<f64>,
        #[arg(long, value_enum, default_value = "auto")]
        sandbox: SandboxArg,
        /// Print the receipt as JSON instead of a summary.
        #[arg(long)]
        json: bool,
        /// Where to save receipts (default: <repo>/.countercheck/receipts).
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Write a starter .countercheck/checks.toml.
    Init {
        #[arg(long, default_value = ".")]
        repo: PathBuf,
    },
    /// Check that a saved receipt hasn't been edited, and print it.
    Show { receipt: PathBuf },
    /// Run as an agent hook (reads the agent's hook JSON on stdin).
    Hook {
        #[command(subcommand)]
        hook: HookCmd,
    },
}

#[derive(Subcommand)]
enum HookCmd {
    /// Hermes `pre_verify` hook: keep Hermes working while the checks fail.
    HermesPreVerify,
    /// Claude Code (and Cortex Code) `Stop` hook: block the stop while the checks fail.
    ClaudeStop,
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            eprintln!("countercheck: {e}");
            ExitCode::from(1)
        }
    }
}

fn run(cli: Cli) -> Result<u8, String> {
    match cli.command {
        Cmd::Verify {
            repo,
            base,
            claim,
            agent,
            model,
            cost_usd,
            sandbox,
            json,
            out,
        } => {
            let sandbox = match sandbox {
                SandboxArg::Auto if runner::seatbelt_works() => runner::Sandbox::Seatbelt,
                SandboxArg::Auto => {
                    eprintln!(
                        "note: no check sandbox on this system; checks run as a normal process \
                         (the receipt records this). Run inside a container with no network for isolation."
                    );
                    runner::Sandbox::None
                }
                SandboxArg::Seatbelt => runner::Sandbox::Seatbelt,
                SandboxArg::None => runner::Sandbox::None,
            };
            if sandbox == runner::Sandbox::Seatbelt
                && !std::path::Path::new("/usr/bin/sandbox-exec").exists()
            {
                return Err("the seatbelt sandbox needs macOS; use --sandbox none".to_string());
            }
            if sandbox == runner::Sandbox::Seatbelt && !runner::seatbelt_works() {
                return Err(
                    "the macOS sandbox could not start (macOS doesn't allow a sandbox inside another \
                     sandbox, e.g. when countercheck runs inside a countercheck check); use --sandbox none"
                        .to_string(),
                );
            }
            let opts = verify::Options {
                repo,
                base,
                claim,
                worker: countercheck_core::receipt::Worker {
                    agent,
                    model,
                    cost_usd,
                },
                sandbox,
                out,
            };
            let (receipt, path) = verify::verify(&opts)?;
            if json {
                let text = serde_json::to_string_pretty(&receipt).map_err(|e| e.to_string())?;
                println!("{text}");
            } else {
                print!("{}", receipt.render_text());
                println!("  receipt:  {}", path.display());
            }
            Ok(u8::try_from(receipt.verdict.exit_code()).unwrap_or(1))
        }
        Cmd::Init { repo } => {
            let root = git::toplevel(&repo)?;
            let path = root.join(countercheck_core::config::CHECKS_PATH);
            if path.exists() {
                return Err(format!("{} already exists", path.display()));
            }
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
            std::fs::write(&path, countercheck_core::config::TEMPLATE)
                .map_err(|e| e.to_string())?;
            println!(
                "wrote {}\nedit the check commands, then commit it: countercheck only trusts checks that are in the base revision.",
                path.display()
            );
            Ok(0)
        }
        Cmd::Hook { hook: which } => {
            let result = match which {
                HookCmd::HermesPreVerify => hook::hermes_pre_verify(),
                HookCmd::ClaudeStop => hook::claude_stop(),
            };
            // Fail open: a broken referee must never stop the agent.
            if let Err(e) = result {
                eprintln!("countercheck hook: {e}");
            }
            Ok(0)
        }
        Cmd::Show { receipt } => {
            let text = std::fs::read_to_string(&receipt)
                .map_err(|e| format!("{}: {e}", receipt.display()))?;
            let r: countercheck_core::receipt::Receipt =
                serde_json::from_str(&text).map_err(|e| format!("not a receipt: {e}"))?;
            print!("{}", r.render_text());
            if r.seal_is_valid() {
                println!("  seal:     intact");
                Ok(0)
            } else {
                println!("  seal:     BROKEN (this receipt was edited after it was written)");
                Ok(4)
            }
        }
    }
}
