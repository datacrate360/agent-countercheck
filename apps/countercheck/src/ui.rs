//! `countercheck ui`: a live page of this project's receipts, served on this computer only.
//!
//! Safety, because a local web server is reachable by every web page your browser opens:
//! - it listens on 127.0.0.1 only, never on the network;
//! - every request needs the random token from the printed address (missing or wrong: 404);
//! - the `Host` header must be `127.0.0.1` or `localhost` on our port, which stops DNS-rebinding
//!   pages from reading it through a name they control;
//! - GET only, read-only, one request at a time with short timeouts, small request size limit;
//! - responses are `no-store` with a strict Content-Security-Policy (only the page's own nonced
//!   script runs; it may only talk to this server).
//!
//! Ports: `--port N` uses exactly N. Otherwise it tries 7357 (rarely used by dev tools) and, if
//! that is taken, lets the system pick any free port. One UI per receipt folder: a second
//! `countercheck ui` finds the running one (`ui.json` next to the receipts) and opens it instead.

use crate::report;
use countercheck_core::report::{live_csp, render_live};
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Tried first when no `--port` is given.
pub const DEFAULT_PORT: u16 = 7357;
/// Longest request we read (request line + headers).
const MAX_REQUEST: usize = 8 * 1024;
const IO_TIMEOUT: Duration = Duration::from_secs(2);
/// Most connections served at once; more are closed straight away. Browsers open a few spare
/// connections per page, so this is plenty for one person's browser tabs.
const MAX_CONNECTIONS: usize = 32;

pub struct Options {
    pub repo: PathBuf,
    pub receipts: Option<PathBuf>,
    /// Exact port (0 = any free port). `None`: 7357, else any free port.
    pub port: Option<u16>,
    pub open: bool,
}

pub fn run(opts: &Options) -> Result<(), String> {
    let report::Located {
        dir,
        title,
        path,
        root,
    } = report::locate(&opts.repo, opts.receipts.as_deref())?;
    if !dir.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        if opts.receipts.is_none() {
            // Same rule as `verify`: the receipt folder never shows up as a change in git.
            let _ = std::fs::write(dir.join(".gitignore"), "*\n");
        }
    }
    let lock = dir.join("ui.json");
    if let Some(url) = already_running(&lock, &dir) {
        let _ = writeln!(
            std::io::stdout(),
            "Countercheck UI is already running for these receipts: {url}"
        );
        if opts.open {
            report::open_in_browser(std::ffi::OsStr::new(&url));
        }
        return Ok(());
    }

    let listener = bind(opts.port)?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("local address: {e}"))?
        .port();
    let token = random_hex(16)?;
    let url = format!("http://127.0.0.1:{port}/?t={token}");
    let record = serde_json::json!({ "port": port, "token": token, "receipts": dir });
    std::fs::write(&lock, record.to_string()).map_err(|e| format!("{}: {e}", lock.display()))?;

    // Best effort: if nobody reads our output any more, keep serving.
    let mut out = std::io::stdout();
    let _ = writeln!(out, "Countercheck UI: {url}");
    let _ = writeln!(
        out,
        "Only this computer can open it. It updates when a new receipt lands. Ctrl+C to stop."
    );
    let _ = out.flush();
    if opts.open {
        report::open_in_browser(std::ffi::OsStr::new(&url));
    }
    // One thread per connection: a browser's spare (pre-opened) connection that sends nothing
    // must not hold up the page's own requests while we wait for it.
    let shared = std::sync::Arc::new((token, dir, title, path, root));
    let active = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        if active.fetch_add(1, std::sync::atomic::Ordering::SeqCst) >= MAX_CONNECTIONS {
            active.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
            continue; // dropped: the connection closes
        }
        let (shared, done) = (shared.clone(), active.clone());
        let spawned = std::thread::Builder::new().spawn(move || {
            let (token, dir, title, path, root) = &*shared;
            // One bad request must not stop the server.
            let _ = serve(stream, port, token, dir, title, path, root.as_deref());
            done.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        });
        if spawned.is_err() {
            active.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        }
    }
    Ok(())
}

