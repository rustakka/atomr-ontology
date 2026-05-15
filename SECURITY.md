# Security policy

## Supported versions

Pre-1.0 releases (the current 0.x series) receive security fixes on the
latest published patch only. Once we cut 1.0 we will document a
support window here.

## Reporting a vulnerability

Please email the maintainers privately rather than opening a public
issue. Provide:

- a description of the vulnerability and the affected crate(s),
- a minimal reproduction (if possible),
- any patches you have already prepared,
- whether you would like to be credited in the release notes.

We aim to acknowledge reports within five business days and to ship a
fix or mitigation within thirty days for high-severity issues. For
coordinated disclosure please give us at least ninety days from
acknowledgement before publishing details.

## Scope

In-scope:
- All crates published from this workspace (`atomr-ontology-*`,
  `atomr-ontology`).
- Default configurations of the in-tree examples.

Out-of-scope (report to the upstream project instead):
- Vulnerabilities in `atomr-agents`, `atomr-infer`, or upstream
  inference providers reached through this crate's adapters.
- Issues in third-party crates pinned via `[workspace.dependencies]`.
