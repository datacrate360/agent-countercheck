# Benchmark blocked: claude-sonnet-5-5 (no results)

The pilot stopped on its first run, so no benchmark data was produced. **Spend so far: $0.00** (`api_calls: 0`).

## Done
- `cargo build --release` OK; `python3 eval/validate.py`: 29/29 tasks valid.
- Hermes Agent installed with the official script: `Hermes Agent v0.22.5 (2026.9.24)`.
- `python3 eval/setup_profile.py --provider anthropic --model claude-sonnet-5-5` OK.
- Sandbox check (`countercheck verify` on `n1`-style task copy), receipt field:
  `"sandbox": "linux-landlock (no internet, localhost only; writes only in the check folder)"`.
- `hermes -p countercheck-eval auth list` sees the key (`env:ANTHROPIC_API_KEY`).

## Blocker
`python3 eval/run.py --tasks pilot --arms A,C ...` fails on the first run (`n1_fizzbuzz A r1`) with, in `usage.json`:

> No Anthropic credentials found. Run 'hermes auth add anthropic' to sign in, or set ANTHROPIC_TOKEN / ANTHROPIC_API_KEY.

`ANTHROPIC_API_KEY` was exported in the same shell (checked by length only) and `hermes auth list` finds it there, but
the child `hermes -p countercheck-eval -z ... --provider anthropic --usage-file ... --yolo` launched by `eval/run.py`
does not. `run.py` then stops by design ("no cost recorded"). Reproduced twice. I did not find the cause without
changing code or writing the key to a file, both forbidden here.

Possible causes to check (not tried): Hermes reading credentials from the profile `.env` only for one-shot (`-z`)
runs; an interaction with `HERMES_ACCEPT_HOOKS` / `TERMINAL_CWD`; or the eval profile needing
`ANTHROPIC_API_KEY=` in `~/.hermes/profiles/countercheck-eval/.env` (the way `setup_profile.py` expects `DEEPSEEK_API_KEY`).

## Other difference from the October method
- `hermes hooks list` shows the Countercheck `pre_verify` hook as "not allowlisted" even with `--accept-hooks`;
  consent is only recorded by a live agent session. `run.py` sets `HERMES_ACCEPT_HOOKS=1` per run, so it should
  fire during runs, but this is unconfirmed. `hermes hooks test pre_verify` ran the hook fine (exit 0, 1.4 s).
- Hermes could not resolve the stable source release (HTTP 403) and installed from git `bf0203a794`.

No code under `crates/`, `apps/` or `eval/` was changed. Not run: pilot cost, full run, judge, report.
