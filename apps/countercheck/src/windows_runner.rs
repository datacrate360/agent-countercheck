//! The Windows full sandbox (ADR-0004): an opt-in, one-time admin setup creates a low-rights
//! local account, `countercheck-runner`, and a firewall rule that blocks its internet (not
//! loopback). Checks then run as that account, inside the same write-restricted token and Job
//! Object as `windows_sandbox` (ADR-0003). Because Git for Windows' `sh` (MSYS) secures its shared
//! objects to the account it runs as, `sh` checks are sandboxed too.
//!
//! - `countercheck windows-setup` (admin) / `--remove`: PowerShell's own cmdlets, no FFI.
//! - `%ProgramData%\countercheck\runner.json`: account, SID, profile, rule and the password
//!   encrypted with machine-scope DPAPI. Not a secret from the agent (any local process can
//!   decrypt it), only the means to start processes as an account with fewer rights than the
//!   agent's own and no internet. Only admins can change the file or the rule.
//! - Per check: this process (the helper, as the developer) grants the account on `writes`
//!   locations, starts the helper again **as the account** (`CreateProcessWithLogonW`) in a
//!   kill-on-close Job Object, and that second helper runs the check write-restricted.

use countercheck_winsandbox as ffi;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// The account's name and the firewall rule's name.
pub const ACCOUNT: &str = "countercheck-runner";
pub const RULE: &str = "countercheck-runner-no-internet";

/// What `windows-setup` wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub version: u64,
    pub user: String,
    pub sid: String,
    pub profile: String,
    pub firewall_rule: String,
    /// The account's password, encrypted with machine-scope DPAPI, as hex.
    pub password: String,
    pub allow_read: Vec<String>,
}

/// `%ProgramData%\countercheck`.
fn config_dir() -> Option<PathBuf> {
    std::env::var_os("ProgramData").map(|p| PathBuf::from(p).join("countercheck"))
}

/// Parse `runner.json`; anything odd is "not set up".
pub fn parse_config(text: &str) -> Option<Config> {
    let v: serde_json::Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()?;
    let text_of = |k: &str| v.get(k).and_then(|x| x.as_str()).map(str::to_string);
    let c = Config {
        version: v.get("version")?.as_u64()?,
        user: text_of("user")?,
        sid: text_of("sid")?,
        profile: text_of("profile")?,
        firewall_rule: text_of("firewall_rule")?,
        password: text_of("password")?,
        // PowerShell writes a single item as a string and none as null.
        allow_read: match v.get("allow_read") {
            Some(serde_json::Value::Array(a)) => a
                .iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect(),
            Some(serde_json::Value::String(s)) => vec![s.clone()],
            _ => Vec::new(),
        },
    };
    let sid_ok = c.sid.starts_with("S-1-5-21-")
        && c.sid.len() < 100
        && c.sid
            .bytes()
            .all(|b| b.is_ascii_digit() || b == b'-' || b == b'S');
    let name_ok = !c.user.is_empty()
        && c.user.len() <= 20
        && c.user
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    let profile_ok = c.profile.len() > 3 && c.profile.get(1..3) == Some(":\\");
    (c.version == 1 && sid_ok && name_ok && profile_ok && !c.password.is_empty()).then_some(c)
}

/// The setup, if there is one.
pub fn load() -> Option<Config> {
    let text = std::fs::read_to_string(config_dir()?.join("runner.json")).ok()?;
    parse_config(&text)
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok())
        .collect()
}

/// The account's password, decrypted.
fn password(c: &Config) -> Result<String, String> {
    let blob = unhex(&c.password).ok_or("runner.json: the password isn't hex")?;
    let plain = ffi::unprotect(&blob).map_err(|e| format!("runner password: {e}"))?;
    String::from_utf8(plain).map_err(|_| "runner password: not UTF-8".to_string())
}

/// A random password that meets Windows' complexity rules (upper, lower, digit, symbol).
fn new_password() -> Result<String, String> {
    const SET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789";
    let mut buf = [0u8; 28];
    getrandom::fill(&mut buf).map_err(|e| format!("random password: {e}"))?;
    let mut s = String::from("Cc7!");
    for b in buf {
        s.push(char::from(SET[usize::from(b) % SET.len()]));
    }
    Ok(s)
}

