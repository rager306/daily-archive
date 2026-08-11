# Handoff — 2026-08-11 (M375 Wave 1)

## Sealed

M371, M372, M373, M374 complete. M374 commits: `12939518` product, `1aa16a77` GSD closeout, `9f963e89` handoff.

## Active M375-h6s6bp

Vision: accepted ADR-058 revision-aware graph with one Paper, immutable revisions/manifests, revision-scoped Sections, real Samyama proof.

### Delivered

- User accepted ADR-058; human decision D223 recorded.
- Commit `f7c1e079`: production DATA for `PaperRevision`, `SourceManifestation`, `HAS_REVISION`, `HAS_MANIFESTATION`, revision `hasPart`, revision `SUPERSEDES`; draft marked superseded.
- Additive VIDs: `paper_revision_vid`, `source_manifestation_vid`, `revision_section_vid`; `paper_vid` unchanged.
- Property test found/fixed generic id `av0` false-positive; regression seed tracked.
- Governor ratcheted: production DATA required by tests; direct revision graph writes inside universal `ArticleLoadUseCase` blocked.
- Commit `39c9d6b1`: compiled edge endpoint matrix aligned exactly with production YAML; workspace-aware DATA registry prevents silent compiled fallback in tests.
- S01/S02 skip-delivered. Full architecture guardrail PASS; da-domain 360 tests; VID 21; governor 33, zero violations.

### Next

Plan/execute S03 `RevisionMaterializationUseCase` as additive da-application use case over injected GraphStore:

1. input carries canonical paper id, provider/revision, exact PDF/HTML manifestations, parsed article;
2. get/create one Paper via unchanged `paper_vid`;
3. get/create PaperRevision + SourceManifestation via new VIDs;
4. revision-scoped Section VIDs and `PaperRevision-hasPart->Section`;
5. observed predecessor `SUPERSEDES`; explicit `is_current` update;
6. idempotent replay counters and D127 false;
7. do not inline writes in ArticleLoadUseCase; CLI composes after normal ingest until migration is proven;
8. S04 real one-process Samyama v1+v5 proof and query tasks.

## Proven source artifacts

- v1 PDF `bd601ac0…`, HTML `0e11dbd…`, 20 sections, 3 entities.
- v5 PDF `aa6e247b…`, HTML `4dc4abdd…`, 13 sections, 2 entities.
- Both exact independent SHA matches; cold miss/miss, offline hit/hit; canonical unit `2506.07853`; D127 false.

## Locks

D127 false; ADR-057 Proposed; ADR-058 Accepted; no `paper_vid` change (GitNexus CRITICAL 74 symbols/8 flows); cache/catalog evidence is not authoritative graph truth.
