# ADR-075: Scholarly Parse Plane as a Staged Native Rust Platform

- Status: Accepted
- Date: 2026-08-27
- Deciders: repository owner + agent
- Decisions: D294 (active learning; corrects D293) and D295 (persistence boundary)
- Milestone: M463-snmgcd
- Tags: da-parse, grobid, tei, rust, ruvector, sona, microlora, ruvllm, smp, simd

## 0. One-line decision

**`da-parse` is the product-owned staged scholarly parsing platform: it keeps
Java GROBID as oracle/fallback while native Rust stages replace the cascade
incrementally; ruVector CPU/SMP patterns accelerate deterministic batches, and
SONA/MicroLoRA/EWC++ plus ruVLLM are active learning and evaluation paths whose
artifacts remain candidates until explicit promotion.**

## 1. Context

The ownership cut already moved Java GROBID HTTP, the current TEI lift,
`TeiParseIr` and the `ParserPort` projector into `crates/da-parse`. It preserved
legacy cache identities and onion direction, but the HTTP transport type still
owns pure lift and projection operations. Real-TEI tests can silently skip,
and there is no public conservation API, typed request/error envelope or
provider-neutral backend seam.

GROBID itself is a cascade:

```text
pdfalto -> LayoutToken -> segmentation -> header/names/affiliations
        -> reference segmentation/citation parsing -> fulltext
        -> figures/tables/formulas -> TEI and coordinates -> consolidation
```

No mature Rust project reproduces this whole scholarly contract. PDF-to-text,
Markdown and VLM parsers are useful donors, not TEI-equivalent replacements.
A big-bang rewrite would hide loss and postpone useful improvements.

Earlier project text incorrectly generalized a DSPy-related D136 note into a
blanket LoRA/SONA prohibition. The owner explicitly corrected that
interpretation in D294. SONA and LoRA development and training are open. The
separate no-auto-promote rule remains binding.

## 2. Decision

### 2.1 Ownership and onion

- `da-parse` is the scholarly product crate; `kg-parse` is rejected.
- `ParsedArticle` and parsing ports remain in `da-ports`.
- `da-application` does not take a production dependency on `da-parse`.
- `da-adapters` may retain the compatibility re-export and infrastructure
  stage during the migration window.
- Samyama and direct `kg-*` dependencies are forbidden in `da-parse`.
- Deterministic `da-parse` does not directly embed SONA, ruVLLM,
  `ruvector-graph` or an LLM serving runtime. Learning is injected through
  typed ports/adapters and immutable policy artifacts.

### 2.2 Platform contracts

The crate will separate transport, deterministic lift and projection:

```rust
trait ParseBackend {
    async fn fulltext(&self, request: &FulltextRequest)
        -> Result<TeiEnvelope, BackendError>;
}

trait TeiLifter {
    fn lift(&self, envelope: &TeiEnvelope, context: &ParseContext)
        -> Result<TeiParseIr, TeiLiftError>;
}

struct ScholarlyParser<B, L> {
    backend: B,
    lifter: L,
}
```

Required types:

- `FulltextRequest` with versioned consolidation and coordinate dialect;
- `TeiEnvelope` binding source hash, backend identity/version, request
  fingerprint, output hash and timing;
- `BackendError` preserving busy/503, no-content, invalid input, limits,
  timeout, tagging, parsing and conversion failures;
- `ConservationDelta` describing every lost, changed or newly lifted family;
- typed page-coordinate boxes and coordinate-frame metadata.

### 2.3 Compatibility and cache

- Historical cache identities remain readable, including
  `da_adapters::GrobidParser@load-v1` and the production TEI-navigation
  contract.
- Raw `TeiEnvelope` cache and projected `ParsedArticle` cache become distinct.
- A projector fix may replay from raw TEI without calling Java.
- Every cache and policy artifact binds configuration and schema fingerprints.

### 2.4 Conservation-first replacement

A native stage is a candidate backend until it passes:

1. non-skipping tracked fixtures;
2. family-level TEI conservation;
3. structure and field-quality metrics;
4. deterministic replay across thread counts;
5. latency, memory, load and failure-path tests;
6. explicit promotion decision.

Replacement is per stage and may be per document class. Java GROBID remains
fallback until the corresponding stage is accepted. Entity extraction F1 is
supporting evidence, not the parse acceptance oracle.

### 2.5 Native sequence

