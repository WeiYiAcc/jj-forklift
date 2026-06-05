# jj-stack Collaboration Flow

## TODO

The current priority is to finish removing authoritative local state. Repo-local
recovery data must live only in `.jj/repo/stack/cache.sqlite`; do not implement a
`cache.json` intermediate format, and do not read or migrate old
`.jj/repo/stack/state.json`.

The MVP milestone remains: one author can reliably submit, update, and merge
their own jj stack on GitHub. Live jj/GitHub discovery must provide correctness;
SQLite cache rows are only hints for speed, recovery, and comment ids.

## Current Priority Order

1. Finish the SQLite-only cache lane: remove any remaining `state.json`
   assumptions, keep cache reads optional, keep cache writes best-effort when
   they are only hints, and ensure dry-runs never write cache data.
2. Make submit/merge/get/unfreeze rely on live jj bookmarks, tracked remote
   bookmarks, and GitHub PR data as the source of truth, using SQLite only as a
   hint.
3. Complete the own-stack MVP: submit, update, sync, merge, README, local jj
   tests, and opt-in real GitHub E2E.
4. Continue collaborator import/frozen dependency flows after own-stack behavior
   is shippable.
5. Add `status` and broader collaboration polish once the workflow behavior is
   complete.

Each MVP item should be small enough to finish in one PR-sized slice. Every
feature slice needs at least one unit or fake-`gh` integration test, and every
jj behavior slice needs a real local jj integration test unless the behavior is
pure formatting/parsing.

## MVP Milestone: Submit, Update, And Merge Own Stack

### MVP 0. Real Test Harness

- [x] Add reusable integration helpers for a fresh local Git + jj repo.
- [x] Add reusable integration helpers for a fresh Git + jj repo with a bare
  remote.
- [x] Add a shared real-jj `TestRepo` helper that owns the temp dir, worktree
  path, remote path, fake-`gh` path, environment, command runner, and cleanup.
- [x] Add a helper to create a single described jj change with a deterministic
  file edit, title, body, commit id, and change id.
- [x] Add a helper to create a linear stack of N described jj changes.
- [x] Add helpers to inspect local bookmark target, tracked remote target, and
  mutability in a real jj repo.
- [x] Add fake GitHub/`gh` fixtures for PR create/view/edit/comment/list paths
  used by `jj-stack`.
- [x] Add fake GitHub/`gh` fixtures for the second-submit path: PR lookup by
  number, PR patch, stack comment lookup, and stack comment update.
- [x] Add a real local jj smoke test for the MVP harness itself.

### MVP 1. Submit Own Stack

- [x] Replace raw `git push <sha>:refs/heads/<branch>` with `jj bookmark set
  --allow-backwards <branch> -r <change>` plus `jj git push --remote <remote>
  --bookmark <branch>`.
- [x] Keep dry-run output mutation-free while showing bookmark set/push intent.
- [x] Preserve remote lease checks before updating existing PR branches.
- [x] Keep `cache.sqlite` for MVP PR lookup, recovery data, and stack comment
  metadata.
- [x] Fail clearly if the local bookmark is missing, untracked, conflicted, or
  points at the wrong revision.
- [x] Test new PR branch creation uses `jj git push --bookmark`.
- [x] Test updating an existing PR moves the local bookmark first.
- [x] Test dry-run does not create local bookmarks or push.
- [x] Test a submitted own branch stays mutable/editable after fetch.
- [x] Test current cache-backed submit saves created PR metadata before comment
  failure.
- [x] Verify the tracked remote bookmark still matches GitHub's live PR head SHA
  before pushing.
- [x] Fail clearly if a deterministic branch already exists but `cache.sqlite` does
  not identify it as an existing own PR.
- [x] Test remote branch exists without a matching safe PR fails before push.

### MVP 2. Update Existing Submitted Stack

- [x] Add fake-`gh` support for fetching an existing PR by number after the first
  submit.
- [x] Add fake-`gh` support for patching an existing PR title/body/base.
- [x] Add fake-`gh` support for finding and updating the existing stack comment.
- [x] Add a real local jj integration test that submits a change, edits it,
  submits again, and verifies the same branch/PR updates.
- [x] Add fake integration coverage for a two-change stack where the bottom PR is
  edited and the top PR base remains correct.
- [x] Ensure the second submit keeps the local revision mutable after fetch.

### MVP 3. Merge Own Stack

- [x] Merge only PRs in the current own stack recorded in `cache.sqlite`.
- [x] Require PRs to be open, non-draft, approved when configured, mergeable, and
  have passing status checks.
- [x] Merge from bottom to top.
- [x] After merging a PR, advance trunk/sync local jj state before evaluating the
  next PR.
- [x] Support dry-run merge output that lists the PRs that would be merged.
- [x] Fail before merging anything when one PR is not mergeable.
- [x] Test merge preflight blocks draft/unapproved/unmergeable/failing-check PRs.
- [x] Test clean own-stack merge with fake `gh`.
- [x] Test dry-run merge does not mutate GitHub.

### MVP 4. Real GitHub E2E

- [x] Add an opt-in real GitHub E2E test that creates a disposable repo with
  `gh repo create`.
- [x] In the disposable repo, initialize a fresh colocated Git+jj workspace.
- [x] Submit a two-change stack with real `jj-stack submit`.
- [x] Edit one submitted change and run `jj-stack submit` again.
- [x] Verify the same PRs/branches were updated, not duplicated.
- [x] Merge the stack when the disposable repo allows it. If GitHub refuses merge
  for environment reasons, verify submit/update against real GitHub and keep
  merge covered by fake-`gh`.