/// The script `windows-setup` runs (Windows PowerShell 5.1, always present).
const SETUP: &str = r#"
$ErrorActionPreference = 'Stop'
$me = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if (-not $me.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
  Write-Output 'countercheck windows-setup needs an administrator PowerShell (Run as administrator).'
  exit 3
}
$name = $env:CC_ACCOUNT; $rule = $env:CC_RULE; $dir = $env:CC_DIR
$plain = $env:CC_PASSWORD
$pw = ConvertTo-SecureString $plain -AsPlainText -Force
$u = Get-LocalUser -Name $name -ErrorAction SilentlyContinue
if ($u) {
  Set-LocalUser -Name $name -Password $pw -PasswordNeverExpires $true -UserMayChangePassword $false
  Enable-LocalUser -Name $name
} else {
  New-LocalUser -Name $name -Password $pw -PasswordNeverExpires -UserMayNotChangePassword -AccountNeverExpires `
    -Description 'Agent Countercheck: runs checks with no internet (countercheck windows-setup)' | Out-Null
}
$sid = (Get-LocalUser -Name $name).SID.Value
$users = Get-LocalGroup -SID 'S-1-5-32-545'
if (-not (Get-LocalGroupMember -Group $users | Where-Object { $_.SID.Value -eq $sid })) {
  Add-LocalGroupMember -Group $users -Member $name
}
$key = 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon\SpecialAccounts\UserList'
New-Item -Path $key -Force | Out-Null
New-ItemProperty -Path $key -Name $name -Value 0 -PropertyType DWord -Force | Out-Null
Remove-NetFirewallRule -Name $rule -ErrorAction SilentlyContinue
$remote = @('0.0.0.0-126.255.255.255', '128.0.0.0-255.255.255.255', '::', '::2-ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff')
New-NetFirewallRule -Name $rule -DisplayName 'Agent Countercheck runner: no internet' `
  -Description 'Blocks the internet for checks run as countercheck-runner; loopback stays open.' `
  -Direction Outbound -Action Block -Profile Any -LocalUser "D:(A;;CC;;;$sid)" -RemoteAddress $remote | Out-Null
$cred = New-Object Management.Automation.PSCredential($name, $pw)
Start-Process -FilePath "$env:SystemRoot\System32\cmd.exe" -ArgumentList '/c', 'exit' -Credential $cred `
  -LoadUserProfile -Wait -WindowStyle Hidden -WorkingDirectory "$env:SystemRoot"
$profile = (Get-CimInstance Win32_UserProfile | Where-Object { $_.SID -eq $sid }).LocalPath
if (-not $profile) { throw "the account's profile was not created" }
Add-Type -AssemblyName System.Security
$blob = [Security.Cryptography.ProtectedData]::Protect([Text.Encoding]::UTF8.GetBytes($plain), $null,
  [Security.Cryptography.DataProtectionScope]::LocalMachine)
$hex = ($blob | ForEach-Object { $_.ToString('x2') }) -join ''
New-Item -ItemType Directory -Force -Path $dir | Out-Null
& icacls.exe $dir /inheritance:r /grant:r '*S-1-5-18:(OI)(CI)F' '*S-1-5-32-544:(OI)(CI)F' '*S-1-5-32-545:(OI)(CI)RX' | Out-Null
if ($LASTEXITCODE) { throw "icacls $dir failed" }
$allow = @()
foreach ($p in ($env:CC_ALLOW_READ -split '\|')) {
  if ($p) {
    & icacls.exe $p /grant "*$($sid):(OI)(CI)RX" /T /C /Q | Out-Null
    if ($LASTEXITCODE) { throw "icacls $p failed" }
    $allow += $p
  }
}
[ordered]@{ version = 1; user = $name; sid = $sid; profile = $profile; firewall_rule = $rule; password = $hex;
  allow_read = $allow } | ConvertTo-Json | Set-Content -Encoding ASCII -Path (Join-Path $dir 'runner.json')
Write-Output "countercheck-runner is set up (SID $sid): checks will run as it, with no internet."
"#;

/// The script `windows-setup --remove` runs.
const REMOVE: &str = r#"
$ErrorActionPreference = 'Stop'
$me = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if (-not $me.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
  Write-Output 'countercheck windows-setup --remove needs an administrator PowerShell (Run as administrator).'
  exit 3
}
$name = $env:CC_ACCOUNT; $rule = $env:CC_RULE; $dir = $env:CC_DIR
Remove-NetFirewallRule -Name $rule -ErrorAction SilentlyContinue
$u = Get-LocalUser -Name $name -ErrorAction SilentlyContinue
if ($u) {
  $sid = $u.SID.Value
  $cfg = Join-Path $dir 'runner.json'
  if (Test-Path $cfg) {
    foreach ($p in (Get-Content -Raw $cfg | ConvertFrom-Json).allow_read) {
      if ($p -and (Test-Path $p)) { & icacls.exe $p /remove "*$sid" /T /C /Q | Out-Null }
    }
  }
  Get-CimInstance Win32_UserProfile | Where-Object { $_.SID -eq $sid } | Remove-CimInstance -ErrorAction SilentlyContinue
  Remove-LocalUser -Name $name
}
$key = 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon\SpecialAccounts\UserList'
Remove-ItemProperty -Path $key -Name $name -ErrorAction SilentlyContinue
Remove-Item -Recurse -Force -Path $dir -ErrorAction SilentlyContinue
Write-Output 'countercheck-runner, its firewall rule and its settings are removed.'
"#;

fn powershell() -> PathBuf {
    std::env::var_os("SystemRoot")
        .map(|r| PathBuf::from(r).join(r"System32\WindowsPowerShell\v1.0\powershell.exe"))
        .filter(|p| p.is_file())
        .unwrap_or_else(|| PathBuf::from("powershell.exe"))
}

/// Base64 (standard alphabet, padded).
fn base64(bytes: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk.first().copied().unwrap_or(0),
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for (i, shift) in [18u32, 12, 6, 0].into_iter().enumerate() {
            if i <= chunk.len() {
                out.push(char::from(A[((n >> shift) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// PowerShell's `-EncodedCommand`: the script as UTF-16LE, in Base64. The whole script is parsed
/// at once (`-Command -` reads stdin line by line and mishandles multi-line blocks). It holds no
/// secrets: the password travels in an environment variable.
fn encoded(script: &str) -> String {
    let bytes: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    base64(&bytes)
}

/// Run a script; its exit code and everything it printed.
fn run_script(script: &str, env: &[(&str, &str)]) -> Result<(i32, String), String> {
    let out = Command::new(powershell())
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-EncodedCommand",
        ])
        .arg(encoded(script))
        .envs(env.iter().copied())
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| format!("could not start PowerShell: {e}"))?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    Ok((out.status.code().unwrap_or(1), text.trim().to_string()))
}

/// `countercheck windows-setup [--remove] [--allow-read DIR]...`.
pub fn setup_main(remove: bool, allow_read: &[PathBuf]) -> Result<u8, String> {
    let dir = config_dir().ok_or("ProgramData is not set")?;
    let dir = dir.to_string_lossy().into_owned();
    if remove {
        let (code, out) = run_script(
            REMOVE,
            &[("CC_ACCOUNT", ACCOUNT), ("CC_RULE", RULE), ("CC_DIR", &dir)],
        )?;
        println!("{out}");
        return Ok(u8::try_from(code.clamp(0, 255)).unwrap_or(1));
    }
    let mut reads = Vec::new();
    for p in allow_read {
        let real = std::fs::canonicalize(p).map_err(|e| format!("{}: {e}", p.display()))?;
        let s = crate::runner::display_path(&real);
        if s.contains('|') {
            return Err(format!("{s}: a path with | can't be granted"));
        }
        reads.push(s);
    }
    let password = new_password()?;
    let joined = reads.join("|");
    let (code, out) = run_script(
        SETUP,
        &[
            ("CC_ACCOUNT", ACCOUNT),
            ("CC_RULE", RULE),
            ("CC_DIR", &dir),
            ("CC_PASSWORD", &password),
            ("CC_ALLOW_READ", &joined),
        ],
    )?;
    println!("{out}");
    Ok(u8::try_from(code.clamp(0, 255)).unwrap_or(1))
}

/// The firewall rule exists, is enabled, blocks outbound traffic and applies to the account.
fn firewall_ok(c: &Config) -> bool {
    let script = r#"$r = Get-NetFirewallRule -Name $env:CC_RULE -ErrorAction SilentlyContinue
if (-not $r) { 'missing'; exit 0 }
$u = (Get-NetFirewallSecurityFilter -AssociatedNetFirewallRule $r).LocalUser
"$($r.Enabled) $($r.Direction) $($r.Action) $u""#;
    match run_script(script, &[("CC_RULE", &c.firewall_rule)]) {
        Ok((0, out)) => parse_firewall(&out, &c.sid),
        _ => false,
    }
}

/// `True Outbound Block D:(A;;CC;;;<sid>)` for this account's SID.
pub fn parse_firewall(out: &str, sid: &str) -> bool {
    let mut f = out.split_whitespace();
    f.next() == Some("True")
        && f.next() == Some("Outbound")
        && f.next() == Some("Block")
        && f.next()
            .is_some_and(|u| u.contains(&format!(";;;{sid})")) && u.starts_with("D:"))
}

/// The environment for a process started as the account: this one's (the check's PATH, TEMP,
/// TMPDIR, COUNTERCHECK_* and matrix variables), with the profile variables pointing at the
/// account's own profile so tools read and write their settings there.
pub fn runner_env(c: &Config, temp: Option<&Path>) -> Vec<(String, String)> {
    let profile = c.profile.trim_end_matches('\\').to_string();
    let drive = profile.get(..2).unwrap_or("C:").to_string();
    let home_path = profile.get(2..).unwrap_or("").to_string();
    let mut overrides: Vec<(String, String)> = vec![
        ("USERNAME".into(), c.user.clone()),
        ("USERPROFILE".into(), profile.clone()),
        ("HOMEDRIVE".into(), drive),
        ("HOMEPATH".into(), home_path),
        ("APPDATA".into(), format!(r"{profile}\AppData\Roaming")),
        ("LOCALAPPDATA".into(), format!(r"{profile}\AppData\Local")),
    ];
    if let Some(t) = temp {
        let t = t.to_string_lossy().into_owned();
        overrides.push(("TEMP".into(), t.clone()));
        overrides.push(("TMP".into(), t));
    }
    let mut out: Vec<(String, String)> = std::env::vars()
        .filter(|(k, _)| !overrides.iter().any(|(o, _)| o.eq_ignore_ascii_case(k)))
        .collect();
    out.extend(overrides);
    out
}

/// Start `cmdline` as the account (suspended), put it in a kill-on-close Job Object, run it up
/// to `timeout`: its exit code, or `None` on timeout. Everything it started is stopped either way.
pub fn run_as_runner(
    c: &Config,
    cmdline: &str,
    cwd: &Path,
    temp: Option<&Path>,
    timeout: Duration,
) -> Result<Option<u32>, String> {
    let password = password(c)?;
    let env = ffi::block::env_block(&runner_env(c, temp))?;
    let child = ffi::spawn_logon(&ffi::Logon {
        user: &c.user,
        password: &password,
        command_line: cmdline,
        cwd,
        env_block: &env,
    })
    .map_err(|e| format!("could not start the check as {}: {e}", c.user))?;
    let mut info = win32job::ExtendedLimitInfo::new();
    info.limit_kill_on_job_close();
    let job = match win32job::Job::create_with_limit_info(&info)
        .and_then(|j| j.assign_process(child.raw_handle()).map(|()| j))
    {
        Ok(j) => j,
        Err(e) => {
            let _ = child.kill();
            return Err(format!("Job Object: {e}"));
        }
    };
    if let Err(e) = child.resume() {
        let _ = child.kill();
        return Err(format!("could not start the check as {}: {e}", c.user));
    }
    let status = child.wait(timeout).map_err(|e| format!("wait: {e}"));
    drop(job);
    let _ = child.wait(Duration::from_secs(5));
    status
}

/// The copy of countercheck this `verify` made for the account (see [`helper_copy`]).
static HELPER: std::sync::Mutex<Option<PathBuf>> = std::sync::Mutex::new(None);

pub fn set_helper(p: PathBuf) {
    if let Ok(mut h) = HELPER.lock() {
        *h = Some(p);
    }
}

pub fn helper() -> Option<PathBuf> {
    HELPER.lock().ok().and_then(|h| h.clone())
}

/// A private copy of this binary the account can run but not change: `dir\countercheck.exe`.
pub fn helper_copy(c: &Config, dir: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    crate::windows_sandbox::grant_read(dir, &c.sid)?;
    let exe = std::env::current_exe().map_err(|e| format!("countercheck binary: {e}"))?;
    let to = dir.join("countercheck.exe");
    std::fs::copy(&exe, &to).map_err(|e| format!("{}: {e}", to.display()))?;
    Ok(to)
}

/// `countercheck sandbox-exec --probe-runner`: `runner=1` when the setup is complete here: the
/// account starts, a write-restricted process running as it is refused a write, and the firewall
/// rule is in place.
pub fn probe_main() -> u8 {
    let ok = probe_runner().unwrap_or_else(|e| {
        eprintln!("[countercheck] Windows runner probe: {e}");
        false
    });
    println!("runner={}", u8::from(ok));
    0
}

fn probe_runner() -> Result<bool, String> {
    let c = load().ok_or("countercheck windows-setup has not been run here")?;
    if !firewall_ok(&c) {
        return Err(format!(
            "the firewall rule {} is missing or changed",
            c.firewall_rule
        ));
    }
    let dir = tempfile::Builder::new()
        .prefix("countercheck-probe-")
        .tempdir()
        .map_err(|e| e.to_string())?;
    let exe = helper_copy(&c, &dir.path().join("bin"))?;
    let out = dir.path().join("out");
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    crate::windows_sandbox::grant_full(&out, &c.sid)?;
    // The account runs the restricted-write probe and leaves its answer in `out`.
    let line = format!(
        "\"{}\" sandbox-exec --probe-to \"{}\"",
        exe.display(),
        out.join("answer.txt").display()
    );
    run_as_runner(&c, &line, &out, Some(&out), Duration::from_secs(120))?;
    let answer = std::fs::read_to_string(out.join("answer.txt")).unwrap_or_default();
    Ok(crate::windows_sandbox::parse_probe(&answer))
}

/// Whether the full sandbox works here, probed once per process with the given binary.
pub fn probe(exe: &Path) -> bool {
    static OK: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *OK.get_or_init(|| {
        load().is_some()
            && Command::new(exe)
                .args(["sandbox-exec", "--probe-runner"])
                .stdin(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .output()
                .ok()
                .filter(|o| o.status.success())
                .is_some_and(|o| String::from_utf8_lossy(&o.stdout).contains("runner=1"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = r#"{"version":1,"user":"countercheck-runner","sid":"S-1-5-21-1-2-3-1001","profile":"C:\\Users\\countercheck-runner","firewall_rule":"countercheck-runner-no-internet","password":"0a0b","allow_read":[]}"#;

    #[test]
    fn config_is_parsed_strictly() {
        let c = parse_config(GOOD).unwrap();
        assert_eq!(c.user, ACCOUNT);
        assert!(
            parse_config(&format!("\u{feff}{GOOD}")).is_some(),
            "a BOM is fine"
        );
        for bad in [
            GOOD.replace("\"version\":1", "\"version\":2"),
            GOOD.replace("S-1-5-21-1-2-3-1001", "S-1-1-0"),
            GOOD.replace("countercheck-runner\",\"sid", "a b\",\"sid"),
            GOOD.replace("\"0a0b\"", "\"\""),
            GOOD.replace("C:\\\\Users", "Users")
                .replace("C:\\Users", "Users"),
            "not json".to_string(),
        ] {
            assert!(parse_config(&bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn firewall_answer_is_checked() {
        let sid = "S-1-5-21-1-2-3-1001";
        assert!(parse_firewall(
            &format!("True Outbound Block D:(A;;CC;;;{sid})"),
            sid
        ));
        for bad in [
            format!("False Outbound Block D:(A;;CC;;;{sid})"),
            format!("True Inbound Block D:(A;;CC;;;{sid})"),
            format!("True Outbound Allow D:(A;;CC;;;{sid})"),
            "True Outbound Block D:(A;;CC;;;S-1-5-21-9-9-9-1001)".to_string(),
            "missing".to_string(),
        ] {
            assert!(!parse_firewall(&bad, sid), "{bad}");
        }
    }

    #[test]
    fn scripts_are_encoded_for_powershell() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
        // `exit 3` as PowerShell expects it (UTF-16LE), checked against
        // [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes('exit 3')).
        assert_eq!(encoded("exit 3"), "ZQB4AGkAdAAgADMA");
    }

    #[test]
    fn passwords_are_random_and_complex() {
        let a = new_password().unwrap();
        assert_ne!(a, new_password().unwrap());
        assert_eq!(a.len(), 32);
        assert!(a.chars().any(|c| c.is_ascii_uppercase()));
        assert!(a.chars().any(|c| c.is_ascii_lowercase()));
        assert!(a.chars().any(|c| c.is_ascii_digit()));
        assert!(a.contains('!'));
        assert_eq!(unhex("0aff"), Some(vec![10, 255]));
        assert_eq!(unhex("0g"), None);
        assert_eq!(unhex("abc"), None);
    }

    #[test]
    fn the_runner_gets_its_own_profile_variables() {
        let c = parse_config(GOOD).unwrap();
        let env = runner_env(&c, Some(Path::new(r"C:\t\tmp")));
        let get = |k: &str| {
            env.iter()
                .filter(|(n, _)| n.eq_ignore_ascii_case(k))
                .map(|(_, v)| v.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            get("USERPROFILE"),
            vec![r"C:\Users\countercheck-runner".to_string()]
        );
        assert_eq!(
            get("HOMEPATH"),
            vec![r"\Users\countercheck-runner".to_string()]
        );
        assert_eq!(get("TEMP"), vec![r"C:\t\tmp".to_string()]);
        assert_eq!(get("USERNAME"), vec![ACCOUNT.to_string()]);
        assert!(ffi::block::env_block(&env).is_ok());
    }
}
