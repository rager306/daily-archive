# S05: Contracts and Invariants

**Goal:** Define conceptual contracts and invariants for the future universal evidence pipeline: generic knowledge records, jobs, artifacts, sidecars, failure taxonomy, review packets, graph-readiness handoff, GraphDB portability, and safety flags.
**Demo:** After this, future implementation has draft contracts for generic knowledge records, jobs, artifacts, sidecars, failure taxonomy, review packets, graph-readiness handoff, GraphDB portability, and safety invariants.

## Must-Haves

- Drafts generic contract inventory for KnowledgeSourceRecord, DomainAdapterRecord, EvidenceArtifactRecord, ProcessingJob, DependencyRecord, FailureRecord, CandidatePacket, ReviewPacket, GraphReadinessHandoff, KnowledgeSubstratePort, and SafetyFlags.
- Drafts paper-specific specializations for ArticleRecord, SourceRecord, ArticleJob, SidecarJob, GROBID/OpenDataLoader/Adaptix output boundaries, and paper review packets.
- Defines status transitions, stale detection rules, retryable versus terminal failures, GraphDB portability constraints, raw-text/redaction rules, and evidence-path constraints.
- Uses Mermaid classDiagram, flowchart, or stateDiagram only where it improves readability and avoids over-specification.
- Keeps graph/import safety flags false by default.
- Addresses S01 audit conflicts that touch contracts or invariants.

## Proof Level

- This slice proves: Contract checklist, invariant audit, and diagram readability check.

## Integration Closure

Feeds roadmap implementation gates and future verifier design.

## Verification

- Makes diagnostics, backend health, cache health, retry state, blocked states, evidence lineage, and graph-substrate portability explicit.

## Tasks

- [x] **T01: Draft core contracts and safety invariants** `est:medium`
  Create CONTRACTS.md and SAFETY-INVARIANTS.md covering generic universal-KB contracts, paper-specific specializations, GraphDB portability, and fail-closed safety flags.
  - Files: `.gsd/milestones/M034-kuei9y/decision-package/CONTRACTS.md`, `.gsd/milestones/M034-kuei9y/decision-package/SAFETY-INVARIANTS.md`
  - Verify: Check both files include required contract names, `KnowledgeSubstratePort`, paper-specific sidecar contracts, and safety defaults.

- [x] **T02: Draft status matrix failure taxonomy and dependency model** `est:medium`
  Create STATUS-MATRIX.md, FAILURE-TAXONOMY.md, and ARTIFACT-DEPENDENCY-MODEL.md describing status transitions, retryable/terminal/blocked failures, stale detection, sidecar dependency graph, and redacted diagnostics.
  - Files: `.gsd/milestones/M034-kuei9y/decision-package/STATUS-MATRIX.md`, `.gsd/milestones/M034-kuei9y/decision-package/FAILURE-TAXONOMY.md`, `.gsd/milestones/M034-kuei9y/decision-package/ARTIFACT-DEPENDENCY-MODEL.md`
  - Verify: Check files include pending/ready/running/succeeded/failed_retryable/failed_terminal/blocked/stale/needs_review statuses, failure codes, sidecar dependency graph, and redaction constraints.

- [x] **T03: Verify S05 contracts and invariants** `est:small`
  Implement and run a verifier for contracts/invariants/status/failure/dependency artifacts, checking required contract names, safety flags, status transitions, failure classes, dependency model, GraphDB portability, and Mermaid readability limits.
  - Files: `scripts/verify_m034_contracts_invariants.py`
  - Verify: `uv run python scripts/verify_m034_contracts_invariants.py --package-dir .gsd/milestones/M034-kuei9y/decision-package && uv run ruff check scripts/verify_m034_contracts_invariants.py`

## Files Likely Touched

- .gsd/milestones/M034-kuei9y/decision-package/CONTRACTS.md
- .gsd/milestones/M034-kuei9y/decision-package/SAFETY-INVARIANTS.md
- .gsd/milestones/M034-kuei9y/decision-package/STATUS-MATRIX.md
- .gsd/milestones/M034-kuei9y/decision-package/FAILURE-TAXONOMY.md
- .gsd/milestones/M034-kuei9y/decision-package/ARTIFACT-DEPENDENCY-MODEL.md
- scripts/verify_m034_contracts_invariants.py
