---
title: GROBID-like Rust Parse Platform on ruVector SONA and ruVLLM
type: architecture-research
date: 2026-08-27
topic: da-parse-staged-platform
status: reviewed
milestone: M463-snmgcd
decision: D294
supersedes:
  - consult-only interpretation in 2026-08-27-da-parse-elegance-vendor-gap-assessment.md
---

# GROBID-like Rust parse platform on ruVector, SONA and ruVLLM

## Executive conclusion

The target is not a Rust port of the GROBID Java repository and not another
sidecar facade. It is a product-owned staged scholarly parse platform in
`crates/da-parse` that gradually replaces GROBID functions with native Rust
stages while retaining Java GROBID as an oracle and fallback until each stage
passes conservation, quality and performance gates.

The implementation has four cooperating planes:

1. **Deterministic parse plane** — typed request, backend, TEI envelope, pure
   lifter, projector and conservation inventory.
2. **Native CPU plane** — layout-token lattice and specialist stages using
   bounded SMP, per-worker arenas and measured SIMD where it helps.
3. **Learning plane** — SONA trajectories, ReasoningBank, MicroLoRA and EWC++
   plus ruVLLM training/critic workflows. This is active implementation and
   experimentation scope under D294.
4. **Promotion plane** — frozen evaluation and explicit candidate promotion.
   Training is allowed; automatic promotion is not.

`D136` must not be interpreted as a blanket SONA or LoRA prohibition. The
binding safety rule is candidate status plus no auto-promote. DSPy remains a
separate concern.

## Evidence reviewed

### Project and Compound Engineering

- `.compound-engineering/STRATEGY.md`
- `.compound-engineering/artifacts/ideation/2026-08-26-pure-rust-scholarly-parse-crate-ideation.html`
- `.compound-engineering/artifacts/plans/2026-08-26-1515-refactor-da-parse-crate-plan.md`
- `.compound-engineering/artifacts/research/2026-08-26-scholarly-parse-mcp-grounding.md`
- `.compound-engineering/artifacts/research/2026-08-27-da-parse-elegance-vendor-gap-assessment.md`
- `doc/research/m463-grobid-parse-plane.md`
- `doc/research/m463-grobid-tei-lift-matrix.md`
- `artifacts/research/m463/tei-inventory-2608.22118.json`
- `artifacts/research/m463/tei-inventory-2507.19457.json`

### Local vendor source

- GROBID: `/root/vendor-source/grobid`
- `grobid-rs`: `/root/vendor-source/grobid-rs`
- `grobid-papers`, `scipdf_parser`, `openalex-pdf-parser`
- `opendataloader-pdf`, `structure-vision`, `pdffigures2`, `pdf-craft`, `dedoc`
- ruVector: `/root/vendor-source/ruvector/crates/ruvector-core`
- SONA: `/root/vendor-source/ruvector/crates/sona`
- ruVLLM: `/root/vendor-source/ruvector/crates/ruvllm`
- RVF: `/root/vendor-source/ruvector/crates/rvf`

### External primary references

- GROBID principles and cascade:
  https://grobid.readthedocs.io/en/latest/Principles/
- GROBID REST API and parallel service behavior:
  https://grobid.readthedocs.io/en/latest/Grobid-service/
- TEI encoding:
  https://grobid.readthedocs.io/en/latest/TEI-encoding-of-results/
- PDF coordinates:
  https://grobid.readthedocs.io/en/latest/Coordinates-in-PDF/
- Consolidation:
  https://grobid.readthedocs.io/en/latest/Consolidation/
- End-to-end evaluation and publisher corpora:
  https://grobid.readthedocs.io/en/latest/End-to-end-evaluation/
- OmniDocBench, CVPR 2025:
  https://arxiv.org/abs/2412.07626
- Docling technical report:
  https://arxiv.org/abs/2408.09869
- GROBID organization and pdfalto project:
  https://github.com/grobidOrg