- [x] Always cleanup the disposable repo/branches on success or failure.
- [x] Document required env vars: `JJ_STACK_REAL_GITHUB_E2E`,
  `JJ_STACK_E2E_OWNER`, `JJ_STACK_E2E_REPO_PREFIX`, and optional debug/cleanup
  controls.

### MVP 5. README And Release Checks

- [x] Update the README with the MVP own-stack workflow: `submit --dry-run`,
  `submit`, edit/reorder, `submit`, `sync`, and `merge`.
- [x] Document branch naming and why submitted branch names are deterministic.
- [x] Document that `cache.sqlite` is MVP local recovery metadata and should not be
  edited by hand.
- [x] Run `cargo fmt`.
- [x] Run unit tests.
- [x] Run the normal integration test suite.
- [x] Run real local jj integration tests.
- [ ] Run the opt-in real GitHub E2E test before release.
- [x] Run `cargo install --path .` after CLI changes.

## Post-MVP Backlog

### 0. Test Harness

- [x] Add reusable integration helpers for a fresh local Git + jj repo.
- [x] Add reusable integration helpers for a fresh Git + jj repo with a bare
  remote.
- [x] Add a shared `TestRepo` helper that owns the temp dir, worktree path,
  remote path, environment, command runner, and cleanup.
- [x] Add a helper to create a single described jj change with a deterministic
  file edit, title, body, commit id, and change id.
- [x] Add a helper to create a linear stack of N described jj changes.
- [x] Add a helper to create sibling jj changes from the same parent.
- [x] Add a helper to create merge-parent history for rejection tests.
- [x] Add a helper to create empty jj changes for rejection tests.
- [x] Add a helper to create conflicted jj changes for rejection tests.
- [x] Add a helper to create local bookmarks, tracked remote bookmarks,
  untracked remote bookmarks, conflicted bookmarks, and divergent bookmarks.
- [x] Add helpers to inspect local bookmark target, tracked remote target,
  bookmark tracking state, and bookmark conflicts.
- [x] Add helpers to assert whether a revision is mutable or immutable in a real
  jj repo.
- [x] Add helpers to read repo-local jj config aliases and assert startup config
  idempotency.
- [x] Add fake GitHub/`gh` fixtures for PR create/view/edit/comment/list paths
  used by `jj-stack`.
- [x] Add fake GitHub fixtures for PR retarget, PR close/merge state, branch
  deletion, comment duplication, stale comment id, and API failure paths.
- [x] Add a fake GitHub fixture test that verifies every configured fake command
  was actually consumed.
- [x] Add a real local jj smoke test for the test harness itself.
- [x] Add an opt-in real GitHub E2E test that uses `gh` to create a disposable
  repo, submit a small stack, update it, sync it, and merge/cleanup it.
- [x] Document opt-in real GitHub E2E environment variables:
  `JJ_STACK_REAL_GITHUB_E2E`, `JJ_STACK_E2E_OWNER`,
  `JJ_STACK_E2E_REPO_PREFIX`, and optional cleanup/debug controls.
- [x] Add a CI/default-test guard proving the real GitHub E2E is skipped unless
  explicitly opted in.

### 1. Startup Config

- [x] Install `jj_stack_frozen_heads()` in repo-local jj config.
- [x] Install `immutable_heads() = builtin_immutable_heads() |
  jj_stack_frozen_heads()` when the repo uses the default immutable config.
- [x] Wrap custom `immutable_heads()` once with `jj_stack_base_immutable_heads()`
  when it is safe to do so.
- [x] Fail before mutations when the existing immutable config cannot be safely
  read or wrapped.
- [x] Test default config install.
- [x] Test idempotent repeated startup.
- [x] Test custom immutable config wrapping.
- [x] Test unsafe/unreadable config failure text.

### 2. Tracked Bookmark Submit Path

- [x] Replace raw `git push <sha>:refs/heads/<branch>` with `jj bookmark set
  --allow-backwards <branch> -r <change>` plus `jj git push --remote <remote> --bookmark
  <branch>`.
- [x] Keep dry-run output mutation-free while showing bookmark set/push intent.
- [x] Preserve remote lease checks before updating existing PR branches.
- [x] Stop recording local head bookmark ownership as authoritative state;
  derive it from jj bookmarks/tracked remotes and use cache only as a hint.
- [x] Replace cache-backed bookmark ownership checks with jj tracked bookmark
  checks plus GitHub PR lookup.
- [x] Discover the tracked local bookmark for an existing PR branch before using
  cache data.
- [x] Verify the local bookmark points at the selected jj change before pushing.
- [x] Verify the tracked remote bookmark has no conflict before pushing.
- [x] Verify the tracked remote bookmark still matches GitHub's live PR head SHA
  before pushing.
- [x] Fail clearly if a matching remote branch exists but does not correspond
  to a current GitHub PR that `jj-stack` can safely update.
- [x] Fail clearly if the local bookmark is missing, untracked, conflicted, or
  points at the wrong revision.
- [x] Test new PR branch creation uses `jj git push --bookmark`.
- [x] Test updating an existing PR moves the local bookmark first.
- [x] Test dry-run does not create local bookmarks or push.
- [x] Test a submitted own branch stays mutable/editable after fetch.
- [x] Test existing PR discovery from a tracked local bookmark with no cache.
- [x] Test cache branch mismatch is ignored when live tracked bookmark and
  GitHub PR agree.
- [x] Test remote branch exists without a matching safe PR fails before push.
- [x] Test remote advanced/lease mismatch fails before PR/comment updates.
- [x] Add a real local jj integration test for updating an existing submitted PR
  branch after editing the change.

### 3. Replace Authoritative State With SQLite Cache

