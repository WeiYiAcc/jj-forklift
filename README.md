# jj-stack

`jj-stack` is a small Rust CLI for managing a jj-native stacked PR workflow. It
looks at the current linear jj stack, creates or updates one GitHub pull request
per jj change through `gh`, and records repo-private metadata keyed by jj change
ID.

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) and
[NOTICE](NOTICE).

## Quickstart

Install from Git:

```text
cargo install --git https://github.com/rivet-dev/jj-stack.git
```

Then run a dry run from a jj repo with GitHub CLI (`gh`) authenticated:

```text
jj-stack submit --dry-run
```

## Workflow Overview

The workflow assumes your jj changes form a single bottom-to-top stack on top of
trunk or on top of an imported frozen dependency. `jj-stack submit` publishes
your mutable changes to GitHub, `jj-stack get` imports an existing GitHub PR
stack locally, `jj-stack sync` refreshes trunk and frozen dependencies,
`jj-stack status` explains what the CLI would manage next, and `jj-stack merge`
lands your owned stack one PR at a time from the bottom upward.

The MVP own-stack loop is:

```text
jj-stack submit --dry-run
jj-stack submit
# edit or reorder your jj changes
jj-stack status
jj-stack submit
jj-stack sync --no-submit
jj-stack sync
jj-stack merge
```

`submit --dry-run` shows the pushes, PR creates/updates, and comment updates it
would make. After submit, your PR head branches are normal tracked jj bookmarks,
so your own submitted revisions stay editable.

For collaboration, import a stack and build on top of its frozen tip:

```text
jj-stack get https://github.com/OWNER/REPO/pull/123
jj new jj-stack/frozen/pr-123
# make your own jj changes
jj-stack status
jj-stack submit
```

Imported PRs are represented by `jj-stack/frozen/pr-*` bookmarks and are not
rewritten by submit. When collaborators update or merge those PRs, run
`jj-stack sync` to move the frozen bookmarks and rebase your owned changes. If
you need to take over a same-repo PR branch you can push to, use
`jj-stack unfreeze <pr-number|url|branch|change-prefix>`.

For a tiny stack:

```text
main <- A <- B <- C
```

`submit` opens PRs with these bases:

```text
A: head stack/<A-title-slug>-<A-change-prefix>, base main
B: head stack/<B-title-slug>-<B-change-prefix>, base stack/<A-title-slug>-<A-change-prefix>
C: head stack/<C-title-slug>-<C-change-prefix>, base stack/<B-title-slug>-<B-change-prefix>
```

## Commands

### `jj-stack submit`

Resolves the current stack with the default revset
`trunk()..@ & ~::(immutable_heads() | root()) & ~empty()`,
validates that it is a linear conflict-free chain, and computes deterministic
head branches as `stack/<title-slug>-<change-id-prefix>`.

The first PR targets trunk. Each child PR targets its parent change's head
branch. Changed heads are published by moving local jj bookmarks with
`jj bookmark set --allow-backwards` and pushing them with `jj git push
--bookmark`, so jj tracks the remote PR branches and your own submitted changes
stay editable. PRs are then created or updated through `gh api`. If the branch
and PR metadata already match the jj change, submit leaves them alone. Submit
also upserts a stack comment on each PR so reviewers can see the stack shape.

If your stack sits on frozen dependencies, submit uses the nearest frozen PR
branch as the base for the first owned PR, uses each previous owned PR as the
base for the next one, and never creates, updates, merges, or comments on frozen
dependency PRs.

### `jj-stack get <target>`

Imports an existing stack from a GitHub PR number, PR URL, exact PR head branch,
or at least 8 characters of the jj change ID embedded in a
`stack/<title-slug>-<change-id-prefix>` branch. `get` reads the stack comment,
fetches each PR head branch, creates local `jj-stack/frozen/pr-*` bookmarks for
the imported PRs, and writes repo-private cache hints. It prints the next
command, usually `jj new jj-stack/frozen/pr-<top-pr>`, so you can start local
work on top of the frozen imported stack.