Web queries used:

- `GROBID 0.9 architecture pdfalto LayoutToken CRF cascade TEI service processFulltextDocument 503 official documentation`
- `GROBID end-to-end evaluation dataset PMC bioRxiv PLOS eLife sciencialab grobid-evaluation metrics official`
- `2025 scientific document parsing benchmark OmniDocBench Docling MinerU Marker reading order table formula PDF`
- `Rust PDF text extraction layout analysis CPU parallel SIMD pdf_oxide oxidize-pdf benchmark`

## What GROBID actually contains

GROBID is a cascade rather than one model:

```text
PDF
  -> pdfalto visual and layout extraction
  -> LayoutToken lattice and lexical/layout features
  -> document segmentation
  -> header, names, affiliations and dates
  -> reference segmentation and citation parsing
  -> full-text structure
  -> figure, table and formula recognition
  -> TEI formatting with optional coordinates
  -> optional bibliographic consolidation
```

Its production value comes from the composition of multiple sequence models,
TEI encoding, error semantics, service pooling and evaluation corpora. A Rust
replacement must therefore advance stage by stage. Replacing only PDF-to-text
or Markdown does not reproduce GROBID.

Important protocol facts to preserve:

- `processFulltextDocument`, `processHeaderDocument` and
  `processReferences` are separate useful endpoints.
- consolidation modes are a dialect, not booleans only (`0|1|2`, and header
  supports mode `3`).
- `teiCoordinates` applies to multiple TEI element families.
- pool saturation is an honest `503`, not an internal retry-until-success.
- structural errors such as `NO_BLOCKS`, `TOO_MANY_TOKENS`, timeout,
  tagging/parsing and pdfalto failures are distinct outcomes.

## Current `da-parse` state

The ownership migration is correct and already shipped:

- Java GROBID HTTP, current TEI lift, `TeiParseIr` and the `ParserPort`
  projector live in `crates/da-parse`.
- `ParsedArticle` remains in `da-ports`.
- `da-application` has no production dependency on `da-parse`.
- `da-adapters` keeps a temporary parser/profile re-export and owns
  `GrobidParseStage`.
- historical cache identities remain byte-identical.
- Samyama and `kg-*` are absent from direct `da-parse` dependencies.

The current inner shape is not yet the target platform:

- HTTP, IR construction, lift and projection remain methods on
  `GrobidParser`.
- `lift.rs` remains a large implementation island.
- no public zero-I/O TEI lifter or conservation API exists.
- request and error dialects are under-typed.
- real-TEI tests skip when their stale fixture path is absent.
- raw TEI and projected-article cache identities are not separate.

## Target architecture

```text
ParserPort
  |
  v
ScholarlyParser<B: ParseBackend, L: TeiLifter>
  |-- FulltextRequest and profile fingerprint
  |-- bounded endpoint admission and retry budget
  |-- raw TeiEnvelope cache
  |-- deterministic TeiLifter
  |-- conservation_inventory
  `-- ParsedArticle projector and schema fingerprint

ParseBackend
  |-- GrobidHttpBackend       current oracle and fallback
  |-- CachedTeiBackend        replay and tests
  |-- NativeHeaderBackend     staged Rust head
  |-- NativeReferenceBackend  staged Rust head
  `-- NativeFulltextBackend   late staged replacement

LearningPlane
  |-- SONA trajectory capture and ReasoningBank
  |-- MicroLoRA and EWC++ candidate training
  |-- ruVLLM critic and trajectory synthesis
  |-- WASM policy packaging and policy_hash
  `-- frozen evaluation and explicit promotion receipt
```

Core contracts:

```rust
trait ParseBackend {
    async fn fulltext(&self, request: &FulltextRequest)
        -> Result<TeiEnvelope, BackendError>;
}

trait TeiLifter {
    fn lift(&self, envelope: &TeiEnvelope, context: &ParseContext)
        -> Result<TeiParseIr, TeiLiftError>;
}

