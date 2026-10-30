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

mod flow;

use crate::receipt::{Receipt, Verdict};
use std::collections::{BTreeMap, BTreeSet};
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

/// The name of runs that say nothing about their agent (no `--agent`, no role, no marker).
pub const UNNAMED: &str = "unnamed agent";

/// Most agents given their own row and filter; runs of the rest stay in the list.
pub const MAX_AGENTS: usize = 24;

/// Longest claim shown as a run's title; a longer one is cut there and shown in full inside.
const CLAIM_TITLE_CHARS: usize = 140;

/// What the page is about: the project's name and where it is (shown as given, escaped).
pub struct Page<'a> {
    pub title: &'a str,
    /// The project folder, e.g. `~/code/my-app` (the caller shortens the home folder).
    pub path: &'a str,
    pub generated_at: &'a str,
    /// Runs in progress (display only; see [`crate::progress`]).
    pub running: &'a [crate::progress::Progress],
    /// Agent hook settings in the project that switch hooks off now: no receipt gets written, so
    /// the page says so above everything else.
    pub hooks_off: &'a [crate::hook_settings::HookSettingsChange],
    /// Sessions that started here and never got a receipt (display only; see [`crate::sessions`]).
    pub not_checked: &'a [crate::sessions::SessionStart],
}

/// Render the static report. `entries` may be in any order; the newest `MAX_RUNS` are shown.
pub fn render_html(entries: &[Entry], page: &Page) -> String {
    render(entries, page, None)
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
pub fn render_live(entries: &[Entry], page: &Page, nonce: &str) -> String {
    let nonce: String = nonce.chars().filter(char::is_ascii_alphanumeric).collect();
    render(entries, page, Some(&nonce))
}

/// One agent on the project: its role when it has one, else the tool's name.
struct Agent<'a> {
    name: String,
    tools: BTreeSet<&'a str>,
    models: BTreeSet<&'a str>,
    sessions: BTreeSet<&'a str>,
    /// Newest first.
    runs: Vec<&'a Entry>,
}

fn agent_name(r: &Receipt) -> String {
    r.worker
        .role
        .clone()
        .or_else(|| r.worker.agent.clone())
        .unwrap_or_else(|| UNNAMED.to_string())
}

/// Agents in order of their latest run (`runs` is newest first).
fn agents<'a>(runs: &[&'a Entry]) -> Vec<Agent<'a>> {
    let mut out: Vec<Agent<'a>> = Vec::new();
    for e in runs {
        let name = agent_name(&e.receipt);
        let i = match out.iter().position(|a| a.name == name) {
            Some(i) => i,
            None => {
                out.push(Agent {
                    name,
                    tools: BTreeSet::new(),
                    models: BTreeSet::new(),
                    sessions: BTreeSet::new(),
                    runs: Vec::new(),
                });
                out.len() - 1
            }
        };
        let (Some(a), w) = (out.get_mut(i), &e.receipt.worker) else {
            continue;
        };
        if let (Some(_), Some(t)) = (&w.role, &w.agent) {
            a.tools.insert(t);
        }
        if let Some(m) = &w.model {
            a.models.insert(m);
        }
        if let Some(s) = &w.session {
            a.sessions.insert(s);
        }
        a.runs.push(e);
    }
    out
}

/// The agent's filter index, when it has one (the first `MAX_AGENTS` agents).
fn agent_index(agents: &[Agent], r: &Receipt) -> Option<usize> {
    let name = agent_name(r);
    agents.iter().take(MAX_AGENTS).position(|a| a.name == name)
}

/// A round initial for an agent. Its colour (one of 8) comes from the name, so an agent keeps its
/// colour when others join or the order changes.
fn avatar(name: &str) -> String {
    // FNV-1a: stable across runs and platforms (std's hasher is randomly seeded).
    let hash = name.bytes().fold(0x811c_9dc5_u32, |h, b| {
        (h ^ u32::from(b)).wrapping_mul(0x0100_0193)
    });
    let letter: String = name
        .chars()
        .find(|c| c.is_alphanumeric())
        .map(|c| c.to_uppercase().collect())
        .unwrap_or_else(|| "?".to_string());
    format!(
        "<i class=\"av c{}\" aria-hidden=\"true\">{}</i>",
        hash % 8,
        escape(&letter)
    )
}

/// Verified / failed / unverified as a stacked bar (widths in whole percent).
fn verdict_bar(v: usize, f: usize, u: usize) -> String {
    let total = (v + f + u).max(1);
    let pct = |n: usize| n * 100 / total;
    format!(
        "<span class=\"vbar\" aria-hidden=\"true\"><i class=\"bv\" style=\"width:{}%\"></i>\
         <i class=\"bf\" style=\"width:{}%\"></i><i class=\"bu\" style=\"width:{}%\"></i></span>",
        pct(v),
        pct(f),
        pct(u)
    )
}

