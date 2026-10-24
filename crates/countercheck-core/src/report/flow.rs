//! The flow map at the top of the report: each agent's latest claim running into Countercheck
//! (the checks locked at the base commit) and out to what happened: delivered, sent back, or
//! left for a person. Runs still in progress show as a moving line into Countercheck.
//!
//! Up to [`MAX_FLOW_AGENTS`] agents get one lane each. With more, the map groups them by tool
//! (`claude-code · 7 agents`, line width by count), and a switch shows each agent instead.
//!
//! Text is HTML, escaped like the rest of the page; the lines are inline SVG built from numbers
//! only. Drawing in, the delivery pulse, the switch and the highlighting on hover are CSS, so the
//! offline report (which may run no script) gets all of it. The highlight rules are generated per
//! lane and per check, again from numbers and fixed text only.

use super::{Agent, MAX_AGENTS, avatar, escape, matrix_parts, verdict_class};
use crate::progress::Progress;
use crate::receipt::{Receipt, Verdict};
use std::fmt::Write as _;

/// Agents drawn one lane each; with more, the map groups them by tool.
pub const MAX_FLOW_AGENTS: usize = 12;
/// Checks listed in the Countercheck box; the rest are counted.
pub const MAX_FLOW_CHECKS: usize = 10;

/// Row pitch and node height of the agent column (px).
const ROW: usize = 58;
const NODE: usize = 48;
/// Where lines may enter the Countercheck box (below its title), and one check row's height (px).
const GATE_TOP: usize = 78;
const CHECK_ROW: usize = 26;

/// The map's HTML and its highlight rules (digits and fixed text only).
pub struct Flow {
    pub html: String,
    pub css: String,
}

/// `tenant {tenant} [tenant=beta]` → `tenant · beta`; other names as they are.
pub fn check_label(name: &str) -> String {
    let Some((base, key, value)) = matrix_parts(name) else {
        return name.to_string();
    };
    let mut plain = String::new();
    let mut depth = 0usize;
    for c in base.chars() {
        match c {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            c if depth == 0 => plain.push(c),
            _ => {}
        }
    }
    let plain = plain.split_whitespace().collect::<Vec<_>>().join(" ");
    let lead = if plain.is_empty() {
        key
    } else {
        plain.as_str()
    };
    format!("{lead} · {value}")
}

/// The name a run in progress is shown under: its role, else its tool (like a receipt's agent).
pub fn running_name(p: &Progress) -> String {
    p.worker
        .role
        .clone()
        .or_else(|| p.worker.agent.clone())
        .unwrap_or_else(|| "unknown agent".to_string())
}

/// "check 2 of 5 · unit tests", or the stage.
pub fn running_status(p: &Progress) -> String {
    match (p.stage, &p.current) {
        (crate::progress::Stage::Check, Some(name)) => format!(
            "check {} of {} · {}",
            p.checks_done + 1,
            p.checks_total.max(p.checks_done + 1),
            check_label(name)
        ),
        (stage, _) => stage.label().to_string(),
    }
}

/// Where outcome `v` sits on the right, as a share of the map's height.
fn outcome_share(v: Verdict) -> f64 {
    match v {
        Verdict::Verified => 0.2,
        Verdict::Failed => 0.5,
        Verdict::Unverified => 0.8,
    }
}

/// One row on the left: an agent, a group of agents using one tool, or a run with no receipt yet.
struct Lane<'a> {
    /// Class token, letters/digits/`-` only: `a-3` (agent 3, also its filter), `g-1`, `r-0`.
    key: String,
    /// The agent filter this lane's node selects (`fa-N`).
    filter: Option<usize>,
    name: String,
    /// Already escaped.
    sub: String,
    /// Oldest first.
    dots: Vec<Verdict>,
    /// Latest outcome(s): one for an agent, counts for a group.
    outs: Vec<(Verdict, usize)>,
    /// Indexes into the listed checks that the lane's latest run(s) failed.
    fails: Vec<usize>,
    running: Vec<&'a Progress>,
}