1. tracked TEI fixtures and public conservation inventory;
2. backend/lifter/projector split;
3. P0 TEI lift: declared arXiv idno, header authors, non-table figures and
   typed coordinates;
4. typed fulltext/header/reference protocol and failure contract;
5. native header and reference candidate heads;
6. neutral `LayoutToken` lattice and Rust PDF/layout bake-off;
7. native segmentation/fulltext/figure/table/formula heads;
8. staged Java retirement when evidence permits.

### 2.6 CPU and SMP

ruVector is a pattern and implementation donor, not a dependency umbrella.
The parse runtime may adopt narrowly:

- per-worker arenas and cache-line alignment;
- bounded crossbeam queues and cache-padded counters;
- private Rayon pools and explicit partitioning;
- measured AVX2/AVX-512/NEON kernels for dense geometry/features;
- mmap for large replay/evaluation buffers when measurement justifies it.

Rules:

- parallelize documents/pages before ordered TEI walking;
- parser tasks backpressure and are never dropped;
- only shadow learning trajectories may be dropped, with telemetry;
- outputs must be byte/semantic stable across 1..N workers;
- no fake NUMA claim and no unsafe/SIMD adoption without benchmark and scalar
  equivalence proof;
- no global Rayon pool capture by the parse plane.

### 2.7 SONA, MicroLoRA, EWC++ and ruVLLM

These are active implementation and training scope:

- SONA trajectories record parse decisions and outcomes.
- ReasoningBank extracts reusable patterns.
- MicroLoRA candidates learn grammar routing, admission, retry and concurrency
  policy.
- EWC++ consolidates policies across document domains.
- ruVLLM provides richer trajectory/verdict representations, critics and
  ablation evaluation.
- accepted policy artifacts are packaged as versioned WASM modules and stamped
  with `policy_hash`.

Training and experimentation do not require a D136 unlock. D136 is not a
blanket SONA/LoRA lock. DSPy enablement remains governed separately.

The following remain forbidden:

- automatic promotion;
- self-confirming evaluation;
- train/eval contamination;
- unstamped mutable policy state;
- learned code directly changing graph-import authority;
- learned code bypassing deterministic conservation evidence.

Promotion requires frozen source-bound evidence, same-n baseline/candidate
comparison, dual task metrics, validation-gap checks, operational evidence and
an explicit decision.

### 2.8 GROBID-compatible surface

The required compatibility subset is:

- `GET /api/isalive`, `/health`, `/version`;
- `POST /api/processFulltextDocument`;
- `POST /api/processHeaderDocument`;
- `POST /api/processReferences`.

Patent and training endpoints are not compatibility requirements. A future
facade uses a port distinct from Java `:8070` and graph RESP `:16379`. Busy
capacity remains an honest 503 with retry metadata.

### 2.9 Persistence and truth (D295)

- Samyama remains the sole authoritative scientific KG store.
- RVF is the canonical durable experience store for trajectories,
  ReasoningBank, MicroLoRA/EWC++ checkpoints, policy history and rollback.
- CVF is a trajectory interchange/event schema persisted into RVF, not a
  competing authority.
- The existing content-addressed pipeline cache stores derived raw TEI,
  `ParsedArticle`, conservation and benchmark artifacts.
- redb is permitted only as a rebuildable cache index, batch cursor or
  checkpoint accelerator. Deleting redb must lose no unique state.
- TEI, native outputs and policies remain candidate evidence;
  `import_eligible=false` remains unchanged unless separately authorized.

### 2.10 Batch and cache contract

When GROBID lacks the required batching surface, the product provides a
bounded resumable batch API. Every input receives a typed result or error;
partial failure does not erase successful work. Queue capacity, backpressure,
checkpoint and retry state are observable.

Raw TEI and projected article caches support `off`, `read-only`,
`write-through` and `refresh`. A cache mode may change cost and latency but not
result semantics. Historical identities remain readable.

### 2.11 Configuration and meta prompt principle

Physical paths, endpoint URLs, cache mode, batch size, worker/queue limits,
RVF and redb locations are supplied through typed configuration at the
composition root. Inner layers receive interfaces and values, not environment
lookups or path defaults. `.env.example` documents supported non-secret keys;
real environment values are not committed.

Stages, API compatibility, metrics, thresholds, route choices, cache modes and
promotion rules live in schema-validated DATA. Rust provides generic execution
and validation rather than duplicated hardcoded policy lists.

## 3. Quality and proof contract