fn bind(port: Option<u16>) -> Result<TcpListener, String> {
    let at = |p: u16| SocketAddr::from((Ipv4Addr::LOCALHOST, p));
    match port {
        Some(p) => TcpListener::bind(at(p)).map_err(|e| {
            format!("port {p} on 127.0.0.1 is not free ({e}); pick another with --port, or leave --port out")
        }),
        None => TcpListener::bind(at(DEFAULT_PORT))
            .or_else(|_| TcpListener::bind(at(0)))
            .map_err(|e| format!("could not open a local port: {e}")),
    }
}

fn random_hex(bytes: usize) -> Result<String, String> {
    let mut buf = vec![0u8; bytes];
    getrandom::fill(&mut buf).map_err(|e| format!("random token: {e}"))?;
    Ok(buf.iter().map(|b| format!("{b:02x}")).collect())
}

/// The address of a UI already serving `dir`, if `ui.json` points at one that answers.
fn already_running(lock: &Path, dir: &Path) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(lock).ok()?).ok()?;
    let port = u16::try_from(v.get("port")?.as_u64()?).ok()?;
    let token = v.get("token")?.as_str()?;
    if token.is_empty() || !token.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let mut s = TcpStream::connect_timeout(
        &SocketAddr::from((Ipv4Addr::LOCALHOST, port)),
        Duration::from_millis(500),
    )
    .ok()?;
    s.set_read_timeout(Some(IO_TIMEOUT)).ok()?;
    let req = format!(
        "GET /health?t={token} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
    );
    s.write_all(req.as_bytes()).ok()?;
    let mut resp = String::new();
    let _ = s.take(64 * 1024).read_to_string(&mut resp);
    let expected = format!("countercheck-ui\n{}", dir.display());
    (resp.starts_with("HTTP/1.1 200") && resp.ends_with(&expected))
        .then(|| format!("http://127.0.0.1:{port}/?t={token}"))
}

/// What the request line and headers say, if they are acceptable.
struct Request {
    method: String,
    path: String,
    token: Option<String>,
    host: Option<String>,
}

/// Why no request could be read.
enum NoRequest {
    /// The connection closed or went quiet before sending a byte (a browser's spare connection).
    /// Close it without a reply: a reply left on it would be read by the browser's next request.
    Idle,
    /// Something arrived, but not a request we accept.
    Bad,
}

fn read_request(stream: &mut TcpStream) -> Result<Request, NoRequest> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
        let n = match stream.read(&mut chunk) {
            Ok(n) => n,
            Err(_) if buf.is_empty() => return Err(NoRequest::Idle),
            Err(_) => return Err(NoRequest::Bad),
        };
        if n == 0 {
            return Err(if buf.is_empty() {
                NoRequest::Idle
            } else {
                NoRequest::Bad
            });
        }
        if buf.len() + n > MAX_REQUEST {
            return Err(NoRequest::Bad);
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    parse_request(buf).ok_or(NoRequest::Bad)
}

fn parse_request(buf: Vec<u8>) -> Option<Request> {
    let text = String::from_utf8(buf).ok()?;
    let mut lines = text.split("\r\n");
    let mut first = lines.next()?.split(' ');
    let method = first.next()?.to_string();
    let target = first.next()?;
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let token = query
        .split('&')
        .find_map(|kv| kv.strip_prefix("t="))
        .map(str::to_string);
    let host = lines
        .filter_map(|l| l.split_once(':'))
        .find(|(k, _)| k.trim().eq_ignore_ascii_case("host"))
        .map(|(_, v)| v.trim().to_string());
    Some(Request {
        method,
        path: path.to_string(),
        token,
        host,
    })
}

