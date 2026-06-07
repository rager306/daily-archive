# S03: Formal ADR Package and GraphDB Deferral

**Goal:** Create the formal Mermaid-assisted ADR package for core post-M033 architecture choices, especially deferred GraphDB selection, sidecar candidate boundaries, no direct GraphDB writes, quant-mind pattern-source status, and agent boundary, all under ADR-000 and S01 audit findings.
**Demo:** After this, accepted architecture choices, deferred GraphDB selection, and rejection boundaries are expressed as formal Mermaid-assisted ADRs.

## Must-Haves

- ADR inventory separates accepted, rejected, deferred, and open decisions.
- ADRs use the Mermaid-assisted enhanced template where diagrams clarify context, safety gates, status transitions, option comparisons, or contract relationships.
- ADRs cover universal KB direction, GraphDB selection deferral, durable lazy async sidecar pipeline before agents, sidecar role boundaries, parser output as candidate evidence, no direct parser to any GraphDB path, quant-mind as pattern source not dependency, and agent boundary.
- GraphDB ADR lists LadybugDB, FalkorDB, HelixDB, and other candidates with comparison dimensions.
- Every ADR states impacted R/D records and references S01 conflict categories.
- Rejected/deferred options explain revisit criteria.

## Proof Level

- This slice proves: ADR format checklist plus consistency check against S01 audit, D061-D067, R054-R061, and ADR-000.

## Integration Closure

Feeds PRD, requirements, contracts, roadmap, and closeout with accepted, rejected, deferred, and conflict-aware decision language.

## Verification

- Creates durable decision records for future agents to cite instead of re-litigating M033 or prematurely locking GraphDB.

## Tasks

- [x] **T01: Draft GraphDB and no-direct-write ADRs** `est:medium`
  Draft ADR-002 deferred GraphDB selection and ADR-005 no direct extractor/parser/sidecar to GraphDB path. Both must use the template, reference S01 audit findings, include R/D impact tables, and preserve safety non-authorization.
  - Files: `.gsd/milestones/M034-kuei9y/decision-package/ADR-002-defer-final-graphdb-selection.md`, `.gsd/milestones/M034-kuei9y/decision-package/ADR-005-no-direct-extractor-to-graphdb-path.md`, `.gsd/milestones/M034-kuei9y/decision-package/ADR-INDEX.md`
  - Verify: Run marker checks that ADR-002 and ADR-005 include required template sections, GraphDB candidates, safety flags, R/D references, and Mermaid diagram counts within limits.

- [x] **T02: Draft evidence pipeline sidecar quant-mind and agent ADRs** `est:large`
  Draft ADR-003 durable lazy async evidence pipeline, ADR-004 sidecars as candidate evidence producers, ADR-006 agent boundary, and ADR-007 quant-mind pattern source not runtime dependency. Each ADR must use the template and route relevant S01 clarification items.
  - Files: `.gsd/milestones/M034-kuei9y/decision-package/ADR-003-durable-lazy-async-evidence-pipeline.md`, `.gsd/milestones/M034-kuei9y/decision-package/ADR-004-sidecars-as-candidate-evidence-producers.md`, `.gsd/milestones/M034-kuei9y/decision-package/ADR-006-agent-boundary.md`, `.gsd/milestones/M034-kuei9y/decision-package/ADR-007-quantmind-pattern-source-not-runtime-dependency.md`, `.gsd/milestones/M034-kuei9y/decision-package/ADR-INDEX.md`
  - Verify: Run marker checks that all four ADRs include required template sections, safety non-authorization, R/D impact, LLM Reading Notes, and bounded Mermaid usage.

- [x] **T03: Verify S03 formal ADR package** `est:small`
  Implement and run a verifier for all S03 ADRs and the ADR index, checking template sections, status/binding levels, GraphDB deferral, safety markers, R/D references, Mermaid limits, and S01 audit route coverage.
  - Files: `scripts/verify_m034_formal_adr_package.py`
  - Verify: `uv run python scripts/verify_m034_formal_adr_package.py --package-dir .gsd/milestones/M034-kuei9y/decision-package && uv run ruff check scripts/verify_m034_formal_adr_package.py`

## Files Likely Touched

- .gsd/milestones/M034-kuei9y/decision-package/ADR-002-defer-final-graphdb-selection.md
- .gsd/milestones/M034-kuei9y/decision-package/ADR-005-no-direct-extractor-to-graphdb-path.md
- .gsd/milestones/M034-kuei9y/decision-package/ADR-INDEX.md
- .gsd/milestones/M034-kuei9y/decision-package/ADR-003-durable-lazy-async-evidence-pipeline.md
- .gsd/milestones/M034-kuei9y/decision-package/ADR-004-sidecars-as-candidate-evidence-producers.md
- .gsd/milestones/M034-kuei9y/decision-package/ADR-006-agent-boundary.md
- .gsd/milestones/M034-kuei9y/decision-package/ADR-007-quantmind-pattern-source-not-runtime-dependency.md
- scripts/verify_m034_formal_adr_package.py