fn agents_html(agents: &[Agent]) -> String {
    let mut h = String::new();
    let n = agents.len();
    let _ = write!(
        h,
        "<section class=\"agents\"><p class=\"eyebrow\">Agents</p>\
         <h2>{n} {} on this project</h2><p class=\"muted hint\">Each row is one agent: its role when it has one \
         (<code>--role</code> or <code>COUNTERCHECK_ROLE</code>), else the tool. Pick one to see only its runs.</p>\
         <div class=\"agent-list\">",
        if n == 1 { "agent" } else { "agents" }
    );
    for (i, a) in agents.iter().take(MAX_AGENTS).enumerate() {
        let count = |v: Verdict| a.runs.iter().filter(|e| e.receipt.verdict == v).count();
        let (v, f, u) = (
            count(Verdict::Verified),
            count(Verdict::Failed),
            count(Verdict::Unverified),
        );
        let mut sub: Vec<String> = a.tools.iter().map(|t| escape(t)).collect();
        sub.extend(a.models.iter().map(|m| escape(m)));
        if !a.sessions.is_empty() {
            let s = a.sessions.len();
            sub.push(format!("{s} session{}", if s == 1 { "" } else { "s" }));
        }
        if a.name == UNNAMED {
            sub.push("run without <code>--agent</code> or a role".to_string());
        }
        // Oldest left, newest right: the last 20 runs as dots.
        let trail: String = a
            .runs
            .iter()
            .take(20)
            .rev()
            .map(|e| format!("<i class=\"d {}\"></i>", verdict_class(e.receipt.verdict)))
            .collect();
        let latest = a.runs.first().map(|e| &e.receipt);
        let _ = write!(
            h,
            "<label class=\"agent a-{i}\" for=\"fa-{i}\"><span class=\"a-main\">{av}<b>{name}</b>\
             <small>{sub}</small></span><span class=\"a-trail\" title=\"last runs, oldest first\">{trail}</span>\
             <span class=\"a-stats\">{bar}<small><b>{runs}</b> run{s}{tally}</small></span>\
             <span class=\"a-last\">{last}</span></label>",
            av = avatar(&a.name),
            name = escape(&a.name),
            sub = if sub.is_empty() {
                "&nbsp;".to_string()
            } else {
                sub.join(" · ")
            },
            bar = verdict_bar(v, f, u),
            runs = a.runs.len(),
            s = if a.runs.len() == 1 { "" } else { "s" },
            tally = [(v, "verified"), (f, "failed"), (u, "unverified")]
                .iter()
                .filter(|(n, _)| *n > 0)
                .map(|(n, w)| format!(" · {n} {w}"))
                .collect::<String>(),
            last = latest
                .map(|r| format!(
                    "{}<small><span class=\"badge {}\">{}</span> · latest, {}</small>",
                    match r.claim.as_deref() {
                        Some(c) => format!("<q>{}</q>", escape(&short(c, 90))),
                        None => "<small class=\"no-claim\">no claim given</small>".to_string(),
                    },
                    verdict_class(r.verdict),
                    r.verdict.label(),
                    escape(&pretty_time(&r.created_at))
                ))
                .unwrap_or_default(),
        );
    }
    h.push_str("</div>");
    if n > MAX_AGENTS {
        let _ = write!(
            h,
            "<p class=\"muted\">and {} more agents (their runs are in the list below)</p>",
            n - MAX_AGENTS
        );
    }
    h.push_str("</section>");
    h
}

/// Filters that work without a script: radio buttons, and CSS that hides the runs they rule out.
fn filters_html(agents: &[Agent], counts: [usize; 4]) -> String {
    let [all, v, f, u] = counts;
    let mut h = String::from(
        "<div class=\"filters\" role=\"group\" aria-label=\"Filter runs\"><div class=\"seg\">",
    );
    for (id, label, n, checked) in [
        ("fv-all", "All", all, true),
        ("fv-failed", "Failed", f, false),
        ("fv-unverified", "Unverified", u, false),
        ("fv-verified", "Verified", v, false),
    ] {
        let _ = write!(
            h,
            "<input type=\"radio\" name=\"fv\" id=\"{id}\"{}><label for=\"{id}\">{label} <span>{n}</span></label>",
            if checked { " checked" } else { "" }
        );
    }
    h.push_str("</div>");
    if agents.len() > 1 {
        h.push_str("<div class=\"seg\"><input type=\"radio\" name=\"fa\" id=\"fa-all\" checked><label for=\"fa-all\">All agents</label>");
        for (i, a) in agents.iter().take(MAX_AGENTS).enumerate() {
            let _ = write!(
                h,
                "<input type=\"radio\" name=\"fa\" id=\"fa-{i}\"><label for=\"fa-{i}\">{} <span>{}</span></label>",
                escape(&a.name),
                a.runs.len()
            );
        }
        h.push_str("</div>");
    } else if let Some(a) = agents.first() {
        // One agent: its radio still exists, so its row in the panel stays a working control.
        let _ = write!(
            h,
            "<input type=\"radio\" name=\"fa\" id=\"fa-all\" checked hidden><input type=\"radio\" name=\"fa\" id=\"fa-0\" hidden aria-label=\"{}\">",
            escape(&a.name)
        );
    }
    h.push_str("</div>");
    h
}