- [x] Replace `.jj/repo/stack/state.json` with `.jj/repo/stack/cache.sqlite`.
- [x] Do not create or support `.jj/repo/stack/cache.json`.
- [x] Rename `StateStore` to a SQLite cache-specific type and remove ownership
  language from its API.
- [x] Add SQLite schema initialization for PR/cache metadata.
- [x] Remove cache fields whose only purpose is proving ownership or push
  authority.
- [x] Keep cache fields that speed up discovery: repo, jj change id, PR number,
  PR node id, head repo id/name, head branch, head SHA, base repo id/name, base
  branch, title, body, created date, and stack comment id.
- [x] Make cache loading optional: missing SQLite cache initializes on write and
  behaves as empty for read-only discovery.
- [x] Make malformed/unopenable SQLite cache non-blocking for live discovery:
  log/debug the failure and continue without cache hints.
- [x] Make old `state.json` ignored; no legacy migration path.
- [x] Use SQLite transactions for cache writes.
- [x] Make cache writes best-effort for non-critical hints, with clear debug
  output on write failure.
- [x] Ensure command correctness never depends on cache contents.
- [x] Treat every cache hit as a hint that must be revalidated against live
  GitHub PR data before use.
- [x] Fall back to live discovery when cache data is missing, stale, malformed,
  or points to a closed/deleted/mismatched PR.
- [x] Discover an existing PR for a change by checking tracked local bookmark,
  cache hint, deterministic branch, and GitHub open PR search.
- [x] Preserve existing PR branches by discovering the live PR branch instead of
  trusting cached branch names.
- [x] Use jj tracked remote bookmark state plus GitHub head SHA for lease
  safety.
- [x] Never use cache data as proof of branch ownership, mutability, adoption,
  or push authority.
- [x] Upsert stack comments by scanning GitHub comments when the cached comment
  id is missing, stale, duplicated, or authored by the wrong user.
- [x] Update dry-run output to say live discovery would run and cache writes are
  skipped.
- [x] Test SQLite cache schema round-trips.
- [x] Test missing cache loads as empty and does not create a file during
  read-only commands.
- [x] Test malformed/unopenable SQLite cache warns/debug-logs and does not block
  live discovery.
- [x] Test old `state.json` is ignored.
- [x] Test cache writes use SQLite transactions.
- [x] Test submit works with no cache.
- [x] Test submit works with stale cache by rediscovering the live PR.
- [x] Test submit preserves an existing PR branch discovered from GitHub when
  cache suggests a different branch.
- [x] Test submit refuses unsafe remote advancement even when cache says it is
  safe.
- [x] Test stack comments are updated by live comment scan when cache has no
  comment id.
- [x] Test stack comments are updated by live comment scan when cached comment id
  no longer exists.
- [x] Test dry-run does not create or update `cache.sqlite`.
- [x] Test current cache-backed submit saves created PR metadata before comment
  failure.
- [x] Test partial failure leaves a recoverable next command without relying on
  cache contents.

### 4. Stack Resolution

- [x] Resolve `trunk()` to exactly one commit before stack operations.
- [x] Resolve owned revisions with `trunk()..@ & ~::(immutable_heads() | root()) &
  ~empty()`.
- [x] Detect frozen dependencies from `jj-stack/frozen/pr-*` bookmarks only.
- [x] Find the nearest frozen boundary below the owned segment.
- [x] Find the full ordered frozen dependency list below the owned segment.
- [x] Reject multiple nearest frozen boundaries.
- [x] Reject non-linear owned stacks.
- [x] Reject sibling owned stacks.
- [x] Reject merge-parent owned revisions.
- [x] Reject empty owned revisions.
- [x] Reject conflicted owned revisions.
- [x] Reject conflicted frozen bookmarks.
- [x] Report the exact offending revision/bookmark in stack-shape errors.
- [x] Test a simple owned stack.
- [x] Test an owned stack on top of one frozen dependency.
- [x] Test an owned stack on top of multiple frozen dependencies.
- [x] Test non-linear owned stack rejection.
- [x] Test sibling owned stack rejection.
- [x] Test merge-parent rejection.
- [x] Test empty revision rejection.
- [x] Test conflicted revision rejection.
- [x] Test conflicted frozen bookmark rejection.
- [x] Test generic immutable history is not treated as a frozen dependency.

### 5. `jj-stack get`

- [x] Resolve target by PR number.
- [x] Resolve target by GitHub PR URL.
- [x] Resolve target by exact branch name.
- [x] Resolve target by jj change-id prefix.
- [x] Reject ambiguous or missing branch/change-id prefix matches with candidates.
- [x] Parse existing jj-stack comments to import the full ordered stack.
- [x] Import only the target PR when no valid stack comment exists.
- [x] Validate PR number, node id, state, title, body, author, and created date.
- [x] Validate every PR's head repo, head branch, and head SHA.
- [x] Validate every PR's base repo and base branch.
- [x] Validate stack order by PR base/head branch links.
- [x] Reject fork-backed stacked dependencies for MVP.
- [x] Fetch every PR head branch.
- [x] Create one `jj-stack/frozen/pr-<number>` bookmark per imported PR.
- [x] Move an existing frozen bookmark only when the old target is an ancestor
  of the new target.
- [x] Fail before moving any frozen bookmark if one import target is invalid.
- [x] Write cache hints only after fetch and frozen bookmark validation succeed.
- [x] Print the next `jj new jj-stack/frozen/pr-<top-pr>` command.
- [x] Test PR-number target.
- [x] Test PR-URL target.
- [x] Test exact-branch target.
- [x] Test change-id-prefix target.
- [x] Test ambiguous prefix rejection with candidates.
- [x] Test full-stack import from comment.
- [x] Test single-PR import without comment.
- [x] Test topology mismatch rejection.
- [x] Test fork-backed PR rejection.
- [x] Test divergent frozen bookmark movement rejection.
- [x] Test imported frozen revisions are immutable to normal jj edits.

