# Jujutsu Stack

Jujutsu Stack is a small, low-intrusive Rust CLI for a Jujutsu-native stacked PR
workflow, inspired by [Graphite](https://graphite.dev). It assumes your jj
changes form a single bottom-to-top stack, where each change becomes one pull
request.

- **Fast:** get out of the developer's way with the bare minimum of waiting. Merges land directly, with no merge queues by design.
- **Jujutsu-friendly workflow:** focus on editing existing revisions, no extra bloat.
- **Collaboration:** built for clear collaboration. Freeze changes you don't own and keep editing your existing stacks safely.
- **GitHub-friendly:** print friendly information for navigating from GitHub.

## Prerequisites

- [Jujutsu (`jj`)](https://github.com/jj-vcs/jj) installed and a colocated repo.
- [GitHub CLI (`gh`)](https://cli.github.com) installed and authenticated
  (`gh auth login`).

## Install

Install from Git:

```text
cargo install --git https://github.com/rivet-dev/jj-stack.git
```

## Quickstart

The core loop:

1. **`jj new`** to start a new change on top of your stack.
2. **Make your edits**: jj tracks the working copy automatically.
3. **`jj describe`** to set the change's message.
4. **`jj-stack submit`** to push your stack as pull requests.
5. **`jj-stack merge`** to land your stack.

Two other commonly used commands:

- **`jj-stack get <target>`** to fetch an existing stack to work on.
- **`jj-stack sync`** to pull in the latest changes and rebase onto main as work
  lands around you.

## Coming from Graphite

If you already know the Graphite CLI (`gt`), here is how its commands map to
jj and Jujutsu Stack:

| Graphite (`gt`)          | jj / Jujutsu Stack |
| ------------------------ | ------------------ |
| `gt create`              | `jj new`           |
| `gt checkout`            | `jj edit`          |
| `gt move` / `gt restack` | `jj rebase`        |
| `gt modify -m`           | `jj describe`      |
| `gt get`                 | `jj-stack get`     |
| `gt sync`                | `jj-stack sync`    |
| `gt submit`              | `jj-stack submit`  |
| `gt merge`               | `jj-stack merge`   |

**What you gain:**

- **Instant merges:** stacks land directly by fast-forwarding trunk, with no merge queue to wait on.
- **Automatic restacks:** jj rebases your whole stack for you; no manual `gt restack` after every change.
- **Fast, conflict-free rebases:** jj records conflicts in the commit instead of halting the rebase, so restacks always finish and you resolve on your own time.
- **Edit in place:** edit any revision directly with `jj edit`; descendants restack automatically.
- **Undo anything:** `jj undo` and the operation log reverse any command, including merges and rebases.
- **Open source:** no proprietary SaaS, no login, no per-seat billing.

**What you give up:**

- **Merge queues:** there is no hosted, stack-aware merge queue; merges land locally by fast-forward.
- **The Graphite dashboard:** no web UI for browsing or reviewing stacks; you navigate from GitHub instead.

## Fundamentals

### Jujutsu fundamentals

You build and edit stacks with plain jj. Jujutsu Stack never replaces these.

**`jj new <rev>`**
Start a new change on top of `<rev>`.

**`jj edit <rev>`**
Move into an existing revision to edit it.

**`jj rebase ...`**
Restructure or move changes.

**`jj describe <rev>`**
Set a change's message.

### Jujutsu Stack

**`jj-stack get <target>`**
Get or fetch an existing stack locally. The target can be a PR number (`123`), a
PR URL, an exact PR head branch, or a jj change-id prefix embedded in a stack
branch.

**`jj-stack sync`**
Fetch the latest changes and rebase onto main.

**`jj-stack submit`**
Push your changes as pull requests.

**`jj-stack merge`**
Merge your changes, starting from the current rev.

**`jj-stack pr`**
Open the current PR in your browser.

### Freezing

Freezing prevents you from clobbering other users' changes. You cannot edit
revisions owned by other users, and imported or collaborator-owned changes are
frozen automatically.

**`jj-stack freeze`**
Manually freeze a revision.

**`jj-stack unfreeze`**
Manually take ownership of a frozen revision you can push to.

## Related tools

Jujutsu Stack is one of several tools for turning a jj stack into pull requests.
Here is how the jj-native tools compare:

- **Jujutsu Stack**: lightweight, Graphite-style workflow.
  - **Workflow:** `jj` new/edit/describe → `jj-stack submit` → `jj-stack merge` (plus `get`/`sync` to pull and rebase others' stacks).
  - **Speed:** fast; merges locally with no queue or CI gate.
  - **Collaboration:** freezes revisions you don't own so you can't clobber them.
  - **Multi-PR support:** merges the whole stack in one command.
  - **Merge:** from the CLI, by fast-forwarding trunk.
  - **Auth:** `gh` CLI, no token.
- **[jj-spr](https://github.com/jennings/jj-spr)**: amend PRs without force-pushes (Jujutsu port of spr).
  - **Workflow:** amend a commit → `jj spr update` → `jj spr land`.
  - **Speed:** fast; local squash-merge.
  - **Collaboration:** none.
  - **Multi-PR support:** one PR at a time, with a manual rebase after each land.
  - **Merge:** from the CLI, squash via the GitHub API.
  - **Auth:** access token with `repo` scope.
- **[jjpr](https://github.com/michaeldhopkins/jjpr)**: multi-forge, automated merging.
  - **Workflow:** set bookmarks → `jjpr watch` opens draft PRs → promotes on passing CI → merges bottom-up.
  - **Speed:** medium; waits for CI to pass before merging.
  - **Collaboration:** can target your PR onto a coworker's unmerged branch.
  - **Multi-PR support:** merges the stack bottom-up.
  - **Merge:** from the CLI, via the forge API.
  - **Auth:** `gh`/`glab` credentials or a token.
- **[jj-ryu](https://github.com/dmmulroy/jj-ryu)**: chained PRs on GitHub and GitLab.
  - **Workflow:** create named bookmarks → `ryu submit` → `ryu sync`.
  - **Speed:** submitting is fast; you merge by hand.
  - **Collaboration:** none.
  - **Multi-PR support:** no merge command; merge each PR yourself.
  - **Merge:** in the GitHub web UI.
  - **Auth:** `gh`/`glab` credentials or a token.
- **[jj-vine](https://codeberg.org/abrenneke/jj-vine)**: flexible, no fixed workflow.
  - **Workflow:** push bookmarks → `jj-vine submit` opens or updates PRs with a stack diagram.
  - **Speed:** submitting is fast; you merge by hand.
  - **Collaboration:** none.
  - **Multi-PR support:** none yet; merging is not built.
  - **Merge:** in the GitHub web UI.
  - **Auth:** access token in config.
- **[keanemind/jj-stack](https://github.com/keanemind/jj-stack)**: turn a jj stack into PRs.
  - **Workflow:** name a bookmark per change → `jst submit` → merge the bottom PR → rerun to retarget.
  - **Speed:** submitting is fast; you merge by hand.
  - **Collaboration:** none.
  - **Multi-PR support:** one PR at a time; merge the bottom, then rerun.
  - **Merge:** in the GitHub web UI.
  - **Auth:** `gh` CLI or a token.

In the Git world, the analogous tools are
[Graphite](https://graphite.dev) (a proprietary SaaS that merges through a
hosted, stack-aware merge queue and supports shared stacks, billed per seat),
[spr](https://github.com/ejoffe/spr) (an open-source Go CLI that merges locally,
one commit per PR), and [ghstack](https://github.com/ezyang/ghstack) (an
open-source Python CLI whose branch layout means PRs can't be merged from the
GitHub UI).

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE).
