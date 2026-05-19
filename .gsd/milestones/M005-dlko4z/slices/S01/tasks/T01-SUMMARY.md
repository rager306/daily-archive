---
id: T01
parent: S01
milestone: M005-dlko4z
key_files:
  - .gsd/milestones/M005-dlko4z/slices/S01/import-ready-chunk-contract.md
key_decisions:
  - Import readiness is defined separately from package structural validity.
  - Annotations are weak deterministic sidecars and must not be promoted to KG facts in M005.
  - Machine artifacts may contain IDs, spans, states, counts, and warning codes but not raw text, raw chunk text, embeddings, vectors, secrets, or optimizer traces.
duration: 
verification_result: passed
completed_at: 2026-05-19T05:00:57.521Z
blocker_discovered: false
---

# T01: Defined the versioned import-ready chunk package contract for M005.

**Defined the versioned import-ready chunk package contract for M005.**

## What Happened

Created the S01 import-ready chunk contract as a concrete implementation spec for future validators and exporters. The document defines `ImportReadyChunkPackage`, `GraphReadyChunk`, `ChunkAnnotation`, evidence paths, source spans, warning/refusal rules, state/route/type enums, route compatibility, redaction policy, diagnostics, and S01 validator expectations. It explicitly separates package validity from import eligibility and blocks raw text, embeddings, production KG writes, and annotation-to-fact promotion.

## Verification

`test -s .../import-ready-chunk-contract.md && rg "ImportReadyChunkPackage|GraphReadyChunk|ChunkAnnotation|GraphReadinessState" ...` passed and found all required contract terms.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `test -s .gsd/milestones/M005-dlko4z/slices/S01/import-ready-chunk-contract.md && rg "ImportReadyChunkPackage|GraphReadyChunk|ChunkAnnotation|GraphReadinessState" .gsd/milestones/M005-dlko4z/slices/S01/import-ready-chunk-contract.md` | 0 | ✅ pass — contract file exists and required terms are present | 0ms |

## Deviations

None.

## Known Issues

The contract is documentary in T01; executable validator coverage is planned in S01/T03.

## Files Created/Modified

- `.gsd/milestones/M005-dlko4z/slices/S01/import-ready-chunk-contract.md`
