//! The flow map at the top of the report: each agent's latest claim running into Countercheck
//! (the checks locked at the base commit) and out to what happened: delivered, sent back, or
//! left for a person.
//!
//! Text is HTML, escaped like the rest of the page; the lines are inline SVG built from numbers
//! only. Drawing in, the delivery pulse and the highlighting on hover are CSS, so the offline
//! report (which may run no script) gets all of it. The highlight rules are generated per agent
//! and per check, again from numbers only.

use super::{Agent, avatar, escape, matrix_parts, verdict_class};
use crate::receipt::Verdict;
use std::fmt::Write as _;

/// Agents drawn on the map; the rest are named in a note (all are in the Agents panel).
pub const MAX_FLOW_AGENTS: usize = 12;
/// Checks listed in the Countercheck box; the rest are counted.
pub const MAX_FLOW_CHECKS: usize = 10;

/// Row pitch and node height of the agent column (px).
const ROW: usize = 58;
const NODE: usize = 48;
/// Where lines may enter the Countercheck box (below its title), and one check row's height (px).
const GATE_TOP: usize = 70;
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

/// Where outcome `v` sits on the right, as a share of the map's height.
fn outcome_share(v: Verdict) -> f64 {
    match v {
        Verdict::Verified => 0.2,
        Verdict::Failed => 0.5,
        Verdict::Unverified => 0.8,
    }
}

