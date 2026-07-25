# ADR-004: Sidecars as Candidate Evidence Producers

**Status:** Accepted  
**Date:** 2026-06-06  
**Deciders:** human  
**Milestone:** M034-kuei9y  
**Scope:** sidecar / safety / evidence-pipeline  
**Binding Level:** binding  
**Revisable:** yes, with explicit future ADR evidence.

## 0. One-line Decision

> We will treat GROBID, OpenDataLoader, Adaptix, and future extractors as candidate evidence producers.  
> We will not treat sidecar success as semantic truth, graph readiness, or import eligibility.

## 1. Context

M033 proved GROBID can produce scholarly TEI candidates, OpenDataLoader can produce layout/OCR/table candidates, and Adaptix can structurally adapt fixed parser JSON. None prove semantic correctness or graph readiness. S01 flagged paper-sidecar scope decisions that must remain first-domain evidence, not universal truth.

### Context Map

```mermaid
flowchart LR
    A[GROBID/OpenDataLoader/Adaptix] --> B[Evidence artifact]
    B --> C[Candidate packet]
    C --> D[Validation]
    D --> E[Review packet]
    E --> F[Readiness handoff]
```

## 2. Decision

Sidecars produce evidence artifacts and candidate packets only. Their outputs must pass contract validation, review packet generation, readiness review, and explicit future promotion before graph storage.

## 3. Applies To

GROBID, OpenDataLoader, Adaptix, future paper-domain extractors, future non-paper domain adapters, candidate packet contracts, review packet contracts.

## 4. Requirements and Decisions Impacted

### Requirements

| Requirement | Impact | Notes |
|---|---|---|
| R050 | supports | Artifact detection remains no-import candidate generation. |
| R056 | supports | Parser outputs remain candidate evidence. |
| R027 | constrains | Quality contract must validate conversion/chunk readiness. |
| R029 | constrains | Import-ready package needs review evidence. |

### Decisions

| Decision | Impact | Notes |
|---|---|---|
| D061 | narrows | Vendor sidecars are research/probe inputs, not production adoption. |
| D062 | narrows | External parser research is paper-domain evidence. |
| D063 | supports | Sidecars need durable orchestration. |

## 5. Options Considered

### Option A — Treat best parser output as truth

Rejected: parser success can hide wrong reading order, table errors, citation mismatch, or OCR noise.

### Option B — Treat sidecars as candidate evidence

Chosen: preserves reviewability and fail-closed graph boundary.

## 6. Trade-off Analysis

| Trade-off | Chosen side | Why |
|---|---|---|
| Parser confidence vs reviewability | Reviewability | Scientific/KB trust requires evidence and review. |
| Fast graph path vs safe candidate path | Safe candidate path | Prevents false facts. |

## 7. Consequences

### Positive
- Enables using strong tools without granting authority.
- Keeps sidecar roles composable.

### Negative
- Requires validators/review packets before usefulness claims.

### New obligations
- Define sidecar output boundaries and candidate packet schemas.

## 8. Safety and Non-Authorization

This ADR does **not** authorize:

- production graph import;
- final GraphDB selection;
- LadybugDB/FalkorDB/HelixDB writes;
- parser output as graph-ready truth;
- agentic orchestration unless this ADR explicitly scopes a future helper;
- bypassing validators or review packets.

Required safety defaults:

```text
graph_import_allowed=false
graphdb_written=false
ladybugdb_written=false
production_import_attempted=false
import_eligible=false
```

## 9. Contract Impact

Affected contracts: `EvidenceArtifactRecord`, `CandidatePacket`, `ReviewPacket`, `SafetyFlags`, paper-specific GROBID/OpenDataLoader/Adaptix sidecar contracts.

## 10. Validation / Evidence Required

Future quality milestones must verify reading order, metadata/reference quality, table/figure handling, coordinate fidelity, adapter shape, and no-import flags.

## 11. Open Questions

| Question | Owner | Needed by | Blocking? |
|---|---|---|---|
| Which sidecar quality metrics are sufficient for paper-domain readiness? | future quality milestone | before adoption | yes |
| Which sidecars apply to non-paper domains? | future domain planning | after paper domain stabilizes | no |

## 12. Follow-up Actions

- [ ] S05 contracts must define sidecar candidate boundaries.
- [ ] Future quality milestone must evaluate GROBID/OpenDataLoader/Adaptix on a corpus.

## 13. Supersedes / Superseded By

### Supersedes

- Narrows any historical wording that conflicts with ADR-000, ADR-002, or ADR-005.

### Superseded By

- Empty until future ADR.

## 14. LLM Reading Notes

- Binding decision:
  - Sidecar outputs are candidate evidence only.
- Do not infer:
  - Do not infer parser success means graph readiness.
- Safe next action:
  - Draft candidate/review contracts.
- Blocked until:
  - Import eligibility requires future reviewed promotion.
