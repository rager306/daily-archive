# Project Trajectory Report

- Verdict: `drift_risk`
- Derived, not canonical: true
- Graph writes: disabled
- Production import: disabled
- Fact promotion: disabled
- codebase-memory snapshot provided: false

## Dimensions

| Dimension | Status | Flags | Evidence |
|---|---|---|---|
| architecture | tracked | none | .gsd/DECISIONS.md, doc/adr/, .codebase-memory/governance-graph.json |
| functionality | tracked | none | requirements=65, statuses={'active': 18, 'validated': 47} |
| module_code | tracked | uncommitted_changes_present | git_changed_files=4 |
| evidence | tracked | none | .gsd/milestones/M042-m64cj9/M042-m64cj9-SUMMARY.md, .gsd/milestones/M043-cqiqeq/M043-cqiqeq-SUMMARY.md, .gsd/milestones/M044-qq02k8/M044-qq02k8-SUMMARY.md |
| safety | clear | none | prohibited-claim scan over PROJECT/README/recent summaries |
| operations | tracked | none | runtime/service state is artifact-derived; live process management remains external |
| next_gate | needs_attention | missing_next_gate | README.md, recent milestone summaries |

## Drift flags

| Severity | Flag | Evidence |
|---|---|---|
| medium | missing_next_gate | README.md |
| info | uncommitted_changes_present | 4 files |

## Recent milestones

| Milestone | Title | Status |
|---|---|---|
| M039-7o4yf1 | Typed Governance Graph Projection | complete |
| M040-4flhk6 | Real Corpus Continuity Normalization and Expansion | complete |
| M041-8k3kv4 | Mixed Corpus Connectivity Smoke | complete |
| M042-m64cj9 | Linked Metadata and Connectivity Readiness | complete |
| M043-cqiqeq | Combined Sidecar Evidence Probe | complete |
| M044-qq02k8 | Live GROBID Probe and Architecture Guardrail | complete |

## Next actions

- Add or confirm the next gate before starting broad implementation.
- Run focused verification and commit or intentionally leave a handoff.