/// The filter rules (digits and fixed text only).
fn filters_css(agents: &[Agent]) -> String {
    let mut h = String::new();
    for i in 0..agents.len().min(MAX_AGENTS) {
        let _ = write!(
            h,
            "body:has(#fa-{i}:checked) .run:not(.a-{i}){{display:none}}\
             body:has(#fa-{i}:checked) .agent.a-{i}{{border-color:var(--ink);background:var(--card)}}"
        );
    }
    h
}

fn render(entries: &[Entry], page: &Page, live: Option<&str>) -> String {
    let (title, generated_at) = (page.title, page.generated_at);
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
        "<nav class=\"top\"><span class=\"brand\">{}Agent&nbsp;Counter<em>check</em></span>{}</nav><main>",
        MARK.trim_end(),
        if live.is_some() {
            "<span class=\"pill live\"><span class=\"dot\"></span><span id=\"live\">live</span></span>"
        } else {
            "<span class=\"pill\">offline report</span>"
        }
    );
    let _ = write!(
        h,
        "<header><p class=\"eyebrow\">Agent Countercheck · receipts</p><h1>{}</h1>\
         <p class=\"path\" title=\"Project folder\"><code>{}</code></p>\
         <p class=\"lead\">Every time an agent said <q>done</q>, re-checked on rules it couldn't change.</p>\
         <p class=\"meta\">Generated {} · newest first</p></header>",
        escape(title),
        escape(page.path),
        escape(&pretty_time(generated_at)),
    );
    if !page.hooks_off.is_empty() {
        h.push_str(
            "<section class=\"hooks-off\" role=\"alert\"><p class=\"hooks-off-h\">Hooks are switched off in this project</p>\
             <p>Agents here stop without being checked, so no receipt is written and this page can look clean. Undo it, or use the GitHub Action as the backstop:</p><ul>",
        );
        for f in page.hooks_off {
            let _ = write!(
                h,
                "<li><code>{}</code> {}</li>",
                escape(&f.path),
                escape(&f.what)
            );
        }
        h.push_str("</ul></section>");
    }
    if !page.not_checked.is_empty() {
        let n = page.not_checked.len();
        let _ = write!(
            h,
            "<section class=\"not-checked\"><p class=\"nc-h\">Not checked: {n} session{}{}</p>\
             <p>Started in this project but no receipt: the Countercheck hook was switched off or removed, it timed out, \
             or the agent was never given a task. Nothing here was verified.</p><ul>",
            if n == 1 { "" } else { "s" },
            if n == crate::sessions::MAX_SHOWN {
                " (newest shown)"
            } else {
                ""
            },
        );
        for s in page.not_checked {
            let short: String = s.id.chars().take(12).collect();
            let more = if s.id.chars().count() > 12 { "…" } else { "" };
            let _ = write!(
                h,
                "<li><b>{}</b>{} · session <code title=\"{}\">{}{more}</code> · started {}</li>",
                escape(s.agent.as_deref().unwrap_or(UNNAMED)),
                s.model
                    .as_deref()
                    .map(|m| format!(" <span class=\"muted\">{}</span>", escape(m)))
                    .unwrap_or_default(),
                escape(&s.id),
                escape(&short),
                escape(&pretty_time(&crate::sessions::rfc3339(s.started_unix))),
            );
        }
        h.push_str("</ul></section>");
    }

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
    h.push_str(&verdict_bar(
        count(Verdict::Verified),
        count(Verdict::Failed),
        count(Verdict::Unverified),
    ));
    let agents = agents(&runs);
    // Rules built from numbers only, emitted once at the end of the page.
    let mut css = filters_css(&agents);

    if runs.is_empty() && page.running.is_empty() {
        h.push_str("<p>No receipts yet. Run <code>countercheck verify</code> first.</p>");
    }
    if let Some(f) = flow::flow(&agents, page.running) {
        h.push_str(&f.html);
        css.push_str(&f.css);
    }
    h.push_str(&flow::running_html(page.running, live.is_some()));
    if !agents.is_empty() {
        h.push_str(&agents_html(&agents));
    }
    if let Some(latest) = runs.first() {
        h.push_str(&matrix_grid(&latest.receipt));
    }

    h.push_str(
        "<section class=\"runs\"><p class=\"eyebrow\">Runs</p><h2>What each agent claimed, and what ran</h2>",
    );
    h.push_str(&filters_html(
        &agents,
        [
            runs.len(),
            count(Verdict::Verified),
            count(Verdict::Failed),
            count(Verdict::Unverified),
        ],
    ));
    for (i, e) in runs.iter().enumerate() {
        h.push_str(&run_html(e, i == 0, agent_index(&agents, &e.receipt)));
    }
    h.push_str("</section>");
    let _ = write!(h, "<style>{css}</style>");
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

