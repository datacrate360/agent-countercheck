//! A self-contained HTML report of saved receipts (`countercheck report`).
//!
//! Receipts carry text from the worker's tree (claims, check names, command output), so everything
//! is escaped, and the page has no script at all: a Content-Security-Policy forbids scripts and any
//! outside resource, so even an escaping bug could not run code or call home. Expanding a run uses
//! plain HTML `<details>`.
//!
//! The live page (`countercheck ui`, [`render_live`]) adds one small script that polls for new
//! receipts. Only a script carrying that response's random nonce may run, so text from a receipt
//! still can't.

use crate::receipt::{Receipt, Verdict};
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// One receipt file, read back.
pub struct Entry {
    /// File name, e.g. `1a2b3c4d5e6f7a8b.json`.
    pub file: String,
    pub receipt: Receipt,
    /// The seal still matches (`Receipt::seal_is_valid`).
    pub seal_ok: bool,
}

/// Most runs shown in one report (newest first).
pub const MAX_RUNS: usize = 200;

/// Escape text for HTML element content and attribute values.
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// Render the static report. `entries` may be in any order; the newest `MAX_RUNS` are shown.
/// `generated_at` and `title` are shown as given (escaped).
pub fn render_html(entries: &[Entry], title: &str, generated_at: &str) -> String {
    render(entries, title, generated_at, None)
}

/// Content-Security-Policy of the live page: only the script with this nonce, fetches only to the
/// page's own server. `nonce` must be letters and digits (the server makes it).
pub fn live_csp(nonce: &str) -> String {
    format!(
        "default-src 'none'; style-src 'unsafe-inline'; script-src 'nonce-{nonce}'; connect-src 'self'; \
         base-uri 'none'; form-action 'none'; frame-ancestors 'none'"
    )
}

/// Render the live page served by `countercheck ui`: the same report plus a script that polls
/// `/state` and reloads when a new receipt lands, keeping open runs open.
pub fn render_live(entries: &[Entry], title: &str, generated_at: &str, nonce: &str) -> String {
    let nonce: String = nonce.chars().filter(char::is_ascii_alphanumeric).collect();
    render(entries, title, generated_at, Some(&nonce))
}

fn render(entries: &[Entry], title: &str, generated_at: &str, live: Option<&str>) -> String {
    let mut runs: Vec<&Entry> = entries.iter().collect();
    runs.sort_by(|a, b| b.receipt.created_at.cmp(&a.receipt.created_at));
    runs.truncate(MAX_RUNS);

    let count = |v: Verdict| runs.iter().filter(|e| e.receipt.verdict == v).count();
    let broken = runs.iter().filter(|e| !e.seal_ok).count();

    let mut h = String::new();
    let csp = match live {
        // frame-ancestors only works as a header (the server sends it); browsers warn in a <meta>.
        Some(n) => live_csp(n).replace("; frame-ancestors 'none'", ""),
        None => "default-src 'none'; style-src 'unsafe-inline'".to_string(),
    };
    let _ = write!(
        h,
        // Our own fixed text; the nonce is letters and digits only.
        "{HEAD_START}<meta http-equiv=\"Content-Security-Policy\" content=\"{csp}\">"
    );
    let _ = write!(h, "<title>{}</title>", escape(title));
    let _ = write!(h, "<style>{STYLE}</style>");
    h.push_str("</head><body>");
    let _ = write!(
        h,
        "<nav class=\"top\"><span class=\"brand\"><span class=\"mark\"></span>Agent&nbsp;Counter<em>check</em></span>{}</nav><main>",
        if live.is_some() {
            "<span class=\"pill live\"><span class=\"dot\"></span><span id=\"live\">live</span></span>"
        } else {
            "<span class=\"pill\">offline report</span>"
        }
    );
    let _ = write!(
        h,
        "<header><p class=\"eyebrow\">Agent Countercheck · receipts</p><h1>{}</h1>\
         <p class=\"lead\">Every time an agent said <q>done</q>, re-checked on rules it couldn't change.</p>\
         <p class=\"meta\">Generated {} · newest first</p></header>",
        escape(title),
        escape(&pretty_time(generated_at)),
    );

    let _ = write!(
        h,
        "<section class=\"totals\">\
         <div><b>{}</b><span>runs</span></div>\
         <div class=\"t-verified\"><b>{}</b><span>verified</span></div>\
         <div class=\"t-failed\"><b>{}</b><span>failed</span></div>\
         <div class=\"t-unverified\"><b>{}</b><span>unverified</span></div>\
         <div class=\"{}\"><b>{}</b><span>broken seals</span></div></section>",
        runs.len(),
        count(Verdict::Verified),
        count(Verdict::Failed),
        count(Verdict::Unverified),
        if broken > 0 { "t-failed" } else { "" },
        broken
    );

    if runs.is_empty() {
        h.push_str("<p>No receipts yet. Run <code>countercheck verify</code> first.</p>");
    }
    if let Some(latest) = runs.first() {
        h.push_str(&matrix_grid(&latest.receipt));
    }

    h.push_str(
        "<section><p class=\"eyebrow\">Runs</p><h2>What each agent claimed, and what ran</h2>",
    );
    for (i, e) in runs.iter().enumerate() {
        h.push_str(&run_html(e, i == 0));
    }
    h.push_str("</section>");
    h.push_str(
        "<footer class=\"muted\"><b>Agent Countercheck</b> · the agent never grades itself. Each receipt is sealed with SHA-256; a broken seal means the file was \
         edited after it was written. Check one with <code>countercheck show &lt;file&gt;</code>.</footer>",
    );
    h.push_str("</main>");
    if let Some(n) = live {
        let _ = write!(h, "<script nonce=\"{n}\">{LIVE_JS}</script>");
    }
    h.push_str("</body></html>\n");
    h
}

