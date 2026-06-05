# Real jj/git test migration

## Goal

Tests must never mock `jj` or `git`. Every command-flow test should run against a
real colocated `jj` repository backed by a real bare `git` remote. The only
process tests may fake is `gh`, because CI cannot create real GitHub pull
requests.

## Current state

Done:

- `AGENTS.md` now documents the rule: real `jj`, real `git`, fake `gh` only.
- `tests/common/mod.rs` provides the shared real-repo harness and the single fake
  `gh` implementation.
- `tests/integration.rs` drives the real `jj-stack` binary against real
  colocated `jj` repos and real bare remotes.
- `src/main.rs` no longer contains the inline command-runner test module.
- `src/bin/ui-scenario.rs` no longer installs fake `jj` or `git`; it seeds a real
  repo and only writes a fake `gh`.

Verification target: search `src`, `tests`, and `AGENTS.md` for the removed
runner/helper names and fake `jj`/`git` environment markers. That search should
return no results. Mentions in historical docs are avoided so the check stays
easy to read.

## Test shape

Real command flows:

- Build a real stack with `jj new`, `jj describe`, real bookmarks, and real
  pushes.
- Assert observable state: remote refs, local bookmarks, mutability, SQLite cache
  rows, and fake-`gh` PR state.
- Do not assert that a specific `jj`/`git` argv was invoked.

Fake GitHub:

- The fake `gh` stores PR/comment metadata in JSON under the test temp directory.
- PR head/base oids and merged state are derived from the real remote via `git
  ls-remote` and `git merge-base --is-ancestor`.
- This keeps GitHub simulation tied to real repository state without mocking
  `jj` or `git`.

## Notes

- The old non-default secondary-workspace integration test was not kept as a full
  submit flow. With real tools, a jj secondary workspace is not colocated, and
  `jj-stack` needs real `git` commands for tree resolution. The behavior that
  mattered there was cache path resolution, not a command-flow test.
- Pure string/parser logic can still be tested directly in normal Rust tests if
  it is extracted behind a small library API. That is not process mocking.