fn run_html(e: &Entry, open: bool, agent: Option<usize>) -> String {
    let r = &e.receipt;
    let passed = r.checks.iter().filter(|c| c.passed).count();
    let name = agent_name(r);
    // The role first, then the tool and model that played it.
    let mut parts = vec![name.clone()];
    if r.worker.role.is_some() {
        parts.extend(r.worker.agent.clone());
    }
    parts.extend(r.worker.model.clone());
    let who = parts.join(" · ");
    let claim = r.claim.as_deref().unwrap_or("");
    let mut h = String::new();
    let _ = write!(
        h,
        "<details class=\"run v-{cls}{a}\" id=\"r-{id}\"{open}><summary>\
         <span class=\"badge {cls}\">{label}</span>\
         <span class=\"when\">{when}</span>\
         <span class=\"who\">{av}{who}</span>\
         <span class=\"count\">{passed}/{total} checks</span>{seal}\
         <span class=\"claim\">{claim}</span></summary>",
        id = escape(&r.id),
        open = if open { " open" } else { "" },
        cls = verdict_class(r.verdict),
        label = r.verdict.label(),
        when = escape(&pretty_time(&r.created_at)),
        who = escape(&who),
        a = agent.map(|i| format!(" a-{i}")).unwrap_or_default(),
        av = avatar(&name),
        total = r.checks.len(),
        seal = if e.seal_ok {
            "<span class=\"seal ok\">seal intact</span>"
        } else {
            "<span class=\"seal bad\">SEAL BROKEN</span>"
        },
        claim = escape(&short(claim, CLAIM_TITLE_CHARS)),
    );

    h.push_str("<div class=\"body\">");
    h.push_str(&steps_html(e, &who, open));
    if !r.reasons.is_empty() {
        h.push_str("<ul class=\"reasons\">");
        for reason in &r.reasons {
            let _ = write!(h, "<li>{}</li>", escape(reason));
        }
        h.push_str("</ul>");
    }
    // The title shows the claim; repeat it in full only when the title had to cut it.
    if claim.chars().count() > CLAIM_TITLE_CHARS {
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
        "<div class=\"rcpt {cls}\"><span class=\"wax\" aria-hidden=\"true\"></span><p class=\"rcpt-h\">Receipt {id} · {label}</p>\
         <dl class=\"facts\">\
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
            " + <b>uncommitted changes</b> (this receipt covers the files on disk, not only the commit)"
        } else {
            ""
        },
        sandbox = escape(&r.sandbox),
        tree = escape(&short(&r.tree_sha256, 16)),
        cls = verdict_class(r.verdict),
        label = r.verdict.label(),
    );
    h.push_str("</div></div></details>");
    h
}

/// What happened after the agent said "done", step by step, from the receipt. Opening a run plays
/// the steps in order (CSS); the run open when the page loads shows them at once.
fn steps_html(e: &Entry, who: &str, open: bool) -> String {
    let r = &e.receipt;
    let passed = r.checks.iter().filter(|c| c.passed).count();
    // Only files whose worker version the checks didn't use; new protected files are kept.
    let undone: Vec<&str> = r
        .protected_changes
        .iter()
        .filter(|p| p.undone())
        .map(|p| p.path.as_str())
        .collect();
    let kept = r.protected_changes.len() - undone.len();
    let protected = undone.len();
    let mut steps: Vec<(&str, String, String)> = vec![
        (
            "ok",
            if r.claim.is_some() {
                "Agent said done"
            } else {
                "Agent asked for a check (no claim given)"
            }
            .to_string(),
            escape(who),
        ),
        if r.checks_locked {
            (
                "ok",
                "Checks locked at the base commit".to_string(),
                escape(&r.checks_source),
            )
        } else {
            (
                "warn",
                "Checks not locked".to_string(),
                format!("{}: not in the base revision", escape(&r.checks_source)),
            )
        },
        (
            "ok",
            "Clean copy of the work".to_string(),
            format!("tree sha256 {}", escape(&short(&r.tree_sha256, 12))),
        ),
        if protected > 0 {
            (
                "bad",
                format!(
                    "Put back {protected} protected file{}",
                    if protected == 1 { "" } else { "s" }
                ),
                format!(
                    "the agent changed {}; the checks used the base version",
                    escape(&undone.join(", "))
                ),
            )
        } else if kept > 0 {
            (
                "ok",
                "No protected file changed".to_string(),
                format!(
                    "{kept} new file{} on protected paths kept (new tests are allowed)",
                    if kept == 1 { "" } else { "s" }
                ),
            )
        } else {
            (
                "ok",
                "No protected file changed".to_string(),
                "tests and check files are as they were".to_string(),
            )
        },
    ];
    // One bar per check, its width from how long it ran.
    let bars: String = r
        .checks
        .iter()
        .map(|c| {
            format!(
                "<i class=\"{}\" style=\"flex-grow:{}\" title=\"{} · {} ms\"></i>",
                if c.passed { "ok" } else { "bad" },
                c.duration_ms.max(1),
                escape(&flow::check_label(&c.name)),
                c.duration_ms
            )
        })
        .collect();
    // Saturating: durations come from a file the worker could have edited.
    let total_ms = r
        .checks
        .iter()
        .fold(0u64, |t, c| t.saturating_add(c.duration_ms));
    steps.push((
        if r.checks.is_empty() {
            "warn"
        } else if passed == r.checks.len() {
            "ok"
        } else {
            "bad"
        },
        format!("Checks ran: {passed} of {} passed", r.checks.len()),
        format!("{total_ms} ms in the sandbox<span class=\"bars\">{bars}</span>"),
    ));
    steps.push(if e.seal_ok {
        (
            match r.verdict {
                Verdict::Verified => "ok",
                Verdict::Failed => "bad",
                Verdict::Unverified => "warn",
            },
            format!("Receipt sealed: {}", r.verdict.label()),
            "SHA-256 over every field; editing it later breaks the seal".to_string(),
        )
    } else {
        (
            "bad",
            "Seal broken".to_string(),
            "this receipt was edited after it was written".to_string(),
        )
    });
    let mut h = format!("<ol class=\"steps{}\">", if open { " now" } else { "" });
    for (i, (cls, title, sub)) in steps.iter().enumerate() {
        let _ = write!(
            h,
            "<li class=\"st {cls}\" style=\"--i:{i}\"><span class=\"k\">{}</span><span><b>{}</b><small>{sub}</small></span></li>",
            if *cls == "bad" {
                "!".to_string()
            } else {
                (i + 1).to_string()
            },
            escape(title),
        );
    }
    h.push_str("</ol>");
    h
}