/// Poll `/state` every 2 s (same token as the page); reload when it changes, remembering which runs
/// were open. Uses only the page's own server.
const LIVE_JS: &str = include_str!("../assets/live.js");

/// `reconcile [schema=CA1]` → (`reconcile`, `schema`, `CA1`).
fn matrix_parts(name: &str) -> Option<(&str, &str, &str)> {
    let (base, rest) = name.rsplit_once(" [")?;
    let inner = rest.strip_suffix(']')?;
    let (key, value) = inner.split_once('=')?;
    Some((base, key, value))
}

/// One matrix key's grid: check names in order, then value -> check -> passed.
type Grid<'a> = (Vec<&'a str>, BTreeMap<&'a str, BTreeMap<&'a str, bool>>);

/// For the latest run: one grid per matrix key (targets down, checks across).
fn matrix_grid(r: &Receipt) -> String {
    let mut grids: BTreeMap<&str, Grid> = BTreeMap::new();
    for c in &r.checks {
        let Some((base, key, value)) = matrix_parts(&c.name) else {
            continue;
        };
        let g = grids.entry(key).or_default();
        if !g.0.contains(&base) {
            g.0.push(base);
        }
        g.1.entry(value).or_default().insert(base, c.passed);
    }
    let mut h = String::new();
    for (key, (cols, rows)) in &grids {
        let failed_rows = rows
            .values()
            .filter(|cells| cells.values().any(|p| !p))
            .count();
        let _ = write!(
            h,
            "<section><h2>Latest run by {k}</h2><p class=\"muted\">{n} {k} value(s), {f} with a failure</p>\
             <div class=\"scroll\"><table class=\"grid\"><tr><th>{k}</th>",
            k = escape(key),
            n = rows.len(),
            f = failed_rows
        );
        for c in cols {
            let _ = write!(h, "<th>{}</th>", escape(c));
        }
        h.push_str("</tr>");
        for (value, cells) in rows {
            let _ = write!(h, "<tr><td>{}</td>", escape(value));
            for c in cols {
                h.push_str(match cells.get(c) {
                    Some(true) => "<td class=\"ok\">pass</td>",
                    Some(false) => "<td class=\"bad\">FAIL</td>",
                    None => "<td class=\"muted\">–</td>",
                });
            }
            h.push_str("</tr>");
        }
        h.push_str("</table></div></section>");
    }
    h
}

/// `2026-10-19T15:55:23Z` → `2026-10-19 15:55:23 UTC`; anything else is shown as it is.
fn pretty_time(t: &str) -> String {
    match t.strip_suffix('Z') {
        Some(rest) if rest.len() == 19 && rest.as_bytes().get(10) == Some(&b'T') => {
            format!("{} {} UTC", &rest[..10], &rest[11..])
        }
        _ => t.to_string(),
    }
}

