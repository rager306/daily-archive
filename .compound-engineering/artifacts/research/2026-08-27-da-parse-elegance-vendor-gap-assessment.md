---
title: da-parse Elegance and Vendor Gap Assessment
type: research-review
date: 2026-08-27
topic: da-parse-crate
sources:
  - .compound-engineering/artifacts/ideation/2026-08-26-pure-rust-scholarly-parse-crate-ideation.html
  - .compound-engineering/artifacts/research/2026-08-26-scholarly-parse-mcp-grounding.md
  - .compound-engineering/artifacts/plans/2026-08-26-1515-refactor-da-parse-crate-plan.md
  - doc/research/m463-grobid-parse-plane.md
  - doc/research/m463-grobid-tei-lift-matrix.md
  - crates/da-parse
  - /root/vendor-source/grobid-rs
  - /root/vendor-source/grobid-papers
  - /root/vendor-source/openalex-pdf-parser
  - /root/vendor-source/scipdf_parser
  - /root/vendor-source/dedoc
  - /root/vendor-source/pdf-craft
  - /root/vendor-source/opendataloader-pdf
  - /root/vendor-source/structure-vision
  - /root/vendor-source/pdffigures2
  - /root/vendor-source/ruvector/crates/sona
  - /root/vendor-source/ruvector/crates/ruvllm
status: reviewed
---

# da-parse: elegance and vendor gap assessment

## Executive verdict

The delivered `da-parse` is an **elegant ownership migration** but not yet the
parse platform described by Compound Engineering ideation.

| Axis | Score | Verdict |
|---|---:|---|
| Crate/onion boundary | 9/10 | Correct product-side crate; application remains on ports; no Samyama or direct kg-* dependency. |
| Compatibility migration | 9/10 | Cache identities preserved byte-for-byte; composition roots moved without changing article semantics. |
| Internal separation | 6/10 | Files are separated, but pure TEI lift and projection remain methods on the HTTP transport type `GrobidParser`. |
| Extensibility toward native stages | 4/10 | No `ParseBackend`, `TeiLifter`, `ParseStage`, conservation API, or neutral layout-token IR; native stages cannot plug in without another refactor. |
| Failure semantics / operability | 4/10 | HTTP errors are flattened; no 503/backpressure/retry budget/health/capacity surface. Empty HTTP-200 TEI still succeeds by compatibility decision. |
| Conservation proof | 3/10 | P0 loss is documented but not gated; real-TEI tests silently return when their stale fixture path is missing. |
| Overall, as the scoped CE ownership cut | **8/10** | Small, safe, reversible, low dependency cost. |
| Overall, as the CE ideation end-state | **5/10** | The boundary exists, but most control-plane and native-cascade seams are absent. |

The main distinction is important: most functional gaps were **not accidental**
inside the implementation plan. The implementation-ready CE plan explicitly
limited authority to crate extraction and deferred conservation, P0 tissue
lift, REST expansion, cache split, layout bake-off, and native stages. The
problem is that the delivered inner shape does not preserve all of the seams
needed by those deferred ideas.

## What was done elegantly

1. **Correct ownership and dependency direction.** `da-parse` depends on
   `da-ports`; `da-application` does not depend on `da-parse`; `kg-*` remains
   universal. This matches CE KTD4/KTD6 and avoids a false `kg-parse` product.
2. **Compatibility-first cutover.** KTD2 preserved both historical cache
   dialects, including `da_adapters::GrobidParser@load-v1`. This prevents a
   corpus-wide cold start for a cosmetic rename.
3. **Honest intermediate type.** `TeiParseIr` is distinct from
   `ParsedArticle`; projection is explicit. The first cut does not pretend that
   authors/idno/figures were lifted when they were not.
4. **Small direct dependency surface.** `da-ports`, async-trait, reqwest,
   tokio, tracing, sha2, hex. No JNI, Java embedding, Samyama, external DB, or
   LLM runtime entered the crate.
5. **Compatibility re-export instead of flag-day migration.** The four-line
   `da-adapters::grobid_parser` re-export is a controlled bridge, while the HTTP
   implementation has one owner.
6. **Observability was retained.** The projector emits PDF/TEI sizes, elapsed
   time, section depth, citations, table cells, formulas, hash, and typed
   missingness.
7. **The scope did not smuggle in speculative functionality.** No auto-promote,
   TEI-as-graph-truth, MinerU-as-TEI, JNI GROBID, LoRA, or ruVLLM hot path.