/// The map for these agents (newest activity first), or nothing when there are none.
pub fn flow(agents: &[Agent]) -> Option<Flow> {
    let shown: Vec<&Agent> = agents.iter().take(MAX_FLOW_AGENTS).collect();
    let latest: Vec<_> = shown
        .iter()
        .map(|a| a.runs.first().map(|e| &e.receipt))
        .collect::<Option<Vec<_>>>()?;
    if latest.is_empty() {
        return None;
    }

    // The checks of the agents' latest runs, in the order they first appear.
    let mut all_checks: Vec<&str> = Vec::new();
    for r in &latest {
        for c in &r.checks {
            if !all_checks.contains(&c.name.as_str()) {
                all_checks.push(&c.name);
            }
        }
    }
    let checks: Vec<&str> = all_checks.iter().take(MAX_FLOW_CHECKS).copied().collect();
    // For each agent, which listed checks its latest run failed.
    let fails: Vec<Vec<usize>> = latest
        .iter()
        .map(|r| {
            checks
                .iter()
                .enumerate()
                .filter(|(_, name)| r.checks.iter().any(|c| c.name == **name && !c.passed))
                .map(|(k, _)| k)
                .collect()
        })
        .collect();

    let n = shown.len();
    let gate_h = GATE_TOP + checks.len().max(1) * CHECK_ROW + 24;
    let height = (n * ROW - (ROW - NODE)).max(gate_h).max(220);
    let h = height as f64;
    let node_y = |i: usize| (i * ROW + NODE / 2) as f64;
    let entry_y = |i: usize| {
        let span = h - GATE_TOP as f64 - 14.0;
        GATE_TOP as f64 + span * (2 * i + 1) as f64 / (2 * n) as f64
    };
    let out_y = |v: Verdict| h * outcome_share(v);

    let mut html = String::new();
    let _ = write!(
        html,
        "<section class=\"flow\" aria-label=\"Each agent's latest claim and what happened to it\">\
         <div class=\"fl-in\"><p class=\"eyebrow\">Flow</p><h2>From <q>done</q> to delivered</h2>\
         <p class=\"fl-legend\"><span class=\"lg verified\">verified, delivered</span>\
         <span class=\"lg failed\">failed, sent back</span>\
         <span class=\"lg unverified\">unverified, needs a person</span>\
         <span class=\"hint\">Point at an agent or a check; pick an agent to see only its runs.</span></p>\
         <div class=\"fl\" style=\"height:{height}px\"><div class=\"fl-col fl-agents\">"
    );

    // Agent nodes (labels for the agent filter, so a click picks that agent's runs).
    for (i, (a, r)) in shown.iter().zip(&latest).enumerate() {
        let mut sub: Vec<String> = a.tools.iter().map(|t| escape(t)).collect();
        sub.extend(r.worker.model.iter().map(|m| escape(m)));
        if sub.is_empty() {
            let k = a.runs.len();
            sub.push(format!("{k} run{}", if k == 1 { "" } else { "s" }));
        }
        let dots: String = a
            .runs
            .iter()
            .take(8)
            .rev()
            .map(|e| format!("<i class=\"d {}\"></i>", verdict_class(e.receipt.verdict)))
            .collect();
        let x: String = fails
            .get(i)
            .into_iter()
            .flatten()
            .map(|k| format!(" x-{k}"))
            .collect();
        let _ = write!(
            html,
            "<label class=\"fn a-{i}{x} {cls}\" for=\"fa-{i}\" style=\"top:{top}px\">{av}\
             <span class=\"fn-t\"><b>{name}</b><small>{sub}</small></span>\
             <span class=\"fn-d\" title=\"last runs, oldest first\">{dots}</span></label>",
            cls = verdict_class(r.verdict),
            top = i * ROW,
            av = avatar(&a.name),
            name = escape(&a.name),
            sub = sub.join(" · "),
        );
    }
    html.push_str("</div>");

    // Lines into Countercheck.
    let _ = write!(
        html,
        "<svg class=\"fl-e fl-l\" viewBox=\"0 0 100 {height}\" preserveAspectRatio=\"none\" aria-hidden=\"true\">"
    );
    for (i, r) in latest.iter().enumerate() {
        let (y, ey) = (node_y(i), entry_y(i));
        let x: String = fails
            .get(i)
            .into_iter()
            .flatten()
            .map(|k| format!(" x-{k}"))
            .collect();
        let _ = write!(
            html,
            "<path class=\"fe a-{i}{x} {cls}\" style=\"--d:{d:.2}s\" vector-effect=\"non-scaling-stroke\" \
             d=\"M0 {y:.1} C55 {y:.1} 45 {ey:.1} 100 {ey:.1}\"/>",
            cls = verdict_class(r.verdict),
            d = 0.1 + i as f64 * 0.06,
        );
    }
    html.push_str("</svg>");

    // The Countercheck box: the checks it ran on the latest claims.
    html.push_str(
        "<div class=\"fl-col fl-gate\"><div class=\"fg\"><b>Countercheck</b>\
         <small>checks locked at the base commit</small><ul>",
    );
    for (k, name) in checks.iter().enumerate() {
        let f: String = fails
            .iter()
            .enumerate()
            .filter(|(_, ks)| ks.contains(&k))
            .map(|(i, _)| format!(" f-{i}"))
            .collect();
        let _ = write!(
            html,
            "<li class=\"fc c-{k}{f}\" title=\"{full}\">{label}</li>",
            full = escape(name),
            label = escape(&check_label(name)),
        );
    }
    if checks.is_empty() {
        html.push_str("<li class=\"fc none\">no checks ran</li>");
    }
    let more = all_checks.len() - checks.len();
    if more > 0 {
        let _ = write!(html, "<li class=\"fc more\">and {more} more</li>");
    }
    html.push_str("</ul></div></div>");

    // Lines out to the outcomes; a light runs along each delivered one, once.
    let _ = write!(
        html,
        "<svg class=\"fl-e fl-r\" viewBox=\"0 0 100 {height}\" preserveAspectRatio=\"none\" aria-hidden=\"true\">"
    );
    for (i, r) in latest.iter().enumerate() {
        let (ey, oy) = (entry_y(i), out_y(r.verdict));
        let x: String = fails
            .get(i)
            .into_iter()
            .flatten()
            .map(|k| format!(" x-{k}"))
            .collect();
        let d = format!("M0 {ey:.1} C55 {ey:.1} 45 {oy:.1} 100 {oy:.1}");
        let _ = write!(
            html,
            "<path class=\"fe a-{i}{x} {cls}\" style=\"--d:{dl:.2}s\" vector-effect=\"non-scaling-stroke\" d=\"{d}\"/>",
            cls = verdict_class(r.verdict),
            dl = 0.75 + i as f64 * 0.06,
        );
        if r.verdict == Verdict::Verified {
            let _ = write!(
                html,
                "<path class=\"pulse a-{i}{x}\" style=\"--d:{dl:.2}s\" vector-effect=\"non-scaling-stroke\" d=\"{d}\"/>",
                dl = 1.7 + i as f64 * 0.12,
            );
        }
    }
    html.push_str("</svg><div class=\"fl-col fl-out\">");
    for (v, title, sub) in [
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
    ] {
        let count = agents
            .iter()
            .filter(|a| a.runs.first().is_some_and(|e| e.receipt.verdict == v))
            .count();
        let _ = write!(
            html,
            "<div class=\"fo {cls}\" style=\"top:{top:.0}px\"><b class=\"fo-n\">{count}</b>\
             <span><b>{title}</b><small>{sub}</small></span></div>",
            cls = verdict_class(v),
            top = (out_y(v) - 22.0).max(0.0),
        );
    }
    html.push_str("</div></div>");
    if agents.len() > n {
        let _ = write!(
            html,
            "<p class=\"fl-more\">and {} more agents: see the Agents panel below</p>",
            agents.len() - n
        );
    }
    html.push_str("</div></section>");

    // Highlights: an agent lights its path and the checks it failed; a check lights the agents it
    // failed. A picked agent (the filter) stays lit.
    let mut css = String::new();
    let dim = "{opacity:.12}";
    for i in 0..n {
        let _ = write!(
            css,
            ".fl:has(.fn.a-{i}:hover) :is(.fe,.fn,.pulse):not(.a-{i}),\
             body:has(#fa-{i}:checked) .fl :is(.fe,.fn,.pulse):not(.a-{i}){dim}\
             .fl:has(.fn.a-{i}:hover) .fc.f-{i},body:has(#fa-{i}:checked) .fc.f-{i}{{color:var(--fl-bad);background:var(--fl-bad-bg)}}"
        );
    }
    for k in 0..checks.len() {
        let _ = write!(
            css,
            ".fl:has(.fc.c-{k}:hover) :is(.fe,.fn,.pulse):not(.x-{k}){dim}"
        );
    }
    Some(Flow { html, css })
}