Required metrics:

- TEI family conservation and source-span retention;
- header, author, affiliation, reference and citation field accuracy;
- reading order, section hierarchy, table, formula and figure coverage;
- coordinate parse/roundtrip properties;
- output hash stability across replay and worker counts;
- p50/p95/p99 latency, throughput, RSS, allocation and queue-wait profiles;
- malformed input, 204, 503, timeout, limit and conversion failures;
- learning train/eval disjointness, ablation lift, lineage and rollback.

Testing follows ADR-051 contours: pure unit/property/serde, backend contracts,
real fixture integration, system journey, architecture rules and load tests.

## 4. Consequences

Positive:

- a coherent Rust destination exists without pretending the Java cascade can
  be replaced at once;
- pure lift, caching and evaluation become independently testable;
- CPU optimization has a deterministic benchmark surface;
- SONA and ruVLLM can advance without contaminating the parse baseline;
- every replacement and learned policy is replayable and reversible.

Costs:

- more explicit types and stage contracts;
- dual Java/native operation during migration;
- frozen corpora and benchmark maintenance;
- native and WASM build/test matrices;
- promotion governance remains deliberate rather than automatic.

## 5. Alternatives rejected

- `kg-parse`: violates product/universal dependency direction.
- Java GROBID fork or JNI embedding: preserves Java operational identity and
  makes staged replacement harder.
- big-bang native rewrite: no credible conservation proof.
- Markdown/VLM output as scholarly TEI truth: loses contract semantics.
- full `ruvector-core`, `ruvector-graph` or ruVLLM runtime inside `da-parse`:
  dependency and responsibility inflation.
- global Rayon and unbounded queues: hidden capacity and interference.
- consult-only forever: contradicts D294 and prevents the intended active SONA
  learning path.
- automatic candidate promotion: removes independent acceptance authority.

## 6. Governance

Governor must:

- expose a `scholarly_parse_plane` dimension;
- enforce parse-core dependency boundaries;
- keep DSPy rules distinct from SONA/LoRA work;
- reject auto-promote and unstamped production policy claims;
- require this ADR and research evidence for staged-platform readiness;
- include negative fixtures proving violations are caught.

M463 retains research evidence. M464-z84in2 owns nineteen fine-grained
implementation slices from API ledger and fixtures through native fulltext,
persistent learning and governor closure. Completion evidence is recorded
through GSD lifecycle tools, not manual edits to planning artifacts.

## 7. Executable governance traceability

The binding executable contract is
`scripts/governor/checks/scholarly_parse_plane.yaml`, with its source-bound
positive and isolated negative fixtures under
`governor/fixtures/scholarly_parse_plane/`. The adapter
`scripts/governor/adapters/checks/scholarly_parse_plane.py` is registered by
`TrajectoryService.build_report`, so the same check runs for both active and
closeout trajectory reports.

The checker proves repository-relative source and canonical-artifact identity,
Cargo/persistence/configuration/cache/quarantine evidence, rollback identity,
closed wire vocabulary, and required integration evidence. It reports typed
high-severity flags containing the plane, violation code, evidence path,
requirement anchor, and expected/observed state; it does not scan comments or
negative fixtures as source code and it does not duplicate the shared
promotion FSM.

Closure remains candidate-only: `candidate`, `deployed=false`,
`auto_promote=forbidden`, `promotion_receipt=forbidden`,
`import_eligible=false`, `production_route=forbidden`, and `graph_write=forbidden`.
The checker is governance evidence, not authorization: it never creates a
Promotion receipt, reloads a Wasm policy, writes the graph, imports data, or
activates a production route.

## 8. References

- Research synthesis:
  `doc/research/m463-grobid-rust-ruvector-platform.md`
- Full CE evidence:
  `.compound-engineering/artifacts/research/2026-08-27-grobid-rust-ruvector-platform-research.md`
- GROBID principles: https://grobid.readthedocs.io/en/latest/Principles/
- GROBID REST: https://grobid.readthedocs.io/en/latest/Grobid-service/
- GROBID evaluation: https://grobid.readthedocs.io/en/latest/End-to-end-evaluation/
- OmniDocBench: https://arxiv.org/abs/2412.07626
- Docling: https://arxiv.org/abs/2408.09869
- Local sources: `/root/vendor-source/grobid`,
  `/root/vendor-source/ruvector/crates/{ruvector-core,sona,ruvllm}`