fn verdict_class(v: Verdict) -> &'static str {
    match v {
        Verdict::Verified => "verified",
        Verdict::Failed => "failed",
        Verdict::Unverified => "unverified",
    }
}

fn short(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(n).collect();
        t.push('…');
        t
    }
}

fn run_html(e: &Entry, open: bool) -> String {
    let r = &e.receipt;
    let passed = r.checks.iter().filter(|c| c.passed).count();
    let who = match (&r.worker.agent, &r.worker.model) {
        (Some(a), Some(m)) => format!("{a} · {m}"),
        (Some(a), None) => a.clone(),
        (None, Some(m)) => m.clone(),
        (None, None) => "unknown worker".to_string(),
    };
    let claim = r.claim.as_deref().unwrap_or("");
    let mut h = String::new();
    let _ = write!(
        h,
        "<details class=\"run\" id=\"r-{id}\"{open}><summary>\
         <span class=\"badge {cls}\">{label}</span>\
         <span class=\"when\">{when}</span>\
         <span class=\"who\">{who}</span>\
         <span class=\"count\">{passed}/{total} checks</span>{seal}\
         <span class=\"claim\">{claim}</span></summary>",
        id = escape(&r.id),
        open = if open { " open" } else { "" },
        cls = verdict_class(r.verdict),
        label = r.verdict.label(),
        when = escape(&pretty_time(&r.created_at)),
        who = escape(&who),
        total = r.checks.len(),
        seal = if e.seal_ok {
            "<span class=\"seal ok\">seal intact</span>"
        } else {
            "<span class=\"seal bad\">SEAL BROKEN</span>"
        },
        claim = escape(&short(claim, 140)),
    );

    h.push_str("<div class=\"body\">");
    if !r.reasons.is_empty() {
        h.push_str("<ul class=\"reasons\">");
        for reason in &r.reasons {
            let _ = write!(h, "<li>{}</li>", escape(reason));
        }
        h.push_str("</ul>");
    }
    if !claim.is_empty() {
        let _ = write!(
            h,
            "<p><b>The worker said:</b></p><blockquote>{}</blockquote>",
            escape(claim)
        );
    }

    h.push_str(
        "<table class=\"checks\"><tr><th></th><th>check</th><th>time</th><th>command</th></tr>",
    );
    for c in &r.checks {
        let net = if c.network { " · internet" } else { "" };
        let writes = if c.writes.is_empty() {
            String::new()
        } else {
            format!(" · writes {}", escape(&c.writes.join(", ")))
        };
        let net = format!("{net}{writes}");
        let mark = if c.passed {
            "<td class=\"ok\">pass</td>"
        } else {
            "<td class=\"bad\">FAIL</td>"
        };
        let _ = write!(
            h,
            "<tr>{mark}<td>{}</td><td class=\"nowrap\">{} ms{net}</td><td><code>{}</code></td></tr>",
            escape(&c.name),
            c.duration_ms,
            escape(&c.run)
        );
        if !c.passed && !c.output_tail.is_empty() {
            let _ = write!(
                h,
                "<tr><td></td><td colspan=\"3\"><pre>{}</pre></td></tr>",
                escape(&c.output_tail)
            );
        }
    }
    h.push_str("</table>");

    if !r.protected_changes.is_empty() {
        h.push_str("<p><b>Protected files the worker changed</b> (the checks used the base version):</p><ul>");
        for p in &r.protected_changes {
            let change = serde_json::to_value(p.change)
                .ok()
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_default();
            let _ = write!(
                h,
                "<li><code>{}</code> {} → {}</li>",
                escape(&p.path),
                escape(&change),
                escape(&p.action)
            );
        }
        h.push_str("</ul>");
    }

    if let Some(w) = r.watched.as_ref().filter(|w| w.total > 0) {
        let _ = write!(
            h,
            "<p><b>Watched files changed</b> ({}; {} existing modified or deleted; review them):</p><ul>",
            w.total, w.existing_changed
        );
        for c in &w.changes {
            let _ = write!(h, "<li><code>{}</code></li>", escape(&c.describe()));
        }
        let more = w.total.saturating_sub(w.changes.len());
        if more > 0 {
            let _ = write!(h, "<li>and {more} more</li>");
        }
        h.push_str("</ul>");
    }

    if let Some(u) = r.uncovered.as_ref().filter(|u| u.total > 0) {
        let _ = write!(
            h,
            "<p><b>Changed files no check covers</b> ({}; listed, not checked):</p><ul>",
            u.total
        );
        for p in &u.paths {
            let _ = write!(h, "<li><code>{}</code></li>", escape(p));
        }
        let more = u.total.saturating_sub(u.paths.len());
        if more > 0 {
            let _ = write!(h, "<li>and {more} more</li>");
        }
        h.push_str("</ul>");
    }

    let _ = write!(
        h,
        "<dl class=\"facts\">\
         <dt>receipt</dt><dd><code>{file}</code> · id {id}</dd>\
         <dt>checks</dt><dd>{src}{locked}</dd>\
         <dt>base</dt><dd><code>{base_rev}</code> = {base}</dd>\
         <dt>head</dt><dd>{head}{dirty}</dd>\
         <dt>sandbox</dt><dd>{sandbox}</dd>\
         <dt>files</dt><dd>tree sha256 {tree}</dd></dl>",
        file = escape(&e.file),
        id = escape(&r.id),
        src = escape(&r.checks_source),
        locked = if r.checks_locked {
            " (locked)"
        } else {
            " (<b>not locked</b>: not in the base revision)"
        },
        base_rev = escape(&r.repo.base_rev),
        base = escape(&short(&r.repo.base_commit, 12)),
        head = escape(&short(&r.repo.head_commit, 12)),
        dirty = if r.repo.dirty {
            " + uncommitted changes"
        } else {
            ""
        },
        sandbox = escape(&r.sandbox),
        tree = escape(&short(&r.tree_sha256, 16)),
    );
    h.push_str("</div></details>");
    h
}

