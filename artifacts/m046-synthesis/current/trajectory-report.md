# Project Trajectory Report

- Verdict: `on_track`
- Phase: `preflight`
- Derived, not canonical: true
- Graph writes: disabled
- Production import: disabled
- Fact promotion: disabled
- codebase-memory snapshot provided: false

## Dimensions

| Dimension | Status | Flags | Evidence |
|---|---|---|---|
| architecture | tracked | none | .gsd/DECISIONS.md, doc/adr/, .codebase-memory/governance-graph.json |
| functionality | tracked | none | requirements=65, statuses={'active': 17, 'validated': 48} |
| module_code | tracked | uncommitted_changes_present | git_changed_files=5 |
| evidence | tracked | none | .gsd/milestones/M045-4s8e44/M045-4s8e44-SUMMARY.md, .gsd/milestones/M046-3b7gp0/M046-3b7gp0-SUMMARY.md, .gsd/milestones/M047-96puxn/M047-96puxn-SUMMARY.md |
| safety | clear | none | prohibited-claim scan over PROJECT/README/recent summaries |
| operations | tracked | none | runtime/service state is artifact-derived; live process management remains external |
| next_gate | clear | none | README.md, recent milestone summaries |
| reverse_adr_audit | clear | none | rule_count=8, src/ (rule: no_ladybugdb_import_in_src, anchor: ADR-002 (Defer Final GraphDB Selection), ADR-005 (No Direct Extractor to GraphDB)), artifacts/ (rule: no_graph_import_ |

## Drift flags

| Severity | Flag | Evidence |
|---|---|---|
| info | uncommitted_changes_present | 5 files |

## Recent milestones

| Milestone | Title | Status |
|---|---|---|
| M042-m64cj9 | Linked Metadata and Connectivity Readiness | complete |
| M043-cqiqeq | Combined Sidecar Evidence Probe | complete |
| M044-qq02k8 | Live GROBID Probe and Architecture Guardrail | complete |
| M045-4s8e44 | Unified Project Trajectory Check | complete |
| M046-3b7gp0 | Universal KB Synthesis Package M033 to M045 | complete |
| M047-96puxn | Architecture Guardrail Enforcement and Reverse ADR Audit | complete |

## Next actions

- Run focused verification and commit or intentionally leave a handoff.