/// Same length and every byte equal, without stopping at the first difference.
fn same(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

fn serve(
    mut stream: TcpStream,
    port: u16,
    token: &str,
    dir: &Path,
    title: &str,
    path: &str,
    root: Option<&Path>,
) -> std::io::Result<()> {
    stream.set_read_timeout(Some(IO_TIMEOUT))?;
    stream.set_write_timeout(Some(IO_TIMEOUT))?;
    let req = match read_request(&mut stream) {
        Ok(req) => req,
        Err(NoRequest::Idle) => return Ok(()),
        Err(NoRequest::Bad) => {
            return respond(&mut stream, 400, "text/plain", "bad request\n", None);
        }
    };
    if req.method != "GET" {
        return respond(&mut stream, 405, "text/plain", "only GET\n", None);
    }
    let host_ok = req
        .host
        .as_deref()
        .is_some_and(|h| h == format!("127.0.0.1:{port}") || h == format!("localhost:{port}"));
    let token_ok = req.token.as_deref().is_some_and(|t| same(t, token));
    if !host_ok || !token_ok {
        return respond(&mut stream, 404, "text/plain", "not found\n", None);
    }
    match req.path.as_str() {
        "/" => {
            let entries = report::read_receipts(dir).unwrap_or_default();
            let nonce = random_hex(16).map_err(std::io::Error::other)?;
            let now = humantime::format_rfc3339_seconds(std::time::SystemTime::now()).to_string();
            let running = crate::progress::read_running(dir);
            let hooks_off = report::hooks_off(root);
            let page = countercheck_core::report::Page {
                title,
                path,
                generated_at: &now,
                running: &running,
                hooks_off: &hooks_off,
            };
            let html = render_live(&entries, &page, &nonce);
            respond(
                &mut stream,
                200,
                "text/html; charset=utf-8",
                &html,
                Some(&live_csp(&nonce)),
            )
        }
        "/state" => respond(&mut stream, 200, "text/plain", &state(dir), None),
        "/health" => {
            let body = format!("countercheck-ui\n{}", dir.display());
            respond(&mut stream, 200, "text/plain", &body, None)
        }
        _ => respond(&mut stream, 404, "text/plain", "not found\n", None),
    }
}

/// Changes whenever a receipt is added, removed or rewritten, or a run in progress moves on
/// (count, newest change, total size, over both folders).
fn state(dir: &Path) -> String {
    let (mut n, mut newest, mut size) = (0u64, 0u128, 0u64);
    for dir in [
        dir.to_path_buf(),
        dir.join(countercheck_core::progress::DIR),
    ] {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in rd.flatten() {
            if e.path().extension().is_none_or(|x| x != "json") {
                continue;
            }
            let Ok(m) = e.metadata() else { continue };
            n += 1;
            size = size.wrapping_add(m.len());
            let t = m
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_nanos());
            newest = newest.max(t);
        }
    }
    format!("{n}-{newest}-{size}")
}

fn respond(
    stream: &mut TcpStream,
    code: u16,
    content_type: &str,
    body: &str,
    csp: Option<&str>,
) -> std::io::Result<()> {
    let reason = match code {
        200 => "OK",
        400 => "Bad Request",
        405 => "Method Not Allowed",
        _ => "Not Found",
    };
    let csp = csp.unwrap_or("default-src 'none'");
    let head = format!(
        "HTTP/1.1 {code} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\
         Cache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\n\
         X-Frame-Options: DENY\r\nCross-Origin-Resource-Policy: same-origin\r\n\
         Content-Security-Policy: {csp}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(body.as_bytes())?;
    stream.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_compare_exactly() {
        assert!(same("abc", "abc"));
        assert!(!same("abc", "abd"));
        assert!(!same("abc", "ab"));
        assert!(!same("", "a"));
    }

    #[test]
    fn random_tokens_differ() {
        let a = random_hex(16).unwrap();
        assert_eq!(a.len(), 32);
        assert_ne!(a, random_hex(16).unwrap());
    }

    #[test]
    fn an_exact_port_that_is_taken_is_a_plain_error() {
        let held = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).unwrap();
        let p = held.local_addr().unwrap().port();
        let e = bind(Some(p)).unwrap_err();
        assert!(e.contains("is not free"), "{e}");
    }
}