## Where the implementation is not elegant

### 1. Transport type owns pure domain-free operations

`ir.rs`, `lift.rs`, and `projector.rs` all implement methods on
`crate::http::GrobidParser`. Thus `GrobidParser` simultaneously means:

- HTTP endpoint/client/profile;
- TEI parser/lifter;
- IR factory;
- `ParsedArticle` projector;
- `ParserPort` orchestration.

The files are separated physically, but the type-level coupling remains. A
native TEI backend, cached TEI blob, test fixture, or future facade cannot use a
public pure lifter without going through or impersonating the HTTP type.

A cleaner inner shape is:

```rust
trait ParseBackend {
    async fn fulltext(&self, request: FulltextRequest<'_>)
        -> Result<TeiEnvelope, BackendError>;
}

trait TeiLifter {
    fn lift(&self, envelope: &TeiEnvelope, context: &ParseContext)
        -> Result<TeiParseIr, TeiLiftError>;
}

struct ScholarlyParser<B, L> { backend: B, lifter: L }
```

`GrobidHttpBackend` then owns HTTP only; `DeterministicTeiLifter` owns TEI;
`ScholarlyParser` implements `ParserPort`.

### 2. `lift.rs` is still a large single implementation island

KTD9 rejected a 3.5k-line dump. The result is better, but `lift.rs` still has
~1,867 lines under one `impl GrobidParser`. CE ideation and `grobid-rs` both
point toward isolated walkers:

- header/idno/authors;
- body/section hierarchy;
- references/callouts;
- figures/tables/formulas;
- coordinate parsing;
- shared XML utilities.

`grobid-rs/src/format/tei/parser.rs` provides a useful `SectionParser` pattern,
and its header/body/references files use `quick_xml` streaming. The engine/JNI
portion must not be adopted, but the TEI walker architecture is a strong donor.

### 3. The crate public API is too narrow for its declared future

Public exports are only `GrobidParser`, `GrobidProfile`, and `TeiParseIr`.
There is no public:

- lift raw TEI without HTTP;
- conservation inventory;
- backend abstraction;
- parse-stage abstraction;
- typed coordinate parser;
- typed GROBID request/response/error model.

This makes `da-parse` an extracted adapter, not yet a provider-neutral staged
platform.

### 4. Real-TEI proof silently does not run

`crates/da-parse/src/real_tei_tests.rs` still joins
`data/article_corpora/m033-grobid-probe-v1/...` and returns early when missing.
That path does not exist. The fixtures exist only in archived locations.
Therefore the advertised real-TEI locks for bibliography, tables, formulas,
and hierarchy can pass without executing.

The correct fix is not another untracked `data/corpora` copy. Put compact,
tracked TEI fixtures under `crates/da-parse/tests/fixtures/` (compressed if
needed), and make absence a test failure. This is prerequisite to claiming a
conservation gate.

### 5. Configuration and errors are not protocol-complete

`GrobidProfile` uses two booleans, while GROBID consolidation is an enum
(`0|1|2`, and header also `3`). The current fingerprint cannot represent future
`segmentSentences`, `includeRawAffiliations`, `includeRawCitations`,
`generateIDs`, `start/end`, or funder/copyright options without another shape
change.

`http.rs` flattens every non-2xx into `ParseFailed("GROBID HTTP ...")` and
transport failures into `Unavailable`. It loses:

- pool-busy 503 and `Retry-After`;
- 204 / no content;
- BAD_INPUT_DATA / NO_BLOCKS / TOO_MANY_BLOCKS / TOO_MANY_TOKENS;
- TIMEOUT / TAGGING_ERROR / PARSING_ERROR / PDFALTO failure;
- endpoint/profile/attempt budget.

### 6. Raw coordinates remain strings

Vendor `grobid-papers` models `PdfLocation` and typed bounding boxes;
`structure-vision` resolves page geometry from TEI surfaces. Our tables and
formulas preserve coordinate strings. This is conservative but not elegant for
figure crops, XY-Cut reconciliation, or provenance.

### 7. Cache compatibility was preserved, but cache architecture was not created

The cut correctly preserved existing application/stage cache identities.
However CE ideation's stronger design—raw TEI blob cache separate from projector
schema cache—was deferred. That means a projector fix still risks reparsing PDF
through Java on uncached entry points, and output-schema changes cannot replay
from a durable TEI envelope everywhere.

