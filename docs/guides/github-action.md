# GitHub Action: verify every pull request

The action runs `countercheck verify` on a pull request: the checks come from the **base** commit (the PR can't
weaken them), protected files are put back from the base, and the result is a verdict, a job summary and a sealed
receipt saved as a workflow artifact.

```yaml
# .github/workflows/countercheck.yml
name: Countercheck
on: pull_request            # not pull_request_target: see "Security" below
permissions:
  contents: read
jobs:
  verify:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      # Install what your checks need first (Python, Node, a database...), as in your normal CI.
      - uses: datacrate360/agent-countercheck@v0.10.0
        with:
          agent: claude-code   # optional: recorded on the receipt
```

Your repository needs a committed `.countercheck/checks.toml` on the base branch (`countercheck init` writes a starter).

## What you get

- **The step's result.** By default it fails unless the verdict is VERIFIED; mark the job as required in branch
  protection to block merging.
- **A job summary** on the run's page: the verdict, the reasons, each check with its result and time, any protected
  file the PR changed, and the output of failing checks.
- **The receipt** (JSON) and an offline HTML report, as the artifact `countercheck-receipt-<id>`.
- **Outputs** for later steps: `verdict` (`verified`, `failed`, `unverified` or `error`), `receipt-id`,
  `receipt-path`.

## Inputs

| Input | Default | What it does |
|---|---|---|
| `base` | the PR's base commit; on a push, the commit before it | The revision before the change |
| `claim` | the PR's title | What the agent said it did (recorded; the verdict comes only from the checks) |
| `agent`, `model` | empty | Recorded on the receipt |
| `path` | `.` | The repository to verify |
| `sandbox` | `auto` | `auto`, `seatbelt`, `landlock`, `windows` or `none` |
| `checks-from`, `checks-sha256` | empty | Checks kept outside the repository ([how](../checks.md)) |
| `fail-on` | `unverified` | `unverified`: fail unless VERIFIED. `failed`: fail only on FAILED or an error. `never`: report only |
| `version` | this action's tag, else the latest | The countercheck version to install (checksum-verified) |
| `upload-receipt` | `true` | Upload the receipt and report as an artifact |

It runs on Linux, macOS and Windows runners. The receipt names the sandbox that ran; on hosted Linux runners that is
usually Landlock with write confinement (whether the internet is blocked depends on the runner allowing user
namespaces, and the receipt says which).

## Security

- **Use `on: pull_request`, never `pull_request_target`** for this workflow. A PR's checks run the PR's code, as
  any CI does; `pull_request_target` would run it with your repository's secrets and write token.
- The action needs no token and only `contents: read`.
- Inputs reach the shell through environment variables, so a PR title can't inject commands.
- Pin the action to a release tag (or a full commit id), as for any action.