### 6. `jj-stack sync`

- [x] Scope sync to frozen dependencies under the current owned stack.
- [x] Support syncing a purely frozen imported stack when `@` is frozen.
- [x] Fetch trunk/base before comparing GitHub state.
- [x] Re-read stack comments or validated PR base graph.
- [x] Fast-forward frozen bookmarks when collaborator PR heads advance.
- [x] Fail on divergent collaborator rewrites.
- [x] Fail on unexpected PR retargets.
- [x] Fail on deleted branches for still-open PRs.
- [x] Fail on closed-unmerged dependency PRs.
- [x] Handle merged dependencies whose branches still exist.
- [x] Handle merged dependencies whose branches were deleted.
- [x] Handle squash-merged dependencies where the frozen head is not an ancestor
  of trunk.
- [x] Rebase only owned mutable revisions onto the updated nearest frozen
  boundary or trunk.
- [x] Stop before submit on conflicts.
- [x] Run submit after a clean rebase unless `--no-submit` is passed.
- [x] Test fast-forward collaborator update.
- [x] Test divergent collaborator rewrite failure.
- [x] Test unexpected retarget failure.
- [x] Test deleted open branch failure.
- [x] Test closed-unmerged PR failure.
- [x] Test merged dependency retarget to trunk.
- [x] Test squash-merged dependency recovery.
- [x] Test `sync --no-submit`.
- [x] Test conflict stops before submit.
- [x] Test sync does not mutate frozen PR comments.

### 7. `jj-stack unfreeze`

- [x] Resolve target by PR number.
- [x] Resolve target by PR URL.
- [x] Resolve target by branch.
- [x] Resolve target by change-id prefix.
- [x] Verify the PR head branch is in the target base repo and pushable by the
  current GitHub actor.
- [x] Verify the PR head still matches the last observed head SHA.
- [x] Remove `jj-stack/frozen/pr-<number>`.
- [x] Track the PR branch as a mutable jj bookmark when adopted.
- [x] Record only cache hints after adoption; do not create a separate ownership
  source of truth.
- [x] Verify the revision is mutable after removing the frozen bookmark.
- [x] Fail with the exact remaining immutable blocker when it is still immutable.
- [x] Test successful takeover.
- [x] Test each target form.
- [x] Test unfreeze fails when the PR advanced remotely.
- [x] Test unfreeze fails when another frozen bookmark/tag/trunk still makes the
  revision immutable.
- [x] Test future submit updates only the adopter's jj-stack comment.

### 8. Submit With Frozen Dependencies

- [x] Use the nearest frozen dependency branch as the base for the first owned
  PR.
- [x] Use the previous owned PR branch as the base for later owned PRs.
- [x] Never create/update PRs or comments for frozen revisions.
- [x] Show frozen dependencies separately in owned PR stack comments.
- [x] Keep the compact current stack comment when no frozen dependencies exist.
- [x] Preserve current compact line format:
  `- [description #123](url) _changeid_ · created-date`.
- [x] Mark the current PR with the left-pointing finger marker.
- [x] Include brief CLI instructions in comments for `get`, `sync`, `submit`,
  and `merge`.
- [x] Test base branch selection with no frozen dependency.
- [x] Test base branch selection with one frozen dependency.
- [x] Test base branch selection with multiple frozen dependencies.
- [x] Test frozen PRs are skipped during submit.
- [x] Test comment rendering with frozen dependencies and current owned stack.
- [x] Test compact comment rendering without frozen dependencies.
- [x] Test comment instructions stay short and do not render as a wide table.

### 9. `jj-stack merge`

- [x] Merge only mutable owned PRs.
- [x] Block merge while any frozen dependency PR is still open.
- [x] Require `jj-stack sync` when dependencies are merged but the bottom owned
  PR still targets a frozen branch.
- [x] Merge only when the bottom owned PR targets trunk/base.
- [x] Support dry-run merge output that lists the PRs that would be merged.
- [x] Fail before merging anything when one PR is not mergeable.
- [x] Test open frozen dependency blocks merge.
- [x] Test merged dependency requires sync before merge.
- [x] Test clean owned stack merge.
- [x] Test merge never targets frozen PRs.
- [x] Test dry-run merge does not mutate GitHub.
- [x] Test partial merge failure leaves a clear recovery command.

### 10. `jj-stack status`

- [x] Add human-readable `jj-stack status`.
- [x] Add stable `jj-stack status --json`.
- [x] Report startup alias state.
- [x] Report repo, remote, trunk, and branch-prefix.
- [x] Report owned PRs that submit would manage.
- [x] Report frozen dependencies and their frozen bookmarks.
- [x] Report stale, advanced, or divergent dependency state.
- [x] Report own bookmark tracking problems.
- [x] Report first owned PR base branch.
- [x] Report merge blockers.
- [x] Report suggested next command.
- [x] Include all of the above in `status --json`.
- [x] Test JSON shape for a simple owned stack.
- [x] Test JSON shape for an owned stack with frozen dependencies.
- [x] Test status reports untracked/conflicted own bookmarks.
- [x] Test status reports merge blockers.
- [x] Test status output with stale/divergent frozen dependencies.

### 11. Docs And Release Checks

- [x] Update the README with the tracked-bookmark/frozen-dependency workflow
  overview.
- [x] Add README examples for `get`, `sync --no-submit`, `sync`, `submit`,
  `merge`, `status`, and `unfreeze`.