const HEAD_START: &str = "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
<meta name=\"referrer\" content=\"no-referrer\">";

/// Styles for the report and the live UI: `assets/report.css`, compiled into the binary.
const STYLE: &str = include_str!("../assets/report.css");

#[cfg(test)]
mod tests {
    use super::*;
    use crate::receipt::{CheckResult, ClaimStatus, RepoInfo, Worker};

    fn receipt(created: &str, verdict: Verdict, checks: Vec<CheckResult>) -> Receipt {
        Receipt {
            schema: crate::receipt::SCHEMA.to_string(),
            id: "abc".to_string(),
            receipt_sha256: String::new(),
            created_at: created.to_string(),
            countercheck_version: "0".to_string(),
            repo: RepoInfo {
                base_rev: "HEAD".to_string(),
                base_commit: "0123456789abcdef".to_string(),
                head_commit: "0123456789abcdef".to_string(),
                dirty: true,
            },
            worker: Worker {
                agent: Some("claude-code".to_string()),
                model: None,
                cost_usd: None,
            },
            claim: Some("Done, all tests pass <script>alert(1)</script>".to_string()),
            claim_status: ClaimStatus::NoClaim,
            checks_locked: true,
            checks_source: ".countercheck/checks.toml @ HEAD".to_string(),
            sandbox: "none".to_string(),
            tree_sha256: "f".repeat(64),
            linked: vec![],
            protected_changes: vec![],
            uncovered: None,
            watched: None,
            checks,
            verdict,
            reasons: vec!["check \"t\" failed".to_string()],
        }
    }

    fn check(name: &str, passed: bool, out: &str) -> CheckResult {
        CheckResult {
            name: name.to_string(),
            run: "sh t.sh".to_string(),
            exit_code: Some(if passed { 0 } else { 1 }),
            timed_out: false,
            duration_ms: 5,
            passed,
            output_tail: out.to_string(),
            output_sha256: String::new(),
            network: false,
            writes: Vec::new(),
        }
    }

    #[test]
    fn escape_covers_every_special_character() {
        assert_eq!(
            escape("<a href=\"x\" onclick='y'>&</a>"),
            "&lt;a href=&quot;x&quot; onclick=&#39;y&#39;&gt;&amp;&lt;/a&gt;"
        );
    }

