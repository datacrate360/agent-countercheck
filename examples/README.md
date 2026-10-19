# Examples

Each example is a small, made-up project in one field, with locked checks, a planted mistake the checks catch, and a
script that proves it (FAILED, then VERIFIED after the fix). Copy the one closest to your work.

| Example | Field | Check types it shows | Run |
|---|---|---|---|
| [data-migration](data-migration/) | data migration (legacy CSV + SQL → new format, per tenant) | matrix, golden (CSV), budget, forbid, coverage | `sh examples/data-migration/run-example.sh` |
| [ml-eval-threshold](ml-eval-threshold/) | machine learning (a toy classifier must keep held-out accuracy ≥ 0.85) | budget, golden (CSV), forbid, protected data | `sh examples/ml-eval-threshold/run-example.sh` |

For real projects, start from a [recipe](../recipes) instead (check files + helpers for one job).

**Add your field:** follow [.claude/skills/add-domain-example](../.claude/skills/add-domain-example/SKILL.md), or open a
"New domain" issue describing how "done" goes wrong in your work. Wanted: game assets, dbt models, web/API contracts, video and image pipelines, infrastructure as code.
