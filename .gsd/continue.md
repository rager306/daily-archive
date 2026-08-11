# Handoff — 2026-08-11 (M375 Wave 2)

## Completed milestones

M371-M374 complete. M375-h6s6bp active.

## M375 delivered

### S01 Accepted revision ontology DATA

- User accepted ADR-058; human decision D223.
- `f7c1e079`: production `PaperRevision`, `SourceManifestation`, `HAS_REVISION`, `HAS_MANIFESTATION`, revision `hasPart`, revision `SUPERSEDES`; draft superseded; additive revision/manif/section VIDs; governor accepted-state ratchet.
- `39c9d6b1`: compiled endpoint matrix exact with YAML; workspace-aware DATA registry.
- S01/S02 skip-delivered.

### S03 dedicated application materializer

- `b85c14ba`: new `RevisionMaterializationUseCase` over injected `DirectGraphStore` + `Clock`.
- One canonical Paper (unchanged `paper_vid`), immutable PaperRevision, content-addressed SourceManifestation, revision-scoped Sections.
- D127 false; structured create/reuse counters and contextual errors.
- Current revision = max observed revision; reverse load order works.
- Adjacent observed SUPERSEDES reconciliation: v1→v5 creates v5→v1; later v3 invalidates stale v5→v1 and creates v5→v3→v1.
- Replay creates zero duplicate mock nodes/edges.
- Corrected manifestation ownership: content-addressed manifestation has no single `revision_vid`; `HAS_MANIFESTATION` supports shared bytes across revisions.
- 8 revision tests; all 124 da-application tests; clippy; full architecture guardrail PASS.

## Important semantic blocker / next task

S03 remains active with newly planned T04 `Unify Revision Section Context`.

Do **not** wire the new materializer directly into current `da load` yet. Existing `IngestUseCase` and `ExtractionUseCase` still create/resolve revision-agnostic Section VIDs. Half-wiring would duplicate Sections and corrupt `FOUND_IN` provenance.

Next thin slice:

1. introduce typed optional revision context at application materialization/extraction boundary;
2. versioned ingest uses only `revision_section_vid`; unversioned behavior remains unchanged;
3. extraction resolves FOUND_IN using the same context;
4. then CLI composes revision context from typed locator + ensured exact assets;
5. add old/new/negative/replay tests before live Samyama proof.

## Following S04

One-process real Samyama v1+v5:
- one Paper, two revisions;
- exact PDF/HTML manifestations;
- v1 sections=20, v5 sections=13;
- current=v5 and explicit v1 queries;
- adjacent SUPERSEDES;
- replay physical writes zero;
- snapshot export/reload;
- D127 false.

## Evidence and locks

- v1 PDF `bd601ac0…`, HTML `0e11dbd…`; v5 PDF `aa6e247b…`, HTML `4dc4abdd…`.
- Full architecture PASS; governor active 0 violations / 2 operational drift flags; app logs no ERROR/panic.
- D127 false; ADR-057 Proposed; ADR-058 Accepted.
- Never change existing `paper_vid` semantics (GitNexus CRITICAL 74 symbols, 8 flows).