    #[test]
    fn hostile_text_is_escaped_and_the_page_has_no_script() {
        let r = receipt(
            "2026-10-19T10:00:00Z",
            Verdict::Failed,
            vec![check(
                "<img src=x onerror=alert(1)>",
                false,
                "</pre><script>steal()</script>",
            )],
        );
        let html = render_html(
            &[Entry {
                file: "a.json".to_string(),
                receipt: r,
                seal_ok: true,
            }],
            "repo <b>",
            "now",
        );
        assert!(!html.to_lowercase().contains("<script"), "{html}");
        assert!(!html.contains("<img"));
        assert!(html.contains("&lt;/pre&gt;&lt;script&gt;steal()"));
        assert!(html.contains("default-src 'none'"));
        assert!(!html.contains("http://") && !html.contains("https://"));
    }

    #[test]
    fn newest_first_counts_and_broken_seal() {
        let entries = vec![
            Entry {
                file: "old.json".to_string(),
                receipt: receipt("2026-10-18T10:00:00Z", Verdict::Verified, vec![]),
                seal_ok: true,
            },
            Entry {
                file: "new.json".to_string(),
                receipt: receipt("2026-10-19T10:00:00Z", Verdict::Failed, vec![]),
                seal_ok: false,
            },
        ];
        let html = render_html(&entries, "r", "now");
        let new = html.find("new.json").unwrap();
        let old = html.find("old.json").unwrap();
        assert!(new < old, "newest first");
        assert!(html.contains("SEAL BROKEN"));
        assert!(html.contains("<b>2</b><span>runs</span>"));
        assert!(html.contains("<b>1</b><span>broken seals</span>"));
    }

    #[test]
    fn matrix_checks_become_a_grid() {
        let r = receipt(
            "2026-10-19T10:00:00Z",
            Verdict::Failed,
            vec![
                check("reconcile [schema=CA1]", true, ""),
                check("reconcile [schema=MN3]", false, "rows differ"),
                check("golden [schema=CA1]", true, ""),
                check("plain", true, ""),
            ],
        );
        let html = render_html(
            &[Entry {
                file: "a.json".to_string(),
                receipt: r,
                seal_ok: true,
            }],
            "r",
            "now",
        );
        assert!(html.contains("Latest run by schema"));
        assert!(html.contains("2 schema value(s), 1 with a failure"));
        assert!(html.contains("<th>reconcile</th><th>golden</th>"));
        assert_eq!(matrix_parts("plain"), None);
        assert_eq!(matrix_parts("a [k=v]"), Some(("a", "k", "v")));
    }

    #[test]
    fn times_are_readable() {
        assert_eq!(
            pretty_time("2026-10-19T15:55:23Z"),
            "2026-10-19 15:55:23 UTC"
        );
        assert_eq!(pretty_time("yesterday"), "yesterday");
        assert_eq!(pretty_time("é€TZ"), "é€TZ");
    }

    #[test]
    fn live_page_runs_only_its_own_nonced_script() {
        let r = receipt(
            "2026-10-19T10:00:00Z",
            Verdict::Failed,
            vec![check("t", false, "<script>steal()</script>")],
        );
        let e = [Entry {
            file: "a.json".to_string(),
            receipt: r,
            seal_ok: true,
        }];
        let html = render_live(&e, "r", "now", "abc123\"><x");
        assert_eq!(html.matches("<script").count(), 1, "{html}");
        assert!(html.contains("<script nonce=\"abc123x\">"));
        assert!(html.contains("script-src 'nonce-abc123x'"));
        assert!(html.contains("connect-src 'self'"));
        assert!(html.contains("id=\"live\""));
        assert!(!render_html(&e, "r", "now").contains("<script"));
    }

    #[test]
    fn assets_are_embedded_and_safe() {
        assert!(STYLE.contains("--teal"));
        assert!(LIVE_JS.contains("/state?t="));
        assert!(!LIVE_JS.to_lowercase().contains("</script"));
        assert!(!STYLE.to_lowercase().contains("</style"));
        for s in [STYLE, LIVE_JS] {
            assert!(
                !s.contains("http://") && !s.contains("https://"),
                "no outside resources"
            );
        }
        let html = render_live(&[], "r", "now", "n1");
        assert!(html.contains("location.reload()"));
        assert!(html.contains("<style>:root") || html.contains("<style>/*"));
    }

    #[test]
    fn empty_report_says_how_to_start() {
        let html = render_html(&[], "r", "now");
        assert!(html.contains("No receipts yet"));
    }
}
