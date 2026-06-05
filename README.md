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

A typical loop:

1. **`jj-stack get <target>`** to fetch an existing stack, or skip this when
   starting fresh on top of trunk.
2. **`jj new ...`** to create your change, then make your edits.
3. **`jj-stack submit`** to push your stack as pull requests.
4. **`jj-stack sync`** to pull in the latest changes and rebase onto main as
   work lands around you.
5. **`jj-stack merge`** to land your stack.

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

Things you no longer have to deal with compared to Graphite:

- **No slow merge queues** — changes merge directly.
- **Less impact from GitHub API rate limits.**
- **Keep working when GitHub is down** — local jj operations don't depend on it.

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE).