fn fails_of(r: &Receipt, checks: &[&str]) -> Vec<usize> {
    checks
        .iter()
        .enumerate()
        .filter(|(_, name)| r.checks.iter().any(|c| c.name == **name && !c.passed))
        .map(|(k, _)| k)
        .collect()
}

fn running_sub(running: &[&Progress]) -> Option<String> {
    match running {
        [] => None,
        [p] => Some(format!("checking now · {}", escape(&running_status(p)))),
        many => Some(format!("{} checking now", many.len())),
    }
}

/// One lane per agent (up to `cap`), plus runs in progress by agents with no receipt yet.
fn agent_lanes<'a>(
    agents: &[Agent],
    running: &'a [Progress],
    checks: &[&str],
    cap: usize,
) -> Vec<Lane<'a>> {
    let mut lanes: Vec<Lane<'a>> = Vec::new();
    // New agents that are checking right now come first: they are the newest activity.
    let mut new_names: Vec<String> = Vec::new();
    for p in running {
        let name = running_name(p);
        if agents.iter().any(|a| a.name == name) || new_names.contains(&name) {
            continue;
        }
        new_names.push(name);
    }
    for (j, name) in new_names.into_iter().enumerate().take(cap) {
        let mine: Vec<&Progress> = running.iter().filter(|p| running_name(p) == name).collect();
        let tool: Vec<String> = mine
            .first()
            .filter(|p| p.worker.role.is_some())
            .and_then(|p| p.worker.agent.as_deref())
            .map(escape)
            .into_iter()
            .collect();
        lanes.push(Lane {
            key: format!("r-{j}"),
            filter: None,
            sub: running_sub(&mine)
                .into_iter()
                .chain(tool)
                .collect::<Vec<_>>()
                .join(" · "),
            name,
            dots: Vec::new(),
            outs: Vec::new(),
            fails: Vec::new(),
            running: mine,
        });
    }
    let room = cap.saturating_sub(lanes.len());
    for (i, a) in agents.iter().enumerate().take(room) {
        let Some(latest) = a.runs.first().map(|e| &e.receipt) else {
            continue;
        };
        let mine: Vec<&Progress> = running
            .iter()
            .filter(|p| running_name(p) == a.name)
            .collect();
        let mut sub: Vec<String> = running_sub(&mine).into_iter().collect();
        sub.extend(a.tools.iter().map(|t| escape(t)));
        sub.extend(latest.worker.model.iter().map(|m| escape(m)));
        if sub.is_empty() {
            let k = a.runs.len();
            sub.push(format!("{k} run{}", if k == 1 { "" } else { "s" }));
        }
        lanes.push(Lane {
            key: format!("a-{i}"),
            filter: Some(i),
            name: a.name.clone(),
            sub: sub.join(" · "),
            dots: a
                .runs
                .iter()
                .take(8)
                .rev()
                .map(|e| e.receipt.verdict)
                .collect(),
            outs: vec![(latest.verdict, 1)],
            fails: fails_of(latest, checks),
            running: mine,
        });
    }
    lanes
}