- [x] Document that old raw-pushed state is unsupported and should fail clearly.
- [x] Document branch naming and why submitted branch names are deterministic.
- [x] Document cache behavior: useful for speed/recovery, never authoritative.
- [x] Document the real GitHub E2E test and cleanup behavior.
- [x] Run `cargo fmt`.
- [x] Run unit tests.
- [x] Run the normal integration test suite.
- [x] Run real local jj integration tests.
- [ ] Run the opt-in real GitHub E2E test before release.
- [x] Run `cargo install --path .` after CLI changes.

## Goal

`jj-stack` should support collaboration without inventing a second local
ownership model. jj already has the right local rewrite boundary:

```text
mutable revisions   = work jj-stack may submit
immutable revisions = frozen dependencies jj-stack must not rewrite
```

`jj-stack` adds GitHub PR mechanics on top of that boundary: tracked PR
bookmarks, branch lookup, last-seen head SHA checks, stack comments, frozen
bookmarks, and safe remote leases.

The intended behavior is:

- your own submitted PR revisions stay editable after `jj-stack submit`;
- imported collaborator PR revisions are frozen by jj;
- trunk, tags, and other normal immutable history stay immutable;
- own remote PR branches are tracked jj bookmarks, so submitting a PR does not
  make the local jj change immutable;
- remote PR branches are updated only when `jj-stack` has a valid push target
  and jj's push safety checks pass.

## Core Model

`jj-stack` uses jj immutability as the source of truth for local submit
selection, but local mutability is not enough to prove remote push authority.
The MVP does not migrate or infer ownership for old `jj-stack` state created
before this tracked-bookmark model.

Submit eligibility has two gates:

1. **Local gate:** the revision is mutable according to jj's actual immutability
   boundary.
2. **Remote gate:** the PR branch is new, was created by `jj-stack`, or was
   explicitly unfrozen/adopted, and the tracked remote bookmark still matches
   the last observed head SHA.

`jj-stack` does not maintain a separate local `owned` / `dependency` revset.
Local state remains PR metadata, comment metadata, recovery data, and remote
lease/adoption data. That state is not the source of truth for whether a
revision is locally mutable.

Submitted `jj-stack` PR branches are local jj bookmarks:

```text
stack/fix-types-restore-repo-check-types-pmtxtmlu -> local jj change
stack/fix-types-restore-repo-check-types-pmtxtmlu@origin -> last observed remote head
```

`jj-stack submit` should set the local bookmark to the selected mutable revision
and push it with `jj git push --bookmark <head-branch> --remote <remote>`.
Jujutsu automatically tracks newly pushed bookmarks. Tracked remote bookmarks do
not participate in jj's default `untracked_remote_bookmarks()` immutable set, so
the local change remains editable after submit.

`jj-stack` should not raw-push own PR branches with:

```text
git push <sha>:refs/heads/<head-branch>
```

Raw pushes create untracked remote bookmarks in jj, which can make the submitted
change immutable and break later `sync`, `submit`, or user edits.

Imported collaborator PRs are frozen by local bookmarks:

```text
jj-stack/frozen/pr-5137
```

This means:

```text
PR #5137's head commit is an immutable head in this workspace
```

Because jj defines `immutable()` as `::(immutable_heads() | root())`, the frozen
bookmark makes the bookmark target and all of its ancestors immutable.

It does not mean "PR #5137 depends on something." It means "this workspace treats
PR #5137 as a frozen dependency, and `jj-stack` must not rewrite or submit it."

## Startup And Tracking

Every `jj-stack` command that resolves stack revsets should first ensure
repo-local jj revset aliases are wired before doing that resolution.

### Tracked Own PR Bookmarks

Own PR branches are created and updated through jj bookmarks only:

```text
jj bookmark set --allow-backwards <head-branch> -r <change>
jj git push --remote <remote> --bookmark <head-branch>
```

`jj git push --bookmark` automatically tracks newly pushed remote bookmarks. For
existing PRs, `jj-stack` should verify the local bookmark, tracked remote
bookmark, and GitHub PR all refer to the same branch/head before updating it.
Local cache may speed up lookup, but it must not prove ownership or push
authority.

Tracking rules:

- new own branches are created as local jj bookmarks and pushed with `jj git
  push --bookmark`;
- existing own branches must be validated through jj bookmark tracking and a
  matching open GitHub PR before update;
- if a matching remote branch exists but no safe matching PR can be found, fail
  instead of tracking/importing it;
- if a known own bookmark is untracked, missing, or conflicted, fail with the
  exact `jj bookmark`/GitHub issue before rebasing or submitting;
- never track collaborator/frozen PR branches.

### Frozen Dependency Alias

The alias only adds imported frozen PR heads to jj immutability. It should not
subtract own stack branches from `builtin_immutable_heads()` in normal operation;
own branches should be tracked instead.

Required shape, using the resolved `remote` and `branch-prefix` config:

```toml
[revset-aliases]
"jj_stack_frozen_heads()" = "bookmarks(glob:'jj-stack/frozen/*')"
"immutable_heads()" = "builtin_immutable_heads() | jj_stack_frozen_heads()"
```

For the default config, this is equivalent to:

```toml
[revset-aliases]
"jj_stack_frozen_heads()" = "bookmarks(glob:'jj-stack/frozen/*')"
"immutable_heads()" = "builtin_immutable_heads() | jj_stack_frozen_heads()"
```

This is paired with tracked own PR bookmarks. jj's default
`builtin_immutable_heads()` includes `untracked_remote_bookmarks()`, so own PR
branches must be created through `jj git push --bookmark`, not raw Git pushes.

Imported collaborator PRs are still frozen because `jj-stack get` creates local
`jj-stack/frozen/pr-*` bookmarks, and those frozen bookmarks are added back to
`immutable_heads()`.

Startup config must be idempotent and preserve existing user behavior:

1. Set or update `jj_stack_frozen_heads()` in repo config.
2. If `immutable_heads()` already has the jj-stack wrapper shape, leave the
   wrapper in place and only update the helper aliases.
3. If `immutable_heads()` is the builtin/default expression, set:

   ```toml
   "immutable_heads()" = "builtin_immutable_heads() | jj_stack_frozen_heads()"
   ```

4. If `immutable_heads()` is custom, wrap it once:

   ```toml
   "jj_stack_base_immutable_heads()" = "<previous immutable_heads() expression>"
   "immutable_heads()" = "jj_stack_base_immutable_heads() | jj_stack_frozen_heads()"
   ```

5. If the current alias cannot be read or safely wrapped, fail before doing any
   submit/sync/merge mutation and print the exact config the user can install.

The config is repo-local and local-machine-only. It should not be committed to
the Git repository.

Commands should either use an inline revset equivalent to jj's actual mutability:

```text
~::(immutable_heads() | root())
```

or verify that the user's `mutable()` / `immutable()` aliases still match jj's
canonical definitions before relying on them.

## Stack Resolution

Commands should distinguish selected ancestry, mutable owned revisions, frozen
dependencies, and the nearest frozen base boundary.

Before resolving a stack, `jj-stack` must resolve `trunk()` and fail clearly if
it does not resolve to exactly one commit.

Conceptually:

```text
selected = <trunk-commit>..@ & ~empty()
actual_mutable = ~::(immutable_heads() | root())
owned = selected & actual_mutable
frozen_dependencies = (::parents(roots(owned)) & jj_stack_frozen_heads() & <trunk-commit>..)
nearest_frozen_boundary = heads(frozen_dependencies)
```

`frozen_dependencies` is plural: it is every frozen PR bookmark reachable below
the bottom owned revision and above trunk, ordered base-to-top. For a fetched
collaborator stack with PRs `#10 -> #11 -> #12`, and local work on top of `#12`,
all three PRs are frozen dependencies. The nearest frozen boundary is `#12`.

The default submit revset can stay equivalent to:

```text
<trunk-commit>..@ & actual_mutable & ~empty()
```

But dependency detection must use `jj_stack_frozen_heads()`, not generic
`immutable()`, because generic immutable history also includes trunk, tags, and
other remote bookmarks.

Shape rules:

- owned revisions must form one linear stack;
- owned revisions must be non-empty and conflict-free;
- there may be zero or one nearest frozen dependency boundary below the bottom
  owned revision;
- comments, status, sync, and merge use the full ordered frozen dependency list;
- multiple dependency boundaries, merge parents, sibling owned branches,
  conflicted frozen bookmarks, or empty/conflicted owned revisions fail clearly;
- root/base validation uses the nearest frozen dependency branch when present,
  otherwise trunk.

## `jj-stack get`

`jj-stack get <target>` imports a GitHub stack as frozen local history.

Accepted targets:

```bash
jj-stack get 5137
jj-stack get https://github.com/org/repo/pull/5137
jj-stack get stack/fix-types-restore-repo-check-types-pmtxtmlu
jj-stack get pmtxtmlu
```

Target resolution:

1. PR URL resolves directly.
2. PR number resolves directly in the configured repo.
3. Exact branch name resolves by querying open PRs for that head branch.
4. Change-id prefix resolves by scanning open jj-stack PR comments/metadata.
5. Ambiguous or missing branch/prefix matches fail with the candidates shown.

Import scope:

- if the target PR has a valid jj-stack stack comment, import the full ordered
  stack from that comment, even if the target is in the middle;
- if the target PR has no valid stack comment, import only that PR;
- MVP does not infer descendants from GitHub branch graph without a stack
  comment.

Validation must happen before moving bookmarks or writing cache:

1. Query every PR in the candidate stack.
2. Verify each PR number, node id, state, head repo, head ref, head SHA, base
   repo, base ref, title, and body.
3. Verify the stack order by checking each PR's base branch/repo relationship
   against the previous PR's head branch/repo.
4. Fail if any PR head branch is from a fork or otherwise cannot be used as a
   base branch in the target repo. Fork-backed stacked dependencies are
   unsupported in the MVP.
5. If a frozen bookmark already exists, move it only when the old frozen target
   is an ancestor of the newly fetched target. Divergent movement fails with
   recovery instructions.

After validation:

1. Fetch every PR head branch.
2. Record PR metadata in local cache:
   - PR number and GitHub node id;
   - head repository id/name;
   - head branch;
   - head SHA;
   - base repository id/name;
   - base branch;
   - title/body;
   - author;
   - stack comment id when known.
3. Create or move one frozen bookmark per fetched PR:

   ```text
   jj-stack/frozen/pr-<number> -> PR head commit
   ```

4. Print the top frozen revision and the next command for the common path:

   ```bash
   jj new jj-stack/frozen/pr-<top-pr>
   ```

`get` should not rely on the submit revset to find fetched revisions, because
fetched revisions become immutable as soon as frozen bookmarks are installed.
It should resolve fetched PR heads by commit id and validated PR metadata.

## Collaboration Paths

### 1. Build On Someone Else's Stack

```bash
jj-stack get 5137
jj new jj-stack/frozen/pr-5137
# edit files
jj describe -m "my feature"
jj-stack submit
```

Result:

```text
collaborator stack  immutable via jj-stack/frozen/pr-5137
my feature          mutable
```

`submit` creates PRs only for mutable owned revisions. The first owned PR uses
the nearest frozen dependency's PR head branch as its base. If there is no frozen
dependency, it uses trunk.

### 2. Sync After Collaborator Updates Their PR

```bash
jj-stack sync
```

Default scope is the current `@` stack:

- if there are mutable owned revisions, sync the frozen dependencies under those
  revisions;
- if `@` is itself frozen and there is no owned segment, sync that imported
  frozen stack;
- `--all` may be added later, but is not required for MVP.

Sync should:

1. Ensure startup config is installed.
2. Fetch trunk.
3. Query GitHub state for the frozen PRs in scope.
4. Re-read the dependency stack comment or validated PR base graph.
5. Compare remote topology with local frozen metadata.
6. Update the frozen set transactionally, or fail without partial bookmark moves.
7. If a frozen PR head advanced and the old head is an ancestor of the new head,
   fetch the new head and move the frozen bookmark.
8. Rebase only owned mutable revisions onto the updated nearest frozen boundary.
9. Stop before submit if the rebase creates conflicts.
10. Run submit only after a clean rebase, unless `--no-submit` was passed.

If a frozen PR was rewritten in a divergent way, closed unmerged, deleted before
merge, retargeted unexpectedly, or is otherwise not recoverable automatically,
sync must fail with concrete recovery instructions.

Merged frozen dependencies are recoverable:

- branch deletion after merge is not an error;
- fetch trunk/base;
- rebase owned changes from the frozen boundary onto trunk/base;
- retarget the bottom owned PR to trunk/base;
- do not require the frozen head to be an ancestor of trunk, because GitHub may
  squash-merge;
- after successful rebase, the old frozen bookmarks may remain for status/review,
  but they are no longer dependencies of the owned stack.

### 3. Submit Your Own Segment

```bash
jj-stack submit
```

Submit should:

1. Ensure startup config is installed.
2. Resolve the mutable owned stack with actual mutability.
3. Find the ordered frozen dependency list and nearest frozen boundary.
4. Resolve the nearest frozen boundary's PR branch from the frozen bookmark and
   live GitHub PR data. Cache may provide a PR-number hint, but stale or missing
   cache must not be trusted.
5. For each owned PR, create or move the local head bookmark to the selected
   mutable revision.
6. Push owned head bookmarks with `jj git push --remote <remote> --bookmark
   <head-branch>`, so new remote bookmarks become tracked and existing tracked
   bookmarks use jj's force-with-lease-equivalent safety checks.
7. Create/update only mutable owned PRs.
8. Never create/update PRs or comments for frozen revisions.

Owned PR comments should show frozen dependencies separately:

```md
Frozen dependencies:
- [collaborator setup #5137](...) _pmtxtmlu_ · 2026-06-03

This stack:
- **[my feature #5200](...)** _rnxuymxn_ · 2026-06-03 👈
- [my followup #5201](...) _xmnqvlsy_ · 2026-06-03
```

If there are no frozen dependencies, keep the current compact list.

### 4. Review Or Pull A Stack Without Editing It

```bash
jj-stack get 5137
```

This fetches the stack and freezes it. The user can inspect it with normal jj
commands. Because frozen heads feed `immutable_heads()`, accidental local
rewrites are blocked by jj unless the user explicitly performs manual recovery.

### 5. Take Over A Frozen PR

Taking over a frozen PR means making it mutable locally and adopting remote
update responsibility for that PR branch.

Preferred path is still social:

```text
Ask the original owner to update their PR.
Run `jj-stack sync`.
```

If takeover is intentional:

```bash
jj-stack unfreeze 5137
jj describe -r <rev> -m "new title"
jj-stack submit
```

`unfreeze` should:

- resolve the target PR/revision;
- verify the PR head branch is in the target base repo and is pushable by the
  current GitHub actor;
- verify the PR head has not advanced since the last local observation;
- remove `jj-stack/frozen/pr-<number>`;
- record the PR branch as explicitly adopted by the current GitHub actor for
  remote lease purposes;
- verify the target revision is actually mutable after the frozen bookmark is
  removed;
- fail with the exact immutability blocker if another frozen bookmark, tag,
  trunk, or untracked remote bookmark still makes the revision immutable;
- print that future submit will update the PR branch through a tracked jj
  bookmark.

`jj --ignore-immutable` is manual recovery only. It can let a user rewrite an
immutable revision locally, but it does not change `immutable_heads()`,
`immutable()`, `mutable()`, or `jj-stack submit` selection. Normal takeover must
make the target pass actual mutability before submit will manage it.

Never edit another user's stack comment during takeover. For adopted mutable
PRs, create or update the current actor's jj-stack comment. `status` should warn
when a foreign stack comment remains on a PR that is now managed locally.

### 6. Merge With Frozen Dependencies

`jj-stack merge` should merge only mutable owned PRs.

MVP behavior:

- if any frozen dependency PR below the owned stack is still open, fail;
- if dependencies are merged but the bottom owned PR still targets a frozen
  branch, require `jj-stack sync` first;
- only merge when the bottom owned PR targets trunk/base.

Example failure:

```text
Cannot merge #5200 because it depends on open frozen PR #5137.
Merge the dependency first, or run `jj-stack sync` after it lands.
```

## Remote Safety

Even when revisions are mutable, remote updates stay conservative:

- no blind force-push;
- own PR branches are local jj bookmarks pushed with `jj git push --bookmark`;
- jj push safety checks must verify the remote bookmark still matches the last
  observed state before updating it;
- fail if the remote branch advanced unexpectedly;
- fail if the branch exists without a matching safe GitHub PR/update target;
- fail if a new branch unexpectedly already exists;
- never push the `jj-stack/frozen/*` bookmark namespace;
- never update stack comments for frozen PRs;
- update only comments authored by the current GitHub actor and matching the
  jj-stack marker/schema.

For updates, prefer jj's tracked bookmark push path:

```text
jj bookmark set --allow-backwards <headRefName> -r <change>
jj git push --remote <remote> --bookmark <headRefName>
```

Raw `git push <sha>:refs/heads/<headRefName>` is not the normal submit path
because it leaves jj seeing `<headRefName>@<remote>` as an untracked remote
bookmark. That can make the submitted change immutable and break later edits or
`jj-stack sync`.

There is no raw `git push` fallback in the MVP. If a branch update cannot be
expressed through jj bookmarks and `jj git push --bookmark`, fail before pushing.

Cached PR metadata may include GitHub identity that helps avoid extra lookup:

- PR number and node id;
- head repository id/name;
- head ref name;
- last observed head oid;
- local tracking bookmark name and expected local target;
- base repository id/name;
- base ref name;
- author;
- branch created/adopted by the current actor when applicable, as a hint only.

If cached metadata is missing or no longer matches GitHub, ignore or refresh the
cache. Fail before pushing only when live GitHub/jj data cannot prove a safe
update target.

Fork-backed PR heads are unsupported for stacked submit in the MVP. `jj-stack`
should fail with a clear message instead of trying to base a new PR on a branch
that cannot be used as an upstream base branch.

## Status Command

Add `jj-stack status` for visibility.

Plain `status` should be human-readable. `status --json` should be stable enough
for tests and scripts.

It should show:

- whether startup aliases are installed and current;
- the resolved repo, remote, trunk, and branch prefix;
- mutable owned PRs that submit would manage;
- frozen dependency PRs and the local bookmark that freezes each one;
- stale/advanced/divergent frozen PR branches;
- known own PR bookmarks that are untracked or conflicted;
- the nearest frozen boundary;
- the base branch for the first owned PR;
- merge blockers;
- what `submit`, `sync`, and `merge` would touch.

Suggested JSON shape:

```json
{
  "config": {
    "remote": "origin",
    "trunk": "main",
    "branch_prefix": "stack",
    "frozen_alias_installed": true,
    "own_pr_bookmarks_tracked": true
  },
  "owned": [
    {
      "change_id": "rnxuymxn",
      "pr_number": 5200,
      "head_branch": "stack/my-feature-rnxuymxn",
      "base_branch": "stack/collaborator-setup-pmtxtmlu"
    }
  ],
  "frozen_dependencies": [
    {
      "pr_number": 5137,
      "bookmark": "jj-stack/frozen/pr-5137",
      "head_branch": "stack/collaborator-setup-pmtxtmlu",
      "state": "open"
    }
  ],
  "merge_blockers": [
    "open frozen dependency #5137"
  ]
}
```

## README Updates

Add a collaboration section with the short version:

```text
jj-stack publishes your own submitted PR heads as tracked jj bookmarks, so they
stay editable after submit.
jj-stack freezes imported PRs with local jj-stack/frozen/pr-* bookmarks.
Those frozen bookmarks feed jj immutable_heads().
Mutable revisions are what jj-stack submits.
Frozen revisions are dependencies.
Use jj-stack unfreeze only when intentionally taking over a PR.
```

Include the common flow:

```bash
jj-stack get <collaborator-pr>
jj new jj-stack/frozen/pr-<collaborator-pr>
jj describe -m "my feature"
jj-stack submit
```

Also include short examples for:

```bash
jj-stack status
jj-stack sync --no-submit
jj-stack sync
jj-stack merge
jj-stack unfreeze <pr>
```

The README should explicitly say that `merge` is blocked while frozen dependency
PRs are still open.

## MVP Decisions

- `jj-stack get` prints the next `jj new ...` command; it does not automatically
  create a mutable change.
- The takeover command is `jj-stack unfreeze`.
- Divergent collaborator rewrites fail with recovery instructions.
- Frozen bookmarks are one per imported PR, not top-only.
- Fork-backed stacked dependencies are unsupported.

## Acceptance Tests

Add integration coverage for:

- startup config installs aliases idempotently;
- startup config wraps custom `immutable_heads()` once;
- submit creates/moves local PR head bookmarks and pushes them with
  `jj git push --bookmark`;
- submitted own `stack/*` branches are tracked and remain mutable/editable after
  submit;
- submit fails clearly if a matching remote `stack/*` branch exists without a
  matching safe GitHub PR/update target;
- imported frozen PRs become immutable and block normal jj rewrites;
- `unfreeze` removes the frozen marker and verifies the revision is mutable;
- stack resolution with a frozen dependency below owned work;
- stack resolution rejects sibling owned branches, merge parents, conflicted
  revisions, empty revisions, and conflicted frozen bookmarks;
- `get` by PR number, PR URL, exact branch, and change-id prefix;
- `get` rejects ambiguous prefixes;
- `get` imports a full stack from a valid stack comment;
- `get` imports only the target PR when no stack comment exists;
- `get` validates PR base/head topology before moving bookmarks;
- `get` rejects divergent movement of an existing frozen bookmark;
- `sync` fast-forwards frozen dependency heads and rebases owned work;
- `sync --no-submit` rebases but does not submit;
- `sync` stops before submit on conflicts;
- `sync` fails on divergent collaborator rewrites;
- `sync` handles merged dependencies, including deleted branches and squash
  merges;
- `submit` uses the nearest frozen dependency branch as the base for the first
  owned PR;
- `submit` ignores or refreshes stale cache and fails only when live GitHub/jj
  data cannot prove a safe update target;
- `submit` fails when jj push safety checks report that the remote bookmark
  advanced unexpectedly;
- `submit` never updates frozen PRs or foreign stack comments;
- `merge` is blocked while frozen dependency PRs are open;
- `status --json` reports config state, own bookmark tracking state, owned PRs,
  frozen dependencies, stale frozen branches, base branch, and merge blockers.
