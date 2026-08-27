# M463: GROBID-like native Rust platform on ruVector, SONA and ruVLLM

Status: reviewed research for ADR-075. Decision: D294.

Full evidence and source inventory:
`.compound-engineering/artifacts/research/2026-08-27-grobid-rust-ruvector-platform-research.md`.

## Finding

The correct target is a staged scholarly parse platform in `da-parse`, not a
line-by-line Java GROBID port. Java GROBID stays the oracle and fallback while
native Rust stages replace its cascade independently after conservation,
quality, determinism and load gates pass.

```text
PDF -> LayoutToken -> segmentation -> header/names/affiliations
    -> references/citations -> full text -> figures/tables/formulas -> TEI
```

Modern Markdown/layout systems are useful donors and comparison backends, but
none supplies GROBID's scholarly TEI, citation-context and consolidation
contract. Entity extraction F1 is not sufficient proof of parse fidelity.

## Platform boundary

```text
ScholarlyParser<B: ParseBackend, L: TeiLifter>
  -> typed request and backend identity
  -> bounded endpoint admission
  -> raw TeiEnvelope cache
  -> pure deterministic lift
  -> conservation_inventory
  -> ParsedArticle projector cache
```

Required contracts:

- `ParseBackend` for fulltext/header/references;
- `TeiLifter` usable without HTTP;
- `TeiEnvelope` binding source hash, backend/version, request fingerprint and
  TEI hash;
- `ConservationDelta` for exact loss accounting;
- typed coordinates, request dialect and backend errors;
- separate raw-TEI and projector caches while retaining legacy lookup.

`da-parse` remains product-side. `ParsedArticle` remains in `da-ports`.
`da-application` must not depend on `da-parse`. Samyama and `kg-*` are excluded.

## Replacement order

1. Make real-TEI fixtures tracked and non-skipping.
2. Split HTTP backend, pure lifter and parser composition.
3. Add conservation inventory and freeze current loss.
4. Lift arXiv idno, header authors, non-table figures and typed coordinates.
5. Implement header and reference native candidate paths.
6. Add neutral layout-token IR and compare Rust PDF/layout donors with pdfalto.
7. Implement segmentation/fulltext/figure/table/formula heads incrementally.
8. Retire a Java stage only after source-bound acceptance evidence.

## CPU and SMP

Adopt ruVector patterns narrowly:

- per-worker arenas and cache-line alignment from `ruvector-core::arena`;
- bounded `ArrayQueue` and cache-padded counters from `lockfree`;
- a private Rayon pool from `ruvector-graph::executor::parallel`;
- measured SIMD/PDX kernels only for dense geometry or feature batches;
- mmap only for proven large replay/evaluation buffers.

Do not pull `ruvector-graph`, Samyama or the ruVLLM serving runtime into the
parse core. Parallelize documents/pages first; preserve ordered TEI walking.
Parser jobs backpressure and never drop. Shadow trajectories may drop only with
explicit counters. Optimization must preserve hashes across 1..N threads.

## SONA and ruVLLM

D294 corrects the earlier consult-only interpretation:

- SONA Loop A/B/C, ReasoningBank, MicroLoRA and EWC++ are active development
  and training scope;
- ruVLLM trajectory, critic and evaluation paths are active scope;
- D136 is not a blanket SONA/LoRA lock;
- automatic promotion remains prohibited.

Learning produces versioned candidates:

```text
parse evidence -> trajectory buffer -> ReasoningBank
 -> MicroLoRA/EWC++ candidate -> ruVLLM critique/ablation
 -> frozen evaluation -> explicit promotion -> WASM policy_hash
```

Candidate evaluation must include train/eval disjointness, same-n baseline,
conservation, task quality, latency/load and rollback evidence. Learned routing
may select backend/admission/retry/concurrency, but every shipped artifact is
versioned and replay-bound.

## Research references

- GROBID principles: https://grobid.readthedocs.io/en/latest/Principles/
- GROBID REST API: https://grobid.readthedocs.io/en/latest/Grobid-service/
- GROBID TEI: https://grobid.readthedocs.io/en/latest/TEI-encoding-of-results/
- GROBID coordinates: https://grobid.readthedocs.io/en/latest/Coordinates-in-PDF/
- GROBID evaluation: https://grobid.readthedocs.io/en/latest/End-to-end-evaluation/
- OmniDocBench: https://arxiv.org/abs/2412.07626
- Docling: https://arxiv.org/abs/2408.09869
- Local GROBID source: `/root/vendor-source/grobid`
- Local SONA source: `/root/vendor-source/ruvector/crates/sona`
- Local ruVLLM source: `/root/vendor-source/ruvector/crates/ruvllm`

## ADR consequences

ADR-075 should be Accepted, because the owner explicitly selected this
direction. It must amend ADR-074's obsolete statement that D136 blocks all
LoRA training. Governor should distinguish:

- DSPy enablement rules;
- active SONA/LoRA candidate work;
- forbidden automatic promotion;
- deterministic parse-core dependency boundaries.