fn conservation_inventory(
    source: &TeiEnvelope,
    lifted: &TeiParseIr,
) -> ConservationDelta;
```

The learning plane must consume typed observations and deltas. It must not
mutate the deterministic lifter in place without producing a versioned
candidate artifact and evaluation record.

## Rust replacement sequence

### Stage 0: make existing proof real

- Track compact TEI fixtures under `crates/da-parse/tests/fixtures/`.
- Missing fixtures fail instead of silently returning.
- Freeze current table, formula, citation and hierarchy behavior.

### Stage 1: platform seams

- Split `GrobidHttpBackend`, `DeterministicTeiLifter` and
  `ScholarlyParser<B,L>`.
- Add typed `FulltextRequest`, consolidation enum, `TeiEnvelope`, backend
  identity and error taxonomy.
- Separate raw TEI cache from projector cache while preserving legacy lookup.

### Stage 2: TEI conservation and P0 lift

- Public deterministic `conservation_inventory`.
- Lift arXiv idno with declared version, header authors, non-table figures and
  typed coordinates.
- Preserve typed missingness and article revision rules.

### Stage 3: cheap native specialist paths

- Implement native header/name/date and reference segmentation/parsing as
  independent candidate backends.
- Compare each candidate with Java TEI on publisher and local gold corpora.
- Promote per endpoint or per document class, not as one global switch.

### Stage 4: neutral layout-token lattice

- Introduce `LayoutToken { text, page, bbox, font, style, line, block }`.
- Bake off Rust PDF donors against pdfalto and existing XY-Cut.
- Preserve reading order, coordinate frames and source spans.

### Stage 5: segmentation and full-text native heads

- Add staged sequence heads for segmentation, full text, figures, tables and
  formulas.
- Maintain GROBID output as oracle/fallback until conservation and latency
  contracts pass.

## CPU and SMP design

Use ruVector as a source of proven patterns, not as a dependency umbrella.
Relevant local donors include:

- `ruvector-core/src/arena.rs`: per-worker bump arenas and cache alignment.
- `ruvector-core/src/lockfree.rs`: bounded `ArrayQueue`, dropped telemetry and
  cache-padded counters.
- `ruvector-core/src/simd_intrinsics.rs`: measured AVX2/AVX-512/NEON kernels.
- `ruvector-core/src/pdx.rs`: lane-major batch layout.
- `ruvector-graph/src/executor/parallel.rs`: private Rayon thread pool.
- `ruvector-gnn/src/mmap.rs`: mmap-backed batch buffers.

Rules for parse:

1. Parallelize across documents and pages before parallelizing ordered XML
   walking.
2. Use a private bounded Rayon pool; do not consume the global pool.
3. Parser work is backpressured, never dropped. Only shadow learning
   trajectories may be dropped with counters.
4. Give each worker its own arena to avoid locks and false sharing.
5. Use SIMD only for measured dense geometry/features, not because XML parsing
   is labelled CPU-heavy.
6. Do not adopt the current ruVector `NumaAllocator` as NUMA proof; its node
   detection is a heuristic, not topology binding.
7. Unsafe arena/SIMD code requires property tests, Miri where applicable and
   scalar equivalence tests.

Benchmark matrix:

- documents: 1, 8, 32, 128, full corpus;
- threads: 1, 2, 4, 8, physical cores;
- page/layout complexity buckets;
- p50/p95/p99 latency, throughput, peak RSS, allocations, queue wait;
- determinism: identical envelope and projected hashes across thread counts;
- 10x load breakpoint and 503/backpressure behavior.

Optimization is accepted only when conservation is unchanged and the measured
workload improves. No unmeasured arena/SIMD import.

## SONA and ruVLLM active path

D294 explicitly opens implementation and training:

### SONA

- Capture `QueryTrajectory`/parse trajectories into bounded buffers.
- Extract ReasoningBank patterns with K-means++.
- Train MicroLoRA candidates for grammar route, admission, retry and
  concurrency policy.
- Use EWC++ for cross-domain consolidation.
- Package selected policies as versioned WASM artifacts with `policy_hash`.

### ruVLLM

- Use the richer trajectory/verdict representation for parse failure analysis.
- Run reflective critics over `ConservationDelta`, typed backend errors and
  latency evidence.
- Use the evaluation harness for ablations: baseline, retrieval only,
  MicroLoRA policy and combined policy.
- Persist candidates and lineage, not authoritative graph state.

### Promotion boundary

Training and candidate creation are active. The following remain forbidden:

- automatic production promotion;
- self-evaluation as sole authority;
- evaluation/train corpus contamination;
- changing graph-import authority;
- silently changing cache or policy identity.

A policy or native stage promotion requires source-bound frozen evidence,
same-n baseline/candidate comparison, conservation metrics, task-specific
quality metrics, latency/load evidence and an explicit promotion decision.

## Quality model

Entity extraction F1 is not a parse-plane gate by itself. Required classes:

- **Conservation:** TEI family counts, text/span retention, references,
  callouts, figures, tables, formulas and coordinate coverage.
- **Structure:** section hierarchy, reading order, header and reference field
  accuracy.
- **Approximate quality:** strict/soft/Levenshtein/Ratcliff-Obershelp field
  matching, table and formula metrics where applicable.
- **Determinism:** identical results across replay and thread-count changes.
- **Operations:** p95 latency, queue wait, 503 rate, retry budget and capacity.
- **Learning:** train/eval disjointness, candidate lineage, ablation lift,
  policy stability and rollback.

Use the GROBID publisher corpora and the local TEI fixtures. OmniDocBench is a
layout/Markdown comparison corpus, not a replacement for scholarly TEI
conservation.

## Decisions and rejected alternatives

Accepted:

- `da-parse` is the product crate.
- Native stages advance independently behind conservation gates.
- SONA/LoRA/ruVLLM implementation and training proceed now.
- ruVector CPU patterns are adopted narrowly and benchmark-first.
- Java GROBID remains oracle/fallback until evidence permits removal.

Rejected:

- `kg-parse` product inversion;
- big-bang Java-to-Rust rewrite;
- embedding the `grobid-rs` JNI engine;
- treating MinerU, Docling or Markdown as TEI truth;
- putting Samyama, `ruvector-graph` or a full ruVLLM serving runtime inside
  deterministic `da-parse`;
- fake NUMA and unmeasured SIMD;
- auto-promote.

## Persistence amendment D295

- Samyama is the authoritative scientific KG store only.
- RVF is canonical for unique trajectories, ReasoningBank, MicroLoRA/EWC++
  checkpoints, policy history and rollback.
- CVF is the trajectory interchange schema persisted into RVF.
- Existing pipeline cache stores derived raw TEI, projected article and
  comparison artifacts with configurable off/read-only/write-through/refresh
  modes.
- Optional redb stores only rebuildable cache indexes and batch cursors; it
  must contain no unique state.
- Paths and capacities come from typed composition-root configuration; stage,
  API, metric and promotion rules remain schema-validated DATA.

## Governor and GSD implications

Governor must:

- track a `scholarly_parse_plane` dimension;
- forbid Samyama, `kg-*`, `ruvector-graph`, SONA and ruVLLM runtime dependencies
  in deterministic `da-parse`;
- allow learning through injected ports/adapters and versioned policy
  artifacts;
- keep DSPy-specific rules separate from SONA/LoRA;
- reject auto-promote and unbound policy artifacts;
- require ADR-075 and evidence paths before claiming staged-platform readiness.

M463 should be reassessed into implementation slices for platform contracts,
protocol/error semantics, native CPU/SMP substrate, active SONA/ruVLLM learning
and governor/promotion closure.
