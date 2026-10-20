//! End-to-end tests for `countercheck ui`: the real binary, a real local port, raw HTTP requests.

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::{Child, Command, Stdio};

mod common;

const BIN: &str = env!("CARGO_BIN_EXE_countercheck");

fn sh(dir: &Path, cmd: &str) {
    let ok = Command::new(common::sh_program())
        .args(["-c", cmd])
        .current_dir(dir)
        .status()
        .unwrap()
        .success();
    assert!(ok, "{cmd}");
}

/// A repo with one failing check and one receipt.
fn project_with_receipt() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    sh(p, common::GIT_INIT);
    fs::create_dir_all(p.join(".countercheck")).unwrap();
    fs::write(
        p.join(".countercheck/checks.toml"),
        "[[check]]\nname = \"t\"\nrun = \"echo '<script>x()</script>'; exit 1\"\n",
    )
    .unwrap();
    sh(p, "git add -A && git commit -qm base");
    verify(p);
    d
}

fn verify(p: &Path) {
    let st = Command::new(BIN)
        .args(["verify", "--sandbox", "none", "--repo"])
        .arg(p)
        .stdout(Stdio::null())
        .status()
        .unwrap();
    assert_eq!(st.code(), Some(2));
}

/// Kills the server when the test ends, even on failure.
struct Server {
    child: Child,
    port: u16,
    token: String,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn start(p: &Path, extra: &[&str]) -> Server {
    let mut child = Command::new(BIN)
        .args(["ui", "--no-open", "--repo"])
        .arg(p)
        .args(extra)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let url = line
        .trim()
        .strip_prefix("Countercheck UI: ")
        .unwrap_or_else(|| panic!("{line}"))
        .to_string();
    let rest = url.strip_prefix("http://127.0.0.1:").unwrap();
    let (port, token) = rest.split_once("/?t=").unwrap();
    Server {
        child,
        port: port.parse().unwrap(),
        token: token.to_string(),
    }
}

/// Raw request; returns (status code, headers, body).
fn request(port: u16, method: &str, path: &str, host: &str) -> (u16, String, String) {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    write!(
        s,
        "{method} {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut resp = String::new();
    s.read_to_string(&mut resp).unwrap();
    let (head, body) = resp.split_once("\r\n\r\n").unwrap();
    let code = head.split(' ').nth(1).unwrap().parse().unwrap();
    (code, head.to_string(), body.to_string())
}

#[test]
fn ui_serves_the_live_page_only_with_the_token_and_a_local_host() {
    let d = project_with_receipt();
    let s = start(d.path(), &["--port", "0"]);
    let host = format!("127.0.0.1:{}", s.port);

    let (code, head, body) = request(s.port, "GET", &format!("/?t={}", s.token), &host);
    assert_eq!(code, 200, "{head}");
    assert!(body.contains("FAILED"));
    // The check's output is shown escaped; the only script is the page's own, with this nonce.
    assert!(body.contains("&lt;script&gt;x()&lt;/script&gt;"));
    assert_eq!(body.matches("<script").count(), 1);
    let nonce = body
        .split("<script nonce=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    assert!(
        head.contains(&format!("script-src 'nonce-{nonce}'")),
        "{head}"
    );
    assert!(head.contains("Cache-Control: no-store"));
    assert!(head.contains("X-Frame-Options: DENY"));

    // No token, a wrong token, another host name (DNS rebinding), or not GET: refused.
    assert_eq!(request(s.port, "GET", "/", &host).0, 404);
    assert_eq!(request(s.port, "GET", "/?t=00", &host).0, 404);
    let evil = format!("evil.example:{}", s.port);
    assert_eq!(
        request(s.port, "GET", &format!("/?t={}", s.token), &evil).0,
        404
    );
    assert_eq!(
        request(s.port, "POST", &format!("/?t={}", s.token), &host).0,
        405
    );
    let local = format!("localhost:{}", s.port);
    assert_eq!(
        request(s.port, "GET", &format!("/?t={}", s.token), &local).0,
        200
    );
}

#[test]
fn ui_state_changes_when_a_new_receipt_lands() {
    let d = project_with_receipt();
    let s = start(d.path(), &["--port", "0"]);
    let host = format!("127.0.0.1:{}", s.port);
    let path = format!("/state?t={}", s.token);
    let (_, _, before) = request(s.port, "GET", &path, &host);
    assert_eq!(request(s.port, "GET", &path, &host).2, before);
    verify(d.path());
    let (code, _, after) = request(s.port, "GET", &path, &host);
    assert_eq!(code, 200);
    assert_ne!(after, before);
}

/// Browsers open spare connections before they need them and may send nothing on them for a
/// while. Such a connection must neither hold up other requests nor be answered: a stale reply
/// sitting on it would be read by the browser's next request (the live page then showed
/// "disconnected" after every reload).
#[test]
fn ui_idle_connections_neither_block_requests_nor_get_a_reply() {
    let d = project_with_receipt();
    let s = start(d.path(), &["--port", "0"]);
    let host = format!("127.0.0.1:{}", s.port);
    let path = format!("/state?t={}", s.token);
    let idle = TcpStream::connect(("127.0.0.1", s.port)).unwrap(); // a preconnect: sends nothing
    let started = std::time::Instant::now();
    let (code, _, _) = request(s.port, "GET", &path, &host);
    assert_eq!(code, 200);
    assert!(
        started.elapsed() < std::time::Duration::from_millis(1500),
        "a request waited {:?} behind an idle connection",
        started.elapsed()
    );
    // after the server gives up on the idle connection, it closes it without writing anything
    idle.set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .unwrap();
    let mut got = Vec::new();
    let _ = (&idle).read_to_end(&mut got);
    assert!(
        got.is_empty(),
        "idle connection got: {}",
        String::from_utf8_lossy(&got)
    );
}

#[test]
fn a_second_ui_finds_the_running_one() {
    let d = project_with_receipt();
    let s = start(d.path(), &["--port", "0"]);
    let out = Command::new(BIN)
        .args(["ui", "--no-open", "--repo"])
        .arg(d.path())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("already running"), "{text}");
    assert!(
        text.contains(&format!("127.0.0.1:{}/?t={}", s.port, s.token)),
        "{text}"
    );
    // The lock file sits with the receipts, so git ignores it.
    let st = Command::new("git")
        .arg("-C")
        .arg(d.path())
        .args(["status", "--porcelain"])
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&st.stdout).trim().is_empty());
}

#[test]
fn ui_reports_a_taken_port_plainly() {
    let d = project_with_receipt();
    let held = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = held.local_addr().unwrap().port().to_string();
    let out = Command::new(BIN)
        .args(["ui", "--no-open", "--port", &port, "--repo"])
        .arg(d.path())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("is not free"));
}