Examples:

```text
jj-stack get 123
jj-stack get https://github.com/OWNER/REPO/pull/123
jj-stack get stack/fix-parser-changeabc
jj-stack get changeabc
```

### `jj-stack sync`

Fetches from the configured remote, moves local trunk forward only when it can
fast-forward to the remote trunk, rebases the stack root onto trunk, and then
runs `submit`. Use `--no-submit` to stop after the local fetch, trunk movement,
and rebase.

If local trunk and remote trunk diverged, sync stops before rebasing and prints
the relevant commit IDs.

Examples:

```text
jj-stack sync --no-submit
jj-stack sync
```

With frozen dependencies, sync fetches the dependency PR branches, fast-forwards
their frozen bookmarks when safe, blocks divergent collaborator rewrites, and
rebases your owned stack onto the updated dependency tip or trunk.

### `jj-stack status`

Shows the repo, config, startup alias state, owned PRs submit would manage,
frozen dependencies, bookmark tracking problems, first owned PR base, merge
blockers, and the suggested next command.

Examples:

```text
jj-stack status
jj-stack status --json
```

Use `--json` for scripts or CI checks that need the same information without
parsing human-readable output.

### `jj-stack unfreeze <target>`

Adopts a same-repo frozen PR branch you can push to. The target can be a PR
number, PR URL, exact head branch, or jj change-id prefix. `unfreeze` verifies
the frozen bookmark still matches GitHub, tracks the PR branch as a mutable jj
bookmark, removes the frozen bookmark, and records only cache hints.

Examples:

```text
jj-stack unfreeze 123
jj-stack unfreeze https://github.com/OWNER/REPO/pull/123
jj-stack unfreeze stack/fix-parser-changeabc
jj-stack unfreeze changeabc
```

### `jj-stack merge`

Checks each PR from the bottom of the stack upward, then lands it with direct
squash merge using:

```text
gh pr merge --squash --delete-branch --match-head-commit
```

After GitHub reports the PR merged, `merge` fetches trunk, moves local trunk
forward, abandons the landed jj change, rebases the remaining stack, and runs
`submit` again so the next PR has the correct base. A repo-private merge journal
lets `jj-stack merge --continue` finish local recovery after an interrupted merge.

By default, `merge` selects the mutable stack from trunk through `@`. You can
merge through a different target with `jj-stack merge <target>`, where the
target can be a PR number, PR URL, exact PR head branch, jj change-id prefix, or
local jj rev. PR-like targets resolve to the PR head; local rev targets resolve
to one jj commit.

If your stack depends on frozen PRs, merge blocks while any dependency PR is
still open. After dependencies merge, run `jj-stack sync`; merge only proceeds
once the bottom owned PR targets trunk.

## MVP Constraints

- Stacks must be linear and conflict-free.
- Old raw-pushed or `state.json`-backed workflow state is unsupported. `jj-stack`
  should fail clearly rather than guessing ownership from legacy data.
- New branch names are deterministic: `stack/<title-slug>-<change-id-prefix>` by
  default. Existing PR branches are preserved from repo-private cache when a
  title changes, so retitling an existing PR updates the PR title without
  renaming the branch. The change ID suffix keeps duplicate titles distinct.
- Workflow cache is repo-private under the backing `.jj/repo/stack/`
  directory. For the MVP this includes `cache.sqlite`, which stores local
  recovery, stack comment ids, and PR lookup hints; do not edit it by hand.
  Correctness comes from live jj bookmarks, tracked remote bookmarks, and GitHub
  PR data, not from cache contents.
- Merges use direct squash merge only. Merge queues, auto-merge, pending
  deployments, and admin-only merge paths are not supported.

## Testing

Normal tests use unit tests, fake-`gh` integration tests, and real local jj
repositories. They do not create network resources.

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
