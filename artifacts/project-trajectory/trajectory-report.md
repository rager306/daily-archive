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
| module_code | tracked | uncommitted_changes_present | git_changed_files=12 |
| evidence | tracked | none | .gsd/milestones/M049-ndk541/M049-ndk541-SUMMARY.md, .gsd/milestones/M050-l8os7p/M050-l8os7p-SUMMARY.md, .gsd/milestones/M051-aaw9j7/M051-aaw9j7-SUMMARY.md |
| safety | clear | none | prohibited-claim scan over PROJECT/README/recent summaries |
| operations | tracked | none | runtime/service state is artifact-derived; live process management remains external |
| next_gate | clear | none | README.md, recent milestone summaries |
| reverse_adr_audit | clear | none | rule_count=8, src/ (rule: no_ladybugdb_import_in_src, anchor: ADR-002 (Defer Final GraphDB Selection), ADR-005 (No Direct Extractor to GraphDB)), artifacts/ (rule: no_graph_import_ |

## Drift flags

| Severity | Flag | Evidence |
|---|---|---|
| info | uncommitted_changes_present | 12 files |

## Recent milestones

| Milestone | Title | Status |
|---|---|---|
| M046-3b7gp0 | Universal KB Synthesis Package M033 to M045 | complete |
| M047-96puxn | Architecture Guardrail Enforcement and Reverse ADR Audit | complete |
| M048-8bhn38 | Trajectory Severity Tuning per Phase | complete |
| M049-ndk541 | Models Registry Foundation | complete |
| M050-l8os7p | Bounded LLM Helper v2 Worker Pool | complete |
| M051-aaw9j7 | Bounded PDF Acquisition for Linked Target Records | complete |

## Next actions

- Run focused verification and commit or intentionally leave a handoff.
