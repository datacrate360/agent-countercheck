//! `countercheck`: checks whether an AI worker's "done" is really done, and writes a receipt.
#![forbid(unsafe_code)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

mod external;
mod git;
mod golden;
mod hook;
#[cfg(target_os = "linux")]
mod linux_sandbox;
mod progress;
mod report;
mod runner;
mod scan;
mod snapshot;
mod ui;
mod verify;
#[cfg(windows)]
mod windows_runner;
#[cfg(windows)]
mod windows_sandbox;

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
    /// The best sandbox that can start here (macOS, Linux or Windows), otherwise none (the
    /// receipt says which).
    Auto,
    /// No network, writes only inside the check folder (macOS).
    Seatbelt,
    /// Writes only inside the check folder; no network where user namespaces are allowed (Linux).
    Landlock,
    /// Writes only inside the check folder; the internet is not blocked (Windows).
    Windows,
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
        /// Which agent did the work (claude-code, codex, hermes...). Default: the agent whose shell
        /// this is, when it marks it (Claude Code, Cursor, Codex, Hermes).
        #[arg(long)]
        agent: Option<String>,
        /// Which model did the work.
        #[arg(long)]
        model: Option<String>,
        /// What this agent is for, e.g. "UI design" or "security review", to tell several agents
        /// on one project apart (default: the COUNTERCHECK_ROLE environment variable).
        #[arg(long)]
        role: Option<String>,
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
        /// Read the checks from outside the repository: `<suite repo>@<full commit id>:<path>`, or a
        /// file with --checks-sha256 (without the hash the checks aren't locked: UNVERIFIED).
        #[arg(long, value_name = "SOURCE")]
        checks_from: Option<String>,
        /// The SHA-256 the --checks-from file must have (pins it).
        #[arg(long, value_name = "HEX", requires = "checks_from")]
        checks_sha256: Option<String>,
    },
    /// (internal) Run one check inside the Linux or Windows sandbox; started by `verify`.
    #[command(hide = true)]
    SandboxExec {
        /// Print what the sandbox can do here and exit.
        #[arg(long)]
        probe: bool,
        #[arg(long)]
        work: Option<PathBuf>,
        #[arg(long)]
        tmp: Option<PathBuf>,
        #[arg(long = "write")]
        writes: Vec<PathBuf>,
        #[arg(long)]
        network: bool,
        #[arg(long)]
        namespaces: bool,
        #[arg(long, default_value_t = 600)]
        timeout_secs: u64,
        /// Windows: the run's SID, which the check's writes are restricted to.
        #[arg(long)]
        sid: Option<String>,
        /// Windows: the check's whole command line.
        #[arg(long)]
        cmdline: Option<String>,
        /// Windows: run the check as the runner account, through this copy of countercheck.
        #[arg(long)]
        runner_exe: Option<PathBuf>,
        /// Windows: print whether the full sandbox (`windows-setup`) works here and exit.
        #[arg(long)]
        probe_runner: bool,
        /// Windows: write the probe's answer to this file (the runner account's probe).
        #[arg(long)]
        probe_to: Option<PathBuf>,
        #[arg(last = true)]
        program: Vec<String>,
    },
    /// Windows: set up the full check sandbox once, as administrator (a low-rights account whose
    /// internet is blocked; checks run as it). `--remove` undoes it.
    WindowsSetup {
        /// Remove the account, its firewall rule and its settings.
        #[arg(long)]
        remove: bool,
        /// Let the account read and run a toolchain outside Program Files (repeatable).
        #[arg(long = "allow-read", value_name = "DIR")]
        allow_read: Vec<PathBuf>,
    },
    /// Write a starter .countercheck/checks.toml.
    Init {
        #[arg(long, default_value = ".")]
        repo: PathBuf,
    },
    /// Check that a saved receipt hasn't been edited, and print it.
    Show { receipt: PathBuf },
    /// Serve a live page of the receipts on this computer (127.0.0.1 only, with a random token).
    Ui {
        /// Repository whose receipts to show (any folder inside it).
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Receipt folder (default: <repo>/.countercheck/receipts).
        #[arg(long)]
        receipts: Option<PathBuf>,
        /// Use exactly this port (0 = any free one). Default: 7357, or any free port if taken.
        #[arg(long)]
        port: Option<u16>,
        /// Don't open the browser.
        #[arg(long)]
        no_open: bool,
    },
    /// Write an HTML page of the saved receipts (newest first; no scripts, works offline).
    Report {
        /// Repository whose receipts to show (any folder inside it).
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Receipt folder (default: <repo>/.countercheck/receipts).
        #[arg(long)]
        receipts: Option<PathBuf>,
        /// Where to write the page (default: <receipts>/report.html).
        #[arg(long)]
        out: Option<PathBuf>,
        /// Open it in the browser.
        #[arg(long)]
        open: bool,
    },
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
    /// Session start (Claude Code `SessionStart`, Hermes `on_session_start`): remember the commit
    /// the session started from, so the checks stay locked even if the agent commits.
    SessionStart,
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
        Cmd::SandboxExec {
            probe,
            work,
            tmp,
            writes,
            network,
            namespaces,
            timeout_secs,
            sid,
            cmdline,
            runner_exe,
            probe_runner,
            probe_to,
            program,
        } => sandbox_exec(SandboxExecArgs {
            probe,
            work,
            tmp,
            writes,
            network,
            namespaces,
            timeout_secs,
            sid,
            cmdline,
            runner_exe,
            probe_runner,
            probe_to,
            program,
        }),
        Cmd::WindowsSetup { remove, allow_read } => windows_setup(remove, &allow_read),
        Cmd::Verify {
            repo,
            base,
            claim,
            agent,
            model,
            role,
            cost_usd,
            sandbox,
            json,
            out,
            checks_from,
            checks_sha256,
        } => {
            let sandbox_arg = sandbox;
            let sandbox = match sandbox {
                SandboxArg::Auto if runner::auto_sandbox() != runner::Sandbox::None => {
                    let s = runner::auto_sandbox();
                    if matches!(s, runner::Sandbox::Landlock { namespaces: false }) {
                        eprintln!(
                            "note: this system doesn't allow user namespaces, so checks can reach the internet \
                             (writes are still confined; the receipt records this)."
                        );
                    }
                    s
                }
                SandboxArg::Auto if cfg!(windows) => {
                    eprintln!(
                        "note: the Windows check sandbox could not start here; checks run as a normal \
                         process (the receipt records this)."
                    );
                    runner::Sandbox::None
                }
                SandboxArg::Auto => {
                    eprintln!(
                        "note: no check sandbox on this system; checks run as a normal process \
                         (the receipt records this). Run inside a container with no network for isolation."
                    );
                    runner::Sandbox::None
                }
                SandboxArg::Seatbelt => runner::Sandbox::Seatbelt,
                SandboxArg::Landlock => match runner::auto_sandbox() {
                    s @ runner::Sandbox::Landlock { .. } => s,
                    _ => {
                        return Err(
                            "the Linux sandbox (Landlock) isn't available here; use --sandbox none"
                                .to_string(),
                        );
                    }
                },
                SandboxArg::Windows => match runner::auto_sandbox() {
                    s @ (runner::Sandbox::RestrictedToken | runner::Sandbox::WindowsRunner) => s,
                    _ if cfg!(windows) => {
                        return Err(
                            "the Windows sandbox could not start here (`countercheck sandbox-exec --probe` \
                             says why); use --sandbox none"
                                .to_string(),
                        );
                    }
                    _ => {
                        return Err(
                            "the Windows sandbox needs Windows; use --sandbox auto".to_string()
                        );
                    }
                },
                SandboxArg::None => runner::Sandbox::None,
            };
            if sandbox == runner::Sandbox::Seatbelt && cfg!(windows) {
                return Err(
                    "seatbelt is the macOS sandbox; on Windows use --sandbox windows (or auto)"
                        .to_string(),
                );
            }
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
            let sandbox_required = matches!(sandbox_arg, SandboxArg::Windows);
            let opts = verify::Options {
                repo,
                base,
                claim,
                worker: countercheck_core::receipt::Worker {
                    // Not named: the agent whose shell this is, if it marks it.
                    agent: agent.or_else(|| verify::agent_from_env().map(str::to_string)),
                    model,
                    cost_usd,
                    role: role
                        .or_else(|| std::env::var("COUNTERCHECK_ROLE").ok())
                        .and_then(|r| {
                            countercheck_core::receipt::clean_label(
                                &r,
                                countercheck_core::receipt::MAX_ROLE_CHARS,
                            )
                        }),
                    session: verify::session_from_env(),
                },
                sandbox,
                sandbox_required,
                out,
                checks_from,
                checks_sha256,
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
                HookCmd::SessionStart => hook::session_start(),
            };
            // Fail open: a broken referee must never stop the agent.
            if let Err(e) = result {
                eprintln!("countercheck hook: {e}");
            }
            Ok(0)
        }
        Cmd::Ui {
            repo,
            receipts,
            port,
            no_open,
        } => {
            ui::run(&ui::Options {
                repo,
                receipts,
                port,
                open: !no_open,
            })?;
            Ok(0)
        }
        Cmd::Report {
            repo,
            receipts,
            out,
            open,
        } => {
            let (path, n) = report::run(&report::Options {
                repo,
                receipts,
                out,
                open,
            })?;
            println!("wrote {} ({n} run(s))", path.display());
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

/// The hidden `sandbox-exec` subcommand's arguments (each system reads its own).
#[allow(dead_code)]
struct SandboxExecArgs {
    probe: bool,
    work: Option<PathBuf>,
    tmp: Option<PathBuf>,
    writes: Vec<PathBuf>,
    network: bool,
    namespaces: bool,
    timeout_secs: u64,
    sid: Option<String>,
    cmdline: Option<String>,
    runner_exe: Option<PathBuf>,
    probe_runner: bool,
    probe_to: Option<PathBuf>,
    program: Vec<String>,
}

#[cfg(windows)]
fn windows_setup(remove: bool, allow_read: &[PathBuf]) -> Result<u8, String> {
    windows_runner::setup_main(remove, allow_read)
}

#[cfg(not(windows))]
fn windows_setup(_remove: bool, _allow_read: &[PathBuf]) -> Result<u8, String> {
    Err("windows-setup is for Windows; macOS and Linux need no setup".to_string())
}

#[cfg(target_os = "linux")]
fn sandbox_exec(a: SandboxExecArgs) -> Result<u8, String> {
    if a.probe {
        return Ok(linux_sandbox::probe_main());
    }
    let (Some(work), Some(tmp)) = (a.work, a.tmp) else {
        return Err("sandbox-exec needs --work and --tmp".to_string());
    };
    Ok(linux_sandbox::run_main(linux_sandbox::Run {
        work,
        tmp,
        writes: a.writes,
        network: a.network,
        timeout_secs: a.timeout_secs,
        namespaces: a.namespaces,
        program: a.program,
    }))
}

#[cfg(windows)]
fn sandbox_exec(a: SandboxExecArgs) -> Result<u8, String> {
    if a.probe {
        return Ok(windows_sandbox::probe_main());
    }
    if a.probe_runner {
        return Ok(windows_runner::probe_main());
    }
    if let Some(file) = &a.probe_to {
        return Ok(windows_sandbox::probe_to_main(file));
    }
    let (Some(work), Some(sid), Some(cmdline)) = (a.work, a.sid, a.cmdline) else {
        return Err("sandbox-exec needs --work, --sid and --cmdline".to_string());
    };
    let code = windows_sandbox::run_main(windows_sandbox::Run {
        work,
        writes: a.writes,
        sid,
        timeout_secs: a.timeout_secs,
        cmdline,
        runner_exe: a.runner_exe,
    });
    // Windows exit codes are 32-bit: pass the check's on unchanged (a u8 could turn a crash into 0).
    std::process::exit(code)
}

#[cfg(not(any(target_os = "linux", windows)))]
fn sandbox_exec(_: SandboxExecArgs) -> Result<u8, String> {
    Err("sandbox-exec is the Linux and Windows sandbox; it doesn't run on this system".to_string())
}