/// One lane per tool, busiest first.
fn tool_lanes<'a>(agents: &[Agent], running: &'a [Progress], checks: &[&str]) -> Vec<Lane<'a>> {
    let tool_of = |r: &Receipt| {
        r.worker
            .agent
            .clone()
            .unwrap_or_else(|| "unknown tool".to_string())
    };
    let mut tools: Vec<String> = Vec::new();
    for a in agents {
        if let Some(e) = a.runs.first() {
            let t = tool_of(&e.receipt);
            if !tools.contains(&t) {
                tools.push(t);
            }
        }
    }
    for p in running {
        let t = p
            .worker
            .agent
            .clone()
            .unwrap_or_else(|| "unknown tool".to_string());
        if !tools.contains(&t) {
            tools.push(t);
        }
    }
    let mut lanes: Vec<Lane<'a>> = tools
        .iter()
        .enumerate()
        .map(|(g, tool)| {
            let members: Vec<&Receipt> = agents
                .iter()
                .filter_map(|a| a.runs.first().map(|e| &e.receipt))
                .filter(|r| &tool_of(r) == tool)
                .collect();
            let mine: Vec<&Progress> = running
                .iter()
                .filter(|p| p.worker.agent.as_deref().unwrap_or("unknown tool") == tool)
                .collect();
            let mut outs = Vec::new();
            for v in [Verdict::Verified, Verdict::Failed, Verdict::Unverified] {
                let n = members.iter().filter(|r| r.verdict == v).count();
                if n > 0 {
                    outs.push((v, n));
                }
            }
            let mut fails: Vec<usize> = members.iter().flat_map(|r| fails_of(r, checks)).collect();
            fails.sort_unstable();
            fails.dedup();
            let k = members.len();
            let mut sub = vec![format!("{k} agent{}", if k == 1 { "" } else { "s" })];
            if !mine.is_empty() {
                sub.push(format!("{} checking", mine.len()));
            }
            Lane {
                key: format!("g-{g}"),
                filter: None,
                name: tool.clone(),
                sub: sub.join(" · "),
                dots: members.iter().take(8).map(|r| r.verdict).collect(),
                outs,
                fails,
                running: mine,
            }
        })
        .collect();
    lanes.sort_by_key(|l| {
        std::cmp::Reverse(l.outs.iter().map(|(_, n)| n).sum::<usize>() + l.running.len())
    });
    // Keys follow the drawn order, so highlight rules stay simple.
    for (g, l) in lanes.iter_mut().enumerate() {
        l.key = format!("g-{g}");
    }
    lanes
}

/// Shared by every map on the page.
struct Ctx<'a> {
    checks: &'a [&'a str],
    more_checks: usize,
    /// Agents whose latest receipt is verified / failed / unverified (all agents, drawn or not).
    counts: [usize; 3],
    running: usize,
}

