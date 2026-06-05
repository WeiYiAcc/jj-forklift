# Safe Collaboration With jj Immutability

This prior discussion has been superseded by
[collaboration-flow.md](collaboration-flow.md).

The current spec intentionally avoids a separate local ownership model:

- jj immutability decides which revisions are locally mutable;
- `jj-stack/frozen/pr-*` bookmarks freeze imported collaborator PRs;
- own submitted PR heads are tracked jj bookmarks, so they stay editable after
  submit;
- remote updates are gated by PR metadata and jj push safety checks;
- intentional takeover is handled by `jj-stack unfreeze`.

Implement [collaboration-flow.md](collaboration-flow.md), not older adopt or
dependency-state proposals.
