# M034 PRD: Universal Evidence Orchestration

## Product Summary

Build the decision package for a future local-first universal knowledge-base evidence orchestration layer. The system's first proving domain is scientific articles, but the architecture must separate generic evidence primitives from paper-specific adapters.

## Goals

1. Support durable, lazy, resumable processing of knowledge sources into evidence artifacts.
2. Keep scientific articles as the first validated domain through GROBID/OpenDataLoader/Adaptix-style sidecars.
3. Preserve candidate-evidence, review, readiness, and no-write boundaries before graph promotion.
4. Keep final GraphDB choice deferred through `KnowledgeSubstratePort`.
5. Defer agentic orchestration until deterministic tools, queue state, contracts, and review gates exist.

## Non-goals

- No queue/worker implementation in M034.
- No final GraphDB selection.
- No LadybugDB/FalkorDB/HelixDB writes.
- No production graph import.
- No parser output as graph-ready truth.
- No agentic orchestration runtime.

## Users and Workflows

### Future maintainer / agent

- Inspect source, artifact, job, and review status.
- Determine why a job is blocked, stale, retryable, or terminal.
- Read ADR/PRD/contracts to know what is generic and what is paper-specific.

### Scientific-paper pipeline user

- Process a paper through sidecar evidence generation.
- Inspect GROBID scholarly candidates, OpenDataLoader layout/table candidates, and Adaptix mapping diagnostics.
- Build review packets without enabling graph writes.

### Future knowledge-substrate evaluator

- Compare LadybugDB, FalkorDB, HelixDB, and other candidates without changing parser contracts.
- Use `KnowledgeSubstratePort` and no-write handoff artifacts.

## Core Workflow

```text
Knowledge source
  -> source record
  -> processing job
  -> evidence artifact
  -> candidate packet
  -> validation
  -> review packet
  -> readiness handoff
  -> explicit future promotion only
```

## Generic vs Paper-specific Scope

| Layer | Generic Universal-KB Scope | Scientific-paper First-domain Scope |
|---|---|---|
| Source | `KnowledgeSourceRecord` | `ArticleRecord`, `PaperSourceRecord` |
| Processing | `ProcessingJob`, `DependencyRecord` | `ArticleJob`, `SidecarJob` |
| Evidence | `EvidenceArtifactRecord` | GROBID TEI, OpenDataLoader layout JSON, Adaptix typed mapping |
| Candidate | `CandidatePacket` | section/reference/citation/table/layout candidates |
| Review | `ReviewPacket` | paper graph-readiness review packet |
| Storage | `KnowledgeSubstratePort` | no-write graph-readiness handoff only |

## Acceptance Criteria

- PRD separates generic and paper-specific scope.
- Requirements include functional and non-functional acceptance criteria.
- Safety defaults remain false: `graph_import_allowed=false`, `graphdb_written=false`, `ladybugdb_written=false`, `production_import_attempted=false`, `import_eligible=false`.
- PRD references ADR-000, ADR-002, ADR-003, ADR-004, ADR-005, ADR-006, and ADR-007.
- PRD incorporates S01 audit clarifications: paper-domain scope, GraphDB deferral, sidecar candidate evidence, and bounded agent/helper language.

## Source ADRs

- ADR-000: Universal KB North Star
- ADR-002: Defer Final GraphDB Selection
- ADR-003: Durable Lazy Async Evidence Pipeline
- ADR-004: Sidecars as Candidate Evidence Producers
- ADR-005: No Direct Extractor to GraphDB Path
- ADR-006: Agent Boundary
- ADR-007: Quant-mind Pattern Source Not Runtime Dependency