/// One map (`kind` is `each` or `tool`), its HTML and highlight rules.
fn map(kind: &str, lanes: &[Lane], cx: &Ctx, hidden: bool) -> (String, String) {
    let n = lanes.len().max(1);
    let gate_h = GATE_TOP + cx.checks.len().max(1) * CHECK_ROW + 24;
    let height = (n * ROW - (ROW - NODE)).max(gate_h).max(220);
    let h = height as f64;
    let node_y = |i: usize| (i * ROW + NODE / 2) as f64;
    let entry_y = |i: usize| {
        let span = h - GATE_TOP as f64 - 14.0;
        GATE_TOP as f64 + span * (2 * i + 1) as f64 / (2 * n) as f64
    };
    let out_y = |v: Verdict| h * outcome_share(v);
    let xs = |l: &Lane| -> String { l.fails.iter().map(|k| format!(" x-{k}")).collect() };

    let mut html = String::new();
    let _ = write!(
        html,
        "<div class=\"fl m-{kind}\"{} style=\"height:{height}px\"><div class=\"fl-col fl-agents\">",
        if hidden { " data-alt" } else { "" }
    );
    for (i, l) in lanes.iter().enumerate() {
        let dots: String = l
            .dots
            .iter()
            .map(|v| format!("<i class=\"d {}\"></i>", verdict_class(*v)))
            .collect();
        let state = match (l.running.is_empty(), l.outs.as_slice()) {
            (false, _) => "checking".to_string(),
            (true, [(v, _)]) => verdict_class(*v).to_string(),
            _ => "mixed".to_string(),
        };
        let (open, close) = match l.filter {
            Some(f) => (format!("<label for=\"fa-{f}\""), "</label>"),
            None => ("<div".to_string(), "</div>"),
        };
        let _ = write!(
            html,
            "{open} class=\"fn {key}{x} {state}\" style=\"top:{top}px\">{av}\
             <span class=\"fn-t\"><b>{name}</b><small>{sub}</small></span>\
             <span class=\"fn-d\" title=\"latest runs, oldest first\">{dots}</span>{close}",
            key = l.key,
            x = xs(l),
            top = i * ROW,
            av = avatar(&l.name),
            name = escape(&l.name),
            sub = l.sub,
        );
    }
    html.push_str("</div>");

    // Lines into Countercheck; a run in progress adds a moving line on top.
    let _ = write!(
        html,
        "<svg class=\"fl-e fl-l\" viewBox=\"0 0 100 {height}\" preserveAspectRatio=\"none\" aria-hidden=\"true\">"
    );
    for (i, l) in lanes.iter().enumerate() {
        let (y, ey) = (node_y(i), entry_y(i));
        let d = format!("M0 {y:.1} C55 {y:.1} 45 {ey:.1} 100 {ey:.1}");
        let cls = match l.outs.as_slice() {
            [] => None,
            [(v, _)] => Some(verdict_class(*v)),
            _ => Some("mixed"),
        };
        if let Some(cls) = cls {
            let _ = write!(
                html,
                "<path class=\"fe {key}{x} {cls}\" style=\"--d:{dl:.2}s\" vector-effect=\"non-scaling-stroke\" d=\"{d}\"/>",
                key = l.key,
                x = xs(l),
                dl = 0.1 + i as f64 * 0.06,
            );
        }
        if !l.running.is_empty() {
            let _ = write!(
                html,
                "<path class=\"fe-run {key}{x}\" vector-effect=\"non-scaling-stroke\" d=\"{d}\"/>",
                key = l.key,
                x = xs(l),
            );
        }
    }
    html.push_str("</svg>");

    // The Countercheck box: the checks it ran on the latest claims.
    html.push_str(
        "<div class=\"fl-col fl-gate\"><div class=\"fg\"><b>Countercheck</b>\
         <small>checks locked at the base commit</small>",
    );
    if cx.running > 0 {
        let _ = write!(
            html,
            "<small class=\"fg-run\">{} checking now</small>",
            cx.running
        );
    }
    html.push_str("<ul>");
    for (k, name) in cx.checks.iter().enumerate() {
        let f: String = lanes
            .iter()
            .filter(|l| l.fails.contains(&k))
            .map(|l| format!(" f-{}", l.key))
            .collect();
        let _ = write!(
            html,
            "<li class=\"fc c-{k}{f}\" title=\"{full}\">{label}</li>",
            full = escape(name),
            label = escape(&check_label(name)),
        );
    }
    if cx.checks.is_empty() {
        html.push_str("<li class=\"fc none\">no checks ran yet</li>");
    }
    if cx.more_checks > 0 {
        let _ = write!(
            html,
            "<li class=\"fc more\">and {} more</li>",
            cx.more_checks
        );
    }
    html.push_str("</ul></div></div>");

    // Lines out to the outcomes (wider for more agents); a light runs along each delivered one.
    let _ = write!(
        html,
        "<svg class=\"fl-e fl-r\" viewBox=\"0 0 100 {height}\" preserveAspectRatio=\"none\" aria-hidden=\"true\">"
    );
    for (i, l) in lanes.iter().enumerate() {
        let ey = entry_y(i);
        for (v, count) in &l.outs {
            let oy = out_y(*v);
            let d = format!("M0 {ey:.1} C55 {ey:.1} 45 {oy:.1} 100 {oy:.1}");
            let width = 2.2 + (count.saturating_sub(1).min(6) as f64) * 0.9;
            let _ = write!(
                html,
                "<path class=\"fe {key}{x} {cls}\" style=\"--d:{dl:.2}s;stroke-width:{width:.1}px\" vector-effect=\"non-scaling-stroke\" d=\"{d}\"/>",
                key = l.key,
                x = xs(l),
                cls = verdict_class(*v),
                dl = 0.75 + i as f64 * 0.06,
            );
            if *v == Verdict::Verified {
                let _ = write!(
                    html,
                    "<path class=\"pulse {key}{x}\" style=\"--d:{dl:.2}s\" vector-effect=\"non-scaling-stroke\" d=\"{d}\"/>",
                    key = l.key,
                    x = xs(l),
                    dl = 1.7 + i as f64 * 0.12,
                );
            }
        }
    }
    html.push_str("</svg><div class=\"fl-col fl-out\">");
    for (k, (v, title, sub)) in [
        (
            Verdict::Verified,
            "Delivered",
            "passed checks it couldn't change",
        ),
        (Verdict::Failed, "Sent back", "the agent has to fix it"),
        (
            Verdict::Unverified,
            "Needs a person",
            "nothing proven either way",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let _ = write!(
            html,
            "<div class=\"fo {cls}\" style=\"top:{top:.0}px\"><b class=\"fo-n\">{count}</b>\
             <span><b>{title}</b><small>{sub}</small></span></div>",
            cls = verdict_class(v),
            top = (out_y(v) - 22.0).max(0.0),
            count = cx.counts.get(k).copied().unwrap_or(0),
        );
    }
    html.push_str("</div></div>");

    // Highlights, scoped to this map: a lane lights its path and the checks it failed; a check
    // lights the lanes that failed it; a picked agent (the filter) stays lit.
    let p = format!(".fl.m-{kind}");
    let all = ":is(.fe,.fe-run,.fn,.pulse)";
    let dim = "{opacity:.12}";
    let bad = "{color:var(--fl-bad);background:var(--fl-bad-bg)}";
    let mut css = String::new();
    for l in lanes {
        let k = &l.key;
        let _ = write!(
            css,
            "{p}:has(.fn.{k}:hover) {all}:not(.{k}){dim}{p}:has(.fn.{k}:hover) .fc.f-{k}{bad}"
        );
        if let Some(f) = l.filter {
            let _ = write!(
                css,
                "body:has(#fa-{f}:checked) {p} {all}:not(.{k}){dim}body:has(#fa-{f}:checked) {p} .fc.f-{k}{bad}"
            );
        }
    }
    for k in 0..cx.checks.len() {
        let _ = write!(css, "{p}:has(.fc.c-{k}:hover) {all}:not(.x-{k}){dim}");
    }
    (html, css)
}

/// The map for these agents (newest activity first) and runs in progress, or nothing when there
/// are neither.
pub fn flow(agents: &[Agent], running: &[Progress]) -> Option<Flow> {
    if agents.is_empty() && running.is_empty() {
        return None;
    }
    // The checks of the agents' latest runs, in the order they first appear.
    let mut all_checks: Vec<&str> = Vec::new();
    for a in agents {
        for c in a
            .runs
            .first()
            .map(|e| &e.receipt.checks)
            .into_iter()
            .flatten()
        {
            if !all_checks.contains(&c.name.as_str()) {
                all_checks.push(&c.name);
            }
        }
    }
    let checks: Vec<&str> = all_checks.iter().take(MAX_FLOW_CHECKS).copied().collect();
    let count = |v: Verdict| {
        agents
            .iter()
            .filter(|a| a.runs.first().is_some_and(|e| e.receipt.verdict == v))
            .count()
    };
    let cx = Ctx {
        checks: &checks,
        more_checks: all_checks.len() - checks.len(),
        counts: [
            count(Verdict::Verified),
            count(Verdict::Failed),
            count(Verdict::Unverified),
        ],
        running: running.len(),
    };
    let mut named: Vec<String> = agents.iter().map(|a| a.name.clone()).collect();
    for p in running {
        let n = running_name(p);
        if !named.contains(&n) {
            named.push(n);
        }
    }
    let grouped = named.len() > MAX_FLOW_AGENTS;

    let mut html = String::from(
        "<section class=\"flow\" aria-label=\"Each agent's latest claim and what happened to it\">\
         <div class=\"fl-in\"><p class=\"eyebrow\">Flow</p><h2>From <q>done</q> to delivered</h2>\
         <p class=\"fl-legend\"><span class=\"lg verified\">verified, delivered</span>\
         <span class=\"lg failed\">failed, sent back</span>\
         <span class=\"lg unverified\">unverified, needs a person</span>",
    );
    if !running.is_empty() {
        html.push_str("<span class=\"lg checking\">checking now</span>");
    }
    html.push_str(
        "<span class=\"hint\">Point at an agent or a check; pick an agent to see only its runs.</span></p>",
    );
    let mut css = String::new();
    if grouped {
        let tools = tool_lanes(agents, running, &checks);
        let each = agent_lanes(agents, running, &checks, MAX_AGENTS);
        let _ = write!(
            html,
            "<div class=\"seg fl-view\" role=\"group\" aria-label=\"Show\">\
             <input type=\"radio\" name=\"flv\" id=\"flv-tool\" checked><label for=\"flv-tool\">By tool <span>{}</span></label>\
             <input type=\"radio\" name=\"flv\" id=\"flv-each\"><label for=\"flv-each\">Each agent <span>{}</span></label></div>",
            tools.len(),
            named.len()
        );
        for (kind, lanes, hidden) in [("tool", &tools, false), ("each", &each, true)] {
            let (h, c) = map(kind, lanes, &cx, hidden);
            html.push_str(&h);
            css.push_str(&c);
        }
        if named.len() > each.len() {
            let _ = write!(
                html,
                "<p class=\"fl-more m-each\">Each agent: the {} most recent; {} more are in the Agents panel below</p>",
                each.len(),
                named.len() - each.len()
            );
        }
    } else {
        let lanes = agent_lanes(agents, running, &checks, MAX_FLOW_AGENTS);
        let (h, c) = map("each", &lanes, &cx, false);
        html.push_str(&h);
        css.push_str(&c);
    }
    html.push_str("</div></section>");
    Some(Flow { html, css })
}

/// Runs in progress as a list: who, what they claimed, which step they are on. Display only.
pub fn running_html(running: &[Progress], live: bool) -> String {
    if running.is_empty() {
        return String::new();
    }
    let mut h = format!(
        "<section class=\"running\"><p class=\"eyebrow\">Checking now</p><h2>{} run{} in progress</h2>\
         <p class=\"muted hint\">{} Not receipts: a verdict exists only once the receipt is sealed.</p>",
        running.len(),
        if running.len() == 1 { "" } else { "s" },
        if live {
            "Updates by itself."
        } else {
            "As they were when this page was made."
        }
    );
    for p in running {
        let name = running_name(p);
        let mut who = vec![escape(&name)];
        if p.worker.role.is_some() {
            who.extend(p.worker.agent.as_deref().map(escape));
        }
        who.extend(p.worker.model.as_deref().map(escape));
        let step = p.stage.step();
        let segs: String = (1..=6)
            .map(|s| {
                let cls = match s.cmp(&step) {
                    std::cmp::Ordering::Less => "done",
                    std::cmp::Ordering::Equal => "now",
                    std::cmp::Ordering::Greater => "",
                };
                format!("<i class=\"{cls}\"></i>")
            })
            .collect();
        let _ = write!(
            h,
            "<div class=\"rn\">{av}<span class=\"rn-main\"><b>{who}</b><q>{claim}</q></span>\
             <span class=\"rn-prog\"><span class=\"rn-segs\" aria-hidden=\"true\">{segs}</span>\
             <small>{status} · started {at}</small></span></div>",
            av = avatar(&name),
            who = who.join(" · "),
            claim = escape(p.claim.as_deref().unwrap_or("(no claim)")),
            status = escape(&running_status(p)),
            at = escape(&super::pretty_time(&p.started_at)),
        );
    }
    h.push_str("</section>");
    h
}