## CE ideation versus implementation plan

### Explicitly researched and intentionally deferred

These must not be labeled accidental misses:

1. `conservation_inventory(gold_tei, lifted) -> Result<Delta, ConservationFault>`;
2. P0 lift: article arXiv idno, header authors, non-table figures;
3. figure/table callout binding and typed coordinates;
4. GROBID-compatible REST control surface;
5. raw TEI blob cache separated from projector cache;
6. neutral LayoutToken lattice and Rust extractor bake-off;
7. N specialist heads + FSM orchestration;
8. SONA on specialist heads and ruVLLM only on train/debug critic path;
9. HTML/XY-Cut migration into the parse crate;
10. native replacement of Java stages only after conservation passes.

The implementation plan narrowed authority deliberately to U1-U5 ownership
extraction. That was a sound delivery choice.

### Actually under-specified or missed in the final plan

1. **Pure lifter/backend interfaces.** CE ideation described `ParseStage` and a
   provider-neutral platform, but the implementation plan did not require a
   backend trait or public zero-IO TEI lifter.
2. **Fixture liveness.** The plan explicitly allowed skipped real-TEI fixtures,
   leaving the most important characterization tests non-binding.
3. **Typed transport outcome.** Research listed GROBID errors and 503 semantics,
   but the ownership plan froze old generic error behavior without reserving a
   typed error envelope.
4. **Extensible request dialect.** The plan froze byte-identical fingerprints
   but did not introduce a versioned request contract capable of later fields.
5. **Backpressure boundary.** R085 said own admission/backpressure, yet neither
   plan nor crate reserved a pool/permit interface.
6. **Policy lineage artifact.** `policy_hash` exists in grammar routing and
   profile fingerprint exists in parse, but no durable parse envelope binds
   source hash + backend/version + request fingerprint + TEI output hash +
   projector schema fingerprint.
7. **Stage-scoped performance interfaces.** CE ideation invoked ruVector arenas,
   SIMD, SMP, and paged ideas, but no neutral layout-feature batch contract was
   introduced, so those capabilities have nowhere clean to attach later.

## Vendor patterns worth adopting

### P0 — before adding more functionality

1. **Pure TEI lifter split from HTTP.** Borrow the walker/module pattern from
   `grobid-rs/src/format/tei/*`, not its JNI engine.
2. **Tracked real-TEI fixtures with non-skipping tests.** Enable current locks
   before claiming migration proof.
3. **Public conservation inventory.** This was CE's second-ranked idea and the
   prerequisite for P0 lift, native stages, SONA learning, and ruVLLM criticism.
4. **Typed protocol contract.** Add `FulltextRequest`, consolidation enums,
   `TeiEnvelope`, and `GrobidHttpError`; include every semantic request field in
   a versioned fingerprint.

### P1 — protocol and operations

5. **Bounded concurrency and backpressure.** Adapt ruVLLM's concepts—not its LLM
   token machinery—into endpoint permits/queue depth/capacity metrics. Fulltext,
   header, and references need distinct budgets. A semaphore plus bounded queue
   is likely enough; importing ruVLLM would be overkill.
6. **503-aware retry budget and circuit state.** Retry only retryable busy
   responses within an explicit attempt/elapsed budget. Do not hide structural
   errors or Crossref enrichment failures. Tiny-dancer's state machine is a
   donor, not a dependency requirement.
7. **Typed coordinates.** Adapt `grobid-papers` bounding-box parsing and TEI
   surface/page dimensions.
8. **Header and references endpoints.** `grobid-rs`, scipdf, and OpenAlex parser
   show that cheap header parsing and bibliography salvage are first-class
   paths, not flags on fulltext only.
9. **Raw TEI cache envelope.** Borrow `grobid-rs` content-addressed cache key
   discipline, but use project-authorized storage and existing pipeline cache
   contracts; do not introduce S3 or an external DB.

### P2 — native cascade preparation

10. **Neutral layout-token batch IR.** CE ideation correctly identified
    GROBID LayoutToken/pdfalto and ODL XY-Cut as donors. Only after the IR and
    conservation gate exist should ruVector arenas/SIMD/SMP be introduced.
11. **Bounded trajectory buffer with dropped telemetry.** SONA's
    `ArrayQueue<QueryTrajectory>` plus dropped counters is a good pattern for
    read-only parse-routing observations. It belongs after parse, not inside the
    deterministic TEI lift.