const HEAD_START: &str = "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
<meta name=\"referrer\" content=\"no-referrer\">";

/// Styles for the report and the live UI: `assets/report.css`, compiled into the binary.
const STYLE: &str = include_str!("../assets/report.css");
/// The logo (docs/assets/brand/mark-small.svg without filters), inline: the page's
/// Content-Security-Policy allows no image files.
const MARK: &str = include_str!("../assets/mark.svg");

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
                role: None,
                session: None,
            },
            claim: Some("Done, all tests pass <script>alert(1)</script>".to_string()),
            claim_status: ClaimStatus::NoClaim,
            checks_locked: true,
            checks_source: ".countercheck/checks.toml @ HEAD".to_string(),
            checks_external: None,
            sandbox: "none".to_string(),
            tree_sha256: "f".repeat(64),
            linked: vec![],
            protected_changes: vec![],
            uncovered: None,
            watched: None,
            hook_settings: Vec::new(),
            checks,
            verdict,
            reasons: vec!["check \"t\" failed".to_string()],
        }
    }

    fn page() -> Page<'static> {
        Page {
            title: "r",
            path: "~/r",
            generated_at: "now",
            running: &[],
            hooks_off: &[],
            not_checked: &[],
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
            sandboxed: None,
            unsandboxed_reason: None,
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
            &Page {
                title: "repo <b>",
                path: "~/code/<i>repo",
                generated_at: "now",
                running: &[],
                hooks_off: &[],
                not_checked: &[],
            },
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
        let html = render_html(&entries, &page());
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
            &page(),
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
        let html = render_live(&e, &page(), "abc123\"><x");
        assert_eq!(html.matches("<script").count(), 1, "{html}");
        assert!(html.contains("<script nonce=\"abc123x\">"));
        assert!(html.contains("script-src 'nonce-abc123x'"));
        assert!(html.contains("connect-src 'self'"));
        assert!(html.contains("id=\"live\""));
        assert!(!render_html(&e, &page()).contains("<script"));
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
        let html = render_live(&[], &page(), "n1");
        assert!(html.contains("location.reload()"));
        assert!(html.contains("<style>:root") || html.contains("<style>/*"));
    }

    #[test]
    fn empty_report_says_how_to_start() {
        let html = render_html(&[], &page());
        assert!(html.contains("No receipts yet"));
    }

    fn by(role: Option<&str>, agent: &str, created: &str, verdict: Verdict) -> Entry {
        let mut r = receipt(
            created,
            verdict,
            vec![check("t", verdict == Verdict::Verified, "")],
        );
        r.worker.role = role.map(str::to_string);
        r.worker.agent = Some(agent.to_string());
        r.worker.session = Some(format!("s-{created}"));
        Entry {
            file: format!("{created}.json"),
            receipt: r,
            seal_ok: true,
        }
    }

    #[test]
    fn agents_panel_has_one_row_per_role_newest_first() {
        let e = vec![
            by(
                Some("UI design"),
                "claude-code",
                "2026-01-01T00:00:01Z",
                Verdict::Failed,
            ),
            by(
                Some("Security <review>"),
                "claude-code",
                "2026-01-01T00:00:02Z",
                Verdict::Verified,
            ),
            by(
                Some("UI design"),
                "claude-code",
                "2026-01-01T00:00:03Z",
                Verdict::Verified,
            ),
            by(None, "codex", "2026-01-01T00:00:04Z", Verdict::Unverified),
        ];
        let html = render_html(&e, &page());
        assert!(html.contains("<h2>3 agents on this project</h2>"), "{html}");
        // Order: the agent with the newest run first; a role names the agent, else the tool.
        let codex = html.find("<b>codex</b>").unwrap();
        let ui = html.find("<b>UI design</b>").unwrap();
        let sec = html.find("<b>Security &lt;review&gt;</b>").unwrap();
        assert!(codex < ui && ui < sec, "{html}");
        // Two runs, one session each, the tool shown under the role.
        assert!(
            html.contains("<b>2</b> runs · 1 verified · 1 failed"),
            "{html}"
        );
        assert!(html.contains("claude-code · 2 sessions"), "{html}");
        // The dot trail runs oldest to newest.
        assert!(
            html.contains("<i class=\"d failed\"></i><i class=\"d verified\"></i>"),
            "{html}"
        );
        // Every run carries its agent's filter class; each agent has a filter rule.
        assert_eq!(html.matches("class=\"run v-").count(), 4);
        assert!(html.contains("class=\"run v-failed a-1\""), "{html}");
        assert!(
            html.contains("body:has(#fa-2:checked) .run:not(.a-2){display:none}"),
            "{html}"
        );
        assert!(
            html.contains("<label for=\"fa-1\">UI design <span>2</span></label>"),
            "{html}"
        );
    }

    #[test]
    fn agents_beyond_the_limit_keep_their_runs_but_get_no_row() {
        let e: Vec<Entry> = (0..MAX_AGENTS + 2)
            .map(|i| {
                by(
                    Some(&format!("agent {i}")),
                    "x",
                    &format!("2026-01-01T00:00:{i:02}Z"),
                    Verdict::Verified,
                )
            })
            .collect();
        let html = render_html(&e, &page());
        assert_eq!(html.matches("<label class=\"agent ").count(), MAX_AGENTS);
        assert!(html.contains("and 2 more agents"), "{html}");
        assert_eq!(html.matches("class=\"run v-").count(), MAX_AGENTS + 2);
        assert!(
            !html.contains(&format!("#fa-{MAX_AGENTS}:checked")),
            "{html}"
        );
    }

    #[test]
    fn the_page_shows_the_project_path_and_a_verdict_bar() {
        let e = vec![
            by(None, "a", "2026-01-01T00:00:01Z", Verdict::Verified),
            by(None, "a", "2026-01-01T00:00:02Z", Verdict::Failed),
        ];
        let html = render_html(
            &e,
            &Page {
                title: "shop",
                path: "~/code/<shop>",
                generated_at: "now",
                running: &[],
                hooks_off: &[],
                not_checked: &[],
            },
        );
        assert!(html.contains(
            "<p class=\"path\" title=\"Project folder\"><code>~/code/&lt;shop&gt;</code></p>"
        ));
        assert!(html.contains("<i class=\"bv\" style=\"width:50%\"></i><i class=\"bf\" style=\"width:50%\"></i><i class=\"bu\" style=\"width:0%\"></i>"), "{html}");
    }

    #[test]
    fn a_short_claim_is_the_title_only_and_a_long_one_is_also_shown_in_full() {
        let mut short_one = by(None, "a", "2026-01-01T00:00:01Z", Verdict::Verified);
        short_one.receipt.claim = Some("Done.".to_string());
        let html = render_html(&[short_one], &page());
        assert!(!html.contains("The worker said"), "{html}");
        let mut long_one = by(None, "a", "2026-01-01T00:00:01Z", Verdict::Verified);
        let claim = "x".repeat(CLAIM_TITLE_CHARS + 1);
        long_one.receipt.claim = Some(claim.clone());
        let html = render_html(&[long_one], &page());
        assert!(
            html.contains(&format!("<blockquote>{claim}</blockquote>")),
            "{html}"
        );
    }

    fn by_checks(role: &str, created: &str, verdict: Verdict, checks: Vec<CheckResult>) -> Entry {
        let mut e = by(Some(role), "claude-code", created, verdict);
        e.receipt.checks = checks;
        e
    }

    #[test]
    fn flow_map_routes_each_agent_to_its_outcome_and_links_failed_checks() {
        let e = vec![
            by_checks(
                "UI design",
                "2026-01-01T00:00:01Z",
                Verdict::Verified,
                vec![check("unit", true, ""), check("lint", true, "")],
            ),
            by_checks(
                "Security",
                "2026-01-01T00:00:02Z",
                Verdict::Failed,
                vec![check("unit", false, "boom"), check("lint", true, "")],
            ),
            by_checks(
                "Refactor",
                "2026-01-01T00:00:03Z",
                Verdict::Unverified,
                vec![check("unit", true, "")],
            ),
        ];
        let html = render_html(&e, &page());
        assert!(html.contains("<section class=\"flow\""), "{html}");
        // Newest first: Refactor a-0, Security a-1, UI design a-2. Each node is a label for its filter.
        assert!(
            html.contains("<label for=\"fa-1\" class=\"fn a-1 x-0 failed\""),
            "{html}"
        );
        assert!(
            html.contains("<label for=\"fa-0\" class=\"fn a-0 unverified\""),
            "{html}"
        );
        // Two lines per agent; only the delivered one gets the pulse.
        assert_eq!(html.matches("<path class=\"fe ").count(), 6);
        assert_eq!(html.matches("<path class=\"pulse a-2\"").count(), 1);
        // The failed check knows who failed it; outcomes count each agent once.
        assert!(
            html.contains("<li class=\"fc c-0 f-a-1\" title=\"unit\">unit</li>"),
            "{html}"
        );
        assert!(
            html.contains("<li class=\"fc c-1\" title=\"lint\">lint</li>"),
            "{html}"
        );
        for (cls, n) in [("verified", 1), ("failed", 1), ("unverified", 1)] {
            assert!(html.contains(&format!("<div class=\"fo {cls}\"")), "{cls}");
            let at = html.find(&format!("<div class=\"fo {cls}\"")).unwrap_or(0);
            assert!(
                html[at..].contains(&format!("<b class=\"fo-n\">{n}</b>")),
                "{cls}"
            );
        }
        // Highlight rules for every agent and check, in the one generated stylesheet.
        assert!(
            html.contains(".fl.m-each:has(.fn.a-1:hover) :is(.fe,.fe-run,.fn,.pulse):not(.a-1)"),
            "{html}"
        );
        assert!(
            html.contains(".fl.m-each:has(.fc.c-0:hover) :is(.fe,.fe-run,.fn,.pulse):not(.x-0)")
                && html.contains("body:has(#fa-1:checked) .fl.m-each"),
            "{html}"
        );
        assert_eq!(html.matches("<style>").count(), 2);
    }

    #[test]
    fn many_agents_are_grouped_by_tool_with_a_switch_to_each_agent() {
        let total = MAX_AGENTS + 3;
        let many: Vec<Entry> = (0..total)
            .map(|i| {
                let failed = i % 3 == 0;
                let mut e = by_checks(
                    &format!("agent {i}"),
                    &format!("2026-01-01T00:{:02}:{:02}Z", i / 60, i % 60),
                    if failed {
                        Verdict::Failed
                    } else {
                        Verdict::Verified
                    },
                    (0..flow::MAX_FLOW_CHECKS + 2)
                        .map(|k| check(&format!("c{k}"), !(failed && k == 0), ""))
                        .collect(),
                );
                e.receipt.worker.agent =
                    Some(if i % 2 == 0 { "codex" } else { "claude-code" }.into());
                e
            })
            .collect();
        let html = render_html(&many, &page());
        // Grouped by tool by default; "Each agent" shows the newest MAX_AGENTS.
        assert!(html.contains("<input type=\"radio\" name=\"flv\" id=\"flv-tool\" checked>"));
        assert!(html.contains("<div class=\"fl m-tool\" style="), "{html}");
        assert!(
            html.contains("<div class=\"fl m-each\" data-alt style="),
            "{html}"
        );
        assert_eq!(html.matches("<div class=\"fn g-").count(), 2, "{html}");
        assert_eq!(
            html.matches("\" class=\"fn a-").count(),
            MAX_AGENTS,
            "{html}"
        );
        assert!(
            html.contains(&format!("<small>{} agents", total.div_ceil(2))),
            "{html}"
        );
        assert!(
            html.contains("3 more are in the Agents panel below"),
            "{html}"
        );
        // A group with mixed outcomes: one line per outcome, wider for more agents.
        assert!(html.contains("<path class=\"fe g-0"), "{html}");
        assert!(html.contains(" mixed\" style="), "{html}");
        assert!(html.contains("stroke-width:"), "{html}");
        assert!(
            html.contains("<li class=\"fc more\">and 2 more</li>"),
            "{html}"
        );
        // Outcome counts include every agent.
        let failed = total.div_ceil(3);
        assert!(html.contains(&format!("<b class=\"fo-n\">{}</b>", total - failed)));
        assert!(html.contains(&format!("<b class=\"fo-n\">{failed}</b>")));
        // Rules are scoped per map, so the two maps' lanes don't light each other.
        assert!(html.contains(".fl.m-tool:has(.fn.g-0:hover)"), "{html}");
        assert!(html.contains(" .fc.f-g-"), "{html}");
        assert!(!render_html(&[], &page()).contains("<section class=\"flow\""));
    }

    fn progress(
        role: &str,
        stage: crate::progress::Stage,
        current: Option<&str>,
    ) -> crate::progress::Progress {
        crate::progress::Progress {
            schema: crate::progress::SCHEMA.into(),
            id: "p1".into(),
            started_at: "2026-01-01T00:01:00Z".into(),
            updated_unix: 0,
            stale_after_unix: u64::MAX,
            worker: crate::receipt::Worker {
                agent: Some("codex".into()),
                model: Some("m<1>".into()),
                cost_usd: None,
                role: Some(role.into()),
                session: None,
            },
            claim: Some("Fixed <it>.".into()),
            stage,
            checks_total: 4,
            checks_done: 1,
            current: current.map(str::to_string),
        }
    }

    #[test]
    fn runs_in_progress_show_on_the_map_and_in_a_list() {
        use crate::progress::Stage;
        let e = vec![by(
            Some("UI design"),
            "claude-code",
            "2026-01-01T00:00:01Z",
            Verdict::Verified,
        )];
        let running = vec![
            progress("UI design", Stage::Check, Some("tenant {t} [t=beta]")),
            progress("Docs <x>", Stage::Copy, None),
        ];
        let with = |entries: &[Entry], running: &[crate::progress::Progress]| {
            render_html(
                entries,
                &Page {
                    title: "r",
                    path: "~/r",
                    generated_at: "now",
                    running,
                    hooks_off: &[],
                    not_checked: &[],
                },
            )
        };
        let html = with(&e, &running);
        // A known agent's lane turns "checking"; a new agent gets its own lane first.
        assert!(html.contains("<div class=\"fn r-0 checking\""), "{html}");
        assert!(
            html.contains("<label for=\"fa-0\" class=\"fn a-0 checking\""),
            "{html}"
        );
        assert!(
            html.contains("checking now · check 2 of 4 · tenant · beta"),
            "{html}"
        );
        assert_eq!(html.matches("<path class=\"fe-run ").count(), 2, "{html}");
        assert!(
            html.contains("<small class=\"fg-run\">2 checking now</small>"),
            "{html}"
        );
        // The list says it isn't a receipt, and escapes what the progress file says.
        assert!(html.contains("<h2>2 runs in progress</h2>"), "{html}");
        assert!(html.contains("Not receipts: a verdict exists only once the receipt is sealed."));
        assert!(
            html.contains("<b>Docs &lt;x&gt; · codex · m&lt;1&gt;</b><q>Fixed &lt;it&gt;.</q>"),
            "{html}"
        );
        assert!(html.contains("making a clean copy · started"), "{html}");
        // Steps 1-2 done, step 3 (copy) now, the rest to come.
        assert!(
            html.contains("<i class=\"done\"></i><i class=\"done\"></i><i class=\"now\"></i><i class=\"\"></i>"),
            "{html}"
        );
        // Progress alone (no receipt yet) still draws a map.
        let html = with(&[], &running[1..]);
        assert!(html.contains("<section class=\"flow\""), "{html}");
        assert!(!html.contains("No receipts yet"), "{html}");
    }

    #[test]
    fn matrix_check_names_are_shortened_on_the_map() {
        assert_eq!(
            flow::check_label("tenant {tenant} [tenant=beta]"),
            "tenant · beta"
        );
        assert_eq!(flow::check_label("{schema} [schema=CA1]"), "schema · CA1");
        assert_eq!(flow::check_label("unit tests"), "unit tests");
    }

    #[test]
    fn steps_say_when_new_protected_files_were_only_kept() {
        let mut e = by_checks(
            "UI",
            "2026-01-01T00:00:01Z",
            Verdict::Verified,
            vec![check("unit", true, "")],
        );
        e.receipt.protected_changes = vec![crate::protect::ProtectedChange::decide(
            "tests/new_test.py",
            crate::protect::Change::Added,
        )];
        let html = render_html(&[e], &page());
        assert!(!html.contains("Put back"), "{html}");
        assert!(
            html.contains("1 new file on protected paths kept (new tests are allowed)"),
            "{html}"
        );
    }

    #[test]
    fn steps_replay_what_happened_from_the_receipt() {
        let mut e = by_checks(
            "UI",
            "2026-01-01T00:00:01Z",
            Verdict::Failed,
            vec![check("unit", false, "x"), check("lint", true, "")],
        );
        // One changed test (put back) and two new files that were kept: only the first counts.
        e.receipt.protected_changes = vec![
            crate::protect::ProtectedChange::decide(
                "tests/a<b>.py",
                crate::protect::Change::Modified,
            ),
            crate::protect::ProtectedChange::decide(
                ".countercheck/checks.toml",
                crate::protect::Change::Added,
            ),
            crate::protect::ProtectedChange::decide(
                "tests/pytest.ini",
                crate::protect::Change::Added,
            ),
        ];
        let older = by(
            Some("UI"),
            "claude-code",
            "2026-01-01T00:00:00Z",
            Verdict::Verified,
        );
        let html = render_html(&[e, older], &page());
        // The open run shows its steps at once; a closed one replays them when opened.
        assert!(html.contains("<ol class=\"steps now\">"), "{html}");
        assert!(html.contains("<ol class=\"steps\">"), "{html}");
        assert!(
            html.contains(
                "Put back 1 protected file</b><small>the agent changed tests/a&lt;b&gt;.py"
            ),
            "{html}"
        );
        assert!(html.contains("<b>Checks ran: 1 of 2 passed</b>"), "{html}");
        assert!(html.contains("<b>Receipt sealed: FAILED</b>"), "{html}");
        assert!(
            html.contains("<div class=\"rcpt failed\"><span class=\"wax\""),
            "{html}"
        );
        // New protected files are kept, never counted as put back.
        assert!(!html.contains("Put back 3"), "{html}");
    }

    #[test]
    fn an_agent_keeps_its_colour_whatever_the_order() {
        assert_eq!(avatar("UI design"), avatar("UI design"));
        let colour = |n: &str| {
            avatar(n)
                .split("av c")
                .nth(1)
                .and_then(|s| s.chars().next())
        };
        let seen: BTreeSet<_> = ["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"]
            .iter()
            .map(|n| colour(n))
            .collect();
        assert!(
            seen.len() > 3,
            "names should spread over the colours: {seen:?}"
        );
    }
}
