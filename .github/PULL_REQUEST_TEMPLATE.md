<!-- Base branch: DEV (GitHub suggests PROD, the release branch; please switch it). -->

## What and why

<!-- What changes, and the problem it solves. Link the issue if there is one. -->

## How I tested it

<!-- Commands you ran, and the new test(s). A behaviour change needs a test that fails without it. -->

## Checklist

- [ ] Against `DEV`, one topic
- [ ] The gate passes: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `python3 eval/validate.py`
- [ ] Docs updated (`docs/checks.md`, `docs/hooks/`, README) if users see the change
- [ ] A line under "Unreleased" in `CHANGELOG.md` if users notice it
- [ ] Doesn't weaken a rule in `docs/architecture.md` ("The mindset"), or it was discussed in an issue first
- [ ] Every commit is signed off (`git commit -s`)
