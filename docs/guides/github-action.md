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
      - uses: datacrate360/agent-countercheck@v0.21.0
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
| `comment` | `false` | `true`: post the verdict on the pull request and keep that one comment updated ([below](#post-the-verdict-on-the-pr)) |

It runs on Linux, macOS and Windows runners. The receipt names the sandbox that ran; on hosted Linux runners that is
usually Landlock with write confinement (whether the internet is blocked depends on the runner allowing user
namespaces, and the receipt says which).

## Post the verdict on the PR

With `comment: true`, the verdict also goes in the pull request's conversation: one comment, updated on every
later run (never one per push). It has the verdict, the claim and whether the checks support it, each check with
its result and time, the reasons, the protected files put back, the receipt id, and a link to the run; at most 60
lines, with only the last lines of a failing check's output.

```yaml
on: pull_request
permissions:
  contents: read
  pull-requests: write      # to post and update the comment
jobs:
  verify:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: datacrate360/agent-countercheck@v0.21.0
        with:
          comment: true
```

- **Forks:** on a pull request from a fork, GitHub gives the job a read-only token, so the comment can't be posted.
  That is a warning in the log and the job summary, never a failed step, and the verdict is the same as without it.
  Don't switch to `pull_request_target` to get a write token (see "Security").
- Any API error (a missing permission, a rate limit) is a warning too. The comment never changes the verdict or
  whether the step fails (`fail-on` works the same).
- The comment is posted as `github-actions[bot]` with the job's token. Only the step that posts gets the token;
  the step that runs the checks never sees it. It finds its earlier comment by a hidden marker and that author, so
  a comment someone else writes with the marker is left alone.
- Text from the pull request (its title, check names and output) is shown as data: HTML and links are escaped,
  `@` mentions don't ping anyone, and output sits in a code block it can't close.
- It needs `gh` (on GitHub-hosted runners) and Python on the runner.

## Security

- **Use `on: pull_request`, never `pull_request_target`** for this workflow. A PR's checks run the PR's code, as
  any CI does; `pull_request_target` would run it with your repository's secrets and write token.
- The action needs no token and only `contents: read` (plus `pull-requests: write` if you turn on `comment`).
- Inputs reach the shell through environment variables, so a PR title can't inject commands.
- Pin the action to a release tag (or a full commit id), as for any action.
