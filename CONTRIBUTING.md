# Contributing

See [CLAUDE.md](CLAUDE.md) for working agreements, jj/version-control rules, and
Rust + testing conventions.

## Testing

Normal tests use unit tests, fake-`gh` integration tests, and real local jj
repositories. They do not create network resources. Never mock `jj` or `git` —
only `gh` may be faked. See [CLAUDE.md](CLAUDE.md) for the full testing policy.

The real GitHub E2E is opt-in:

```text
JJ_STACK_REAL_GITHUB_E2E=1 \
JJ_STACK_E2E_OWNER=<owner> \
JJ_STACK_E2E_REPO_PREFIX=jj-stack-e2e \
cargo test --test real_github -- --nocapture
```

It creates a disposable private repository with `gh repo create`, submits a
two-change stack, edits one submitted change, submits again, verifies the same
PRs were updated, attempts `jj-stack merge`, and deletes the repository with
`gh repo delete --yes`. Set `JJ_STACK_E2E_KEEP_REPO=1` to keep the disposable
repo and local temp dir for debugging. Cleanup requires the `delete_repo` scope:
`gh auth refresh -s delete_repo`.

## Options and Configuration

Global options:

- `--dry-run`: plan actions without mutating local cache, remote refs, jj, or
  GitHub.
- `--verbose`: print resolved config, stack details, planned mutations, external
  commands, and recovery phases.

Debug logs are written automatically to `.jj/repo/stack/logs/` when possible,
falling back to `$XDG_STATE_HOME/jj-stack/logs/`. `JJ_STACK_LOG` controls the
file log filter and defaults to `warn,jj_stack=debug`; set it to `trace` for a
full trace or `off` to disable file logging. Stderr tracing is opt-in via
`JJ_STACK_LOG_STDERR=1`, or set `JJ_STACK_LOG_STDERR` to a tracing filter.

Stack commands accept `--revset <REVSET>` to operate on a narrower jj stack than
the default `trunk()..@ & ~::(immutable_heads() | root()) & ~empty()`.

Configuration is read from `jj config`, then `git config`, then defaults:

- `stack.remote`: defaults to `origin`
- `stack.trunk`: defaults to `main`
- `stack.require-approval`: defaults to `true`
- `stack.branch-prefix`: defaults to `stack`

GitHub repository and username are resolved through the `gh` CLI.
</content>
</invoke>