12. **ReasoningBank plus active MicroLoRA/EWC++ learning.** `find_similar` may
    advise grammar route, admission, concurrency, or retry policy; D294 also
    authorizes pattern insertion, candidate training and versioned WASM policy
    packaging. Candidates remain non-authoritative until frozen evaluation and
    explicit promotion.
13. **ruVLLM training, critic and evaluation path.** It may consume a
    deterministic `ConservationDelta`, synthesize trajectories and run
    ablations. It must not replace the deterministic CI parse baseline or
    auto-promote its own candidate.

## Vendor patterns to reject

1. **`grobid-rs` JNI engine as a dependency.** The crate itself is MIT/Apache-2;
   GROBID is Apache-2 and OpenJDK is GPLv2 with Classpath Exception. The reason
   to reject is architectural/operational, not a blanket GPL claim: it embeds a
   JVM, packages a large Java runtime, and keeps Java as in-process identity.
   Reuse its TEI walker, error, config, and cache patterns only.
2. **OpenAlex parser S3 cache.** External persistence and credentials conflict
   with local-first replay and project storage constraints.
3. **Automatic fixed-count retries.** Retryability must be code- and
   profile-aware; structural parse must not retry non-retryable errors or hide
   503/429 indefinitely.
4. **Dedoc/Docling/Markdown as TEI truth.** They are optional body/layout/OCR
   backends and donors, not scholarly bibliography parity.
5. **Unversioned self-modifying SONA inside deterministic lift.** SONA,
   MicroLoRA and EWC++ training are active under D294, but learned policies are
   versioned candidates and cannot mutate the TEI lifter without evaluation
   and explicit promotion.
6. **ruVLLM serving runtime inside `da-parse`.** Its scheduler and metrics are
   conceptual donors; importing an LLM runtime for GROBID backpressure would be
   architectural excess.
7. **HNSW/paged attention in the current ownership cut.** No current hot path
   needs vector search or paged KV. They become relevant only for specialist
   native heads and routing history after measurement.

## Recommended target architecture

```text
ParserPort
  |
  v
ScholarlyParser<B: ParseBackend, L: TeiLifter>
  |-- request/profile fingerprint
  |-- bounded endpoint permits + retry budget
  |-- raw TEI envelope cache (PDF hash + backend version + request hash)
  |-- deterministic TeiLifter
  |-- conservation_inventory
  `-- ParsedArticle projector (projector schema fingerprint)

ParseBackend
  |-- GrobidHttpBackend       now
  |-- CachedTeiBackend        replay/tests
  `-- NativeCascadeBackend    later, stage-by-stage gold-gated

TeiLifter
  |-- header/idno/authors
  |-- sections/body
  |-- references/callouts
  |-- figures/tables/formulas
  `-- typed coordinates + missingness

Control plane (outside deterministic lift)
  |-- FSM grammar_route/admission/retry/concurrency
  |-- SONA ReasoningBank + MicroLoRA/EWC++ candidate training
  `-- ruVLLM conservation critic + ablation/evaluation
```

## Recommended sequence

1. Re-enable real-TEI fixtures; absence must fail.
2. Extract `GrobidHttpBackend` and `DeterministicTeiLifter` interfaces without
   changing output.
3. Implement public `conservation_inventory` and pin current loss.
4. Lift P0 tissues (idno, authors, non-table figures) until conservation passes.
5. Introduce typed request/error/envelope contracts and endpoint-specific
   timeout/retry/backpressure.
6. Split TEI blob cache from projector cache.
7. Add header/references endpoints and typed coordinates/callouts.
8. Define neutral layout-token IR; benchmark donor extractors.
9. Attach measured ruVector arena/SIMD/SMP patterns to native specialist heads.
10. Run SONA/MicroLoRA/EWC++ and ruVLLM learning in parallel with native-stage
    development; preserve candidate lineage and the no-auto-promote boundary.

## Bottom line

The implementation should be accepted as a successful **ownership cut**, not
as completion of the CE parse-plane programme. Its most elegant decisions are
boundary placement, compatibility preservation, and dependency restraint. Its
largest architectural debt is that the HTTP client remains the type that owns
all pure TEI operations, so the crate is not yet backend-neutral. The most
important omitted deliverable is not retry or SIMD; it is the deterministic,
public conservation API with live fixtures. Without that gate, P0 lift, native
heads, SONA, and ruVLLM cannot be evaluated honestly.
