# ADR-051: Testing Strategy — Overlapping Verification Contours

**Status:** Accepted
**Date:** 2026-08-02
**Deciders:** collaborative
**Related:**
- ADR-040 (Samyama sole KG+vector+persist store)
- ADR-050 (kg-* universal crate family — traits must have contract suites)
- ADR-034 (hexagonal/onion overlay — crate boundaries are test boundaries)
- ADR-046 (bi-temporal edge model — fact vs structural write path)
- Research (2026-07-28): Cockburn Hexagonal Architecture, proptest-state-machine,
  Rust test stack 2026, RuVector/Samyama/grok-build/EdgeQuake/iwe vendor analysis
- Samyama upstream: `case_studies/DEFINITION_OF_DONE.md`, `examples/test_kg_queries.rs`,
  `case_studies/_lib/validate_queries.py`, UC1–UC5 executable specs

## Context

daily-archive has 303+ tests across 10 crates, but they form a **flat
collection** rather than a **layered verification matrix**. The project
has a temporal knowledge graph, event sourcing, extraction pipeline,
healing operations, and (future) agent runtime — each requiring
specialised semantic test suites.

Key gaps identified:

1. **No port contract suites** — GraphStore, Embedder, GraphAlgorithms
   lack reusable suites running against every implementation.
2. **No serde roundtrip tests** — public Serialize types lack
   wire-format stability verification.
3. **No lifecycle/E2E tests** — snapshot export/import, use-case specs.
    **Partial (2026-08):** disk→same-process query exists (`da query|graph-stats
    --snapshot`); extraction eval (`eval_batch`) is **not** a graph-task suite.
4. **No deterministic test infrastructure** — tests rely on system time,
   random UUIDs, shared state.
5. **No property-based or metamorphic tests** — bi-temporal intervals,
   VID hashing, retry backoff use only example-based tests.
6. **No concurrency testing** — no Loom/Shuttle/Turmoil for interleavings.
7. **No mutation testing** — test quality (do tests catch bugs?) unverified.
8. **No fuzzing** — parsers, event deserialization untested with random input.
9. **No flaky detection** — non-deterministic failures caught manually.
10. **No domain-specific semantic suites** — temporal graph invariants,
    vector retrieval hard constraints, event sourcing replay equivalence,
    candidate→verify→promote safety not codified.

## Decision

Adopt the **seven-contour verification model** from Cockburn's Ports &
Adapters, enriched with domain-specific semantic suites, property-based
testing, concurrency simulation, and mutation testing. The model treats
automated test harness as a **driving adapter** and in-memory fake as a
**driven adapter** — both must satisfy the same contract.

---

## 1. Seven verification contours

```text
┌──────────────────────────────────────────────────────────────┐
│ Contour 7: Architecture Invariant (governor + cargo metadata) │
├──────────────────────────────────────────────────────────────┤
│ Contour 6: System Journey E2E (use-case specs, lifecycle)     │
├──────────────────────────────────────────────────────────────┤
│ Contour 5: Adapter Integration (real Samyama, wiremock HTTP)  │
├──────────────────────────────────────────────────────────────┤
│ Contour 4: Port Contract Suite (GraphStore, Embedder, GraphAlgorithms) │
├──────────────────────────────────────────────────────────────┤
│ Contour 3: Application Functional (use cases via driving port)│
├──────────────────────────────────────────────────────────────┤
│ Contour 2: Domain Semantic (invariants, property, metamorphic)│
├──────────────────────────────────────────────────────────────┤
│ Contour 1: Determinism Infrastructure (Clock, ID, RNG ports)  │
└──────────────────────────────────────────────────────────────┘
```

The key difference from a traditional test pyramid: **contract suites
are a mandatory separate level** between application tests and adapter
integration tests. A fake that is too lenient (no version conflicts,
stable order, no transactional isolation) will pass application tests
but fail in production.

---

## 2. Test type taxonomy (8 categories)

| Type | What it verifies | Oracle | Tool | Speed |
|------|------------------|--------|------|-------|
| **Unit** | Single function/method/struct | Direct input→output | `#[test]` inline | <1ms |
| **Property** | Algebraic invariant over random inputs | Metamorphic relation | `proptest!` | ~50ms |
| **Serde roundtrip** | Wire-format stability | encode→decode≡original | `roundtrip<T>()` macro | <1ms |
| **Contract** | Trait semantics across all impls | Shared suite | `contract_*.rs` | <10ms |
| **Integration** | Real external system behaviour | Observable effect | `cargo test` + real deps | ~100ms |
| **Business logic** | Domain policy/invariant enforcement | Domain expert rule | use-case test | ~50ms |
| **Mutation** | Test suite catches code mutations | Mutant killed | `cargo-mutants` | ~minutes |
| **E2E journey** | Full business scenario | Observable system result | CLI/HTTP test | ~seconds |

### Full matrix: layer × type

| Layer | Unit | Property | Serde | Contract | Integration | Business | Mutation | E2E |
|-------|------|----------|-------|----------|-------------|----------|----------|-----|
| **1. Determinism** | ✅ | ❌ | ❌ | Clock trait | ❌ | ❌ | ❌ | ❌ |
| **2. Domain** | ✅ 248 | ✅ vid/retry/temporal | ✅ all Serialize | ❌ | ❌ | ✅ invariants | ✅ domain | ❌ |
| **3. Application** | ✅ | ❌ | ❌ | ❌ | ❌ (mock) | ✅ policy | ✅ app crates | ❌ |
| **4. Port contract** | ❌ | ❌ | ❌ | ✅ GraphStore/Embedder | ❌ | ❌ | ❌ | ❌ |
| **5. Adapter** | ❌ | ❌ | ❌ | ✅ (re-run) | ✅ Samyama/wiremock | ❌ | ❌ | ❌ |
| **6. E2E journey** | ❌ | ❌ | ❌ | ❌ | ✅ (real deps) | ✅ scenario | ❌ | ✅ UC1-5 |
| **7. Architecture** | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ governor |

---

## 3. Contour 1: Determinism Infrastructure

**Principle:** every non-deterministic source must be injectable as a port.
A test is flaky if it can pass or fail without code changes.

### Ports to inject

```rust
pub trait Clock: Send + Sync {
    fn now(&self) -> chrono::DateTime<chrono::Utc>;
}

pub trait IdGenerator: Send + Sync {
    fn next_id(&self) -> String;
}

pub trait RandomSource: Send + Sync {
    fn next_f32(&mut self) -> f32;
    fn next_u64(&mut self) -> u64;
}

pub trait Sleeper: Send + Sync {
    async fn sleep(&self, duration: std::time::Duration);
}
```

### Test implementations

```rust
struct FixedClock(DateTime<Utc>);
impl Clock for FixedClock {
    fn now(&self) -> DateTime<Utc> { self.0 }
}

struct SeededRng(u64);
impl RandomSource for SeededRng {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005)
                       .wrapping_add(1442695040888963407);
        self.0
    }
}
```

### Tokio paused time

```rust
#[tokio::test(start_paused = true)]
async fn test_retry_backoff_progression() {
    let policy = RetryPolicy::fast();
    // Time auto-advances; no real sleep
}
```

### Deterministic vector (LCG, borrowed from RuVector/Samyama)

```rust
pub fn deterministic_vector(dim: usize, seed: u64) -> Vec<f32> {
    let mut v = Vec::with_capacity(dim);
    let mut x = seed;
    for _ in 0..dim {
        x = x.wrapping_mul(6364136223846793005)
             .wrapping_add(1442695040888963407);
        v.push(((x >> 33) as f32) / (u32::MAX as f32) - 0.5);
    }
    v
}
```

### Rule

A test must NOT depend on: real clock, random UUID, `sleep`, OS thread
scheduling, global state, shared database, previous test execution.

For each simulation run, preserve: seed, transitions, toolchain,
feature set, dependency versions, input corpus, event trace.

---

## 4. Contour 2: Domain Semantic Tests

**Scope:** kg-ontology + da-domain (pure logic, no I/O)
**Oracle:** domain invariants, algebraic properties, idempotency,
metamorphic relations.

### 4.1 Property-based testing (`proptest`)

```rust
proptest! {
    #[test]
    fn vid_deterministic(prefix: String, key: String) {
        prop_assert_eq!(sha256_vid(&prefix, &key), sha256_vid(&prefix, &key));
        prop_assert_eq!(sha256_vid(&prefix, &key).len(), 64);
    }

    #[test]
    fn retry_delay_monotonic(retries in 0u32..20) {
        let p = RetryPolicy::default();
        let d1 = p.delay_for_retry(retries);
        let d2 = p.delay_for_retry(retries + 1);
        prop_assert!(d2 >= d1);
    }

    #[test]
    fn bitemporal_no_future_knowledge(
        valid_from in 1_000_000_000i64..2_000_000_000,
        valid_to in 1_000_000_000i64..2_000_000_000,
        query_at in 1_000_000_000i64..2_000_000_000,
    ) {
        let mut props = PropertyMap::new();
        props.insert("valid_from".into(), json!(valid_from));
        props.insert("valid_to".into(), json!(valid_to));
        let active = is_active_at(&props, DateTime::from_timestamp(query_at, 0).unwrap());
        if valid_from <= query_at && query_at < valid_to {
            prop_assert!(active);
        } else {
            prop_assert!(!active);
        }
    }
}
```

Save `proptest-regressions/` files in git to reproduce previously found cases.

### 4.2 Model-based state-machine testing (`proptest-state-machine`)

Compares system under test with an abstract reference state machine,
generates operation sequences, shrinks both values and sequences.

**SUT candidates:**
- SamyamaGraphStore (create/set/query lifecycle)
- ExtractionUseCase (candidate→verify→promote)
- TaskScheduler (retry scheduling)

**Reference model requirements:**
- Significantly simpler than production implementation
- Independent of production helpers
- Implements only required semantics
- Suitable for comparison after each transition

**Critical rule:** do NOT build oracle through the same code being tested
(common-mode failure).

Note: `proptest-state-machine` supports only sequential strategy.
Concurrent transitions need Loom/Shuttle/Turmoil (see §9).

### 4.3 Metamorphic testing

When exact expected output is hard to obtain, verify relations between
multiple executions.

| Transformation | Expected relation |
|---------------|-------------------|
| Reorder independent facts | Semantically equivalent answer |
| Repeat idempotent command | Same externally visible state |
| Add isolated graph component | Relevant result unchanged |
| Paraphrase query | Same evidence IDs |
| Shift query time within validity interval | Result unchanged |
| Rebuild projection from full event log | Same canonical state |
| Use different port adapter | Same contract holds |
| Serialize and deserialize command/event | Semantic preservation |

Especially important for RAG, graph retrieval, and agentic components
where exact string equality is a bad oracle.

### 4.4 Serde roundtrip tests

```rust
fn roundtrip<T>(value: &T)
where
    T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let json = serde_json::to_value(value).expect("serialize");
    let parsed: T = serde_json::from_value(json.clone()).expect("deserialize");
    assert_eq!(&parsed, value, "round-trip mismatch");
}

#[test]
fn test_temporal_edge_serde_roundtrip() {
    let edge = TemporalEdge { valid_at: ts(1000), invalid_at: None, .. };
    roundtrip(&edge);
}
```

Applied to: TemporalEdge, RetryPolicy, ProvenanceEvent, EdgeContract,
QueryResult, VectorSearchResult, YamlNodeSchema, FieldDef.

---

## 5. Contour 3: Application Functional Tests

**Scope:** da-application use cases via driving port.
**Oracle:** use-case result, events, state changes, effects.

### Functional vs semantic test

**Functional:** does the application execute the required use case with
given inputs and environment state?

**Semantic:** is the meaning and set of business guarantees preserved
(not the specific sequence of internal calls)?

One test can be both functional and semantic.

### State-based fakes (not choreography mocks)

Prefer: state-based fakes, deterministic stubs, recording spies, event
collectors, controlled clocks and schedulers.

Do NOT verify by default:
- Repository method called exactly once
- Methods called in specific order
- Which internal helper was used
- How many internal queries the adapter performed

Interaction assertions are justified ONLY when interaction is part of
semantics:
- Event must be published at most once
- Promotion must not happen before verification
- Transaction must complete before sending acknowledgment
- After cancellation, new external operations are forbidden
- Audit record must be created before changing authoritative state

### Fixture builder (borrowed from iwe)

```rust
pub struct GraphFixture {
    store: SamyamaGraphStore,
    embedder: DeterministicEmbedder,
}

impl GraphFixture {
    pub async fn with_papers(arxiv_ids: &[&str]) -> Self {
        let store = SamyamaGraphStore::new();
        let embedder = DeterministicEmbedder::new();
        for id in arxiv_ids {
            let nid = store.create_node_direct("Paper").await;
            store.set_property_direct(nid, "arxiv_id",
                PropertyValue::String((*id).into())).await;
        }
        Self { store, embedder }
    }
}
```

---

## 6. Contour 4: Port Contract Suite

**The most critical practice.** A single semantic specification of each
port, run against every implementation (Mock, Samyama, future RuVector).

### What contract suite must include

#### For GraphStore / DirectGraphStore (repository-like)

- Save and read
- Duplicate identity (same VID → same node)
- Not-found semantics (missing property → None, not panic)
- Create node returns unique ID
- Create edge returns unique edge ID
- Set and get string/int/float/bool properties
- Find node by string property
- Get outgoing/incoming edges
- Node/edge count after creates
- Temporal edge create and query
- Edge property set and get
- `get_edges_between` filters by type
- No partial write state (atomicity)

#### For Embedder (vector retrieval)

- Embed returns correct dimension
- Same text → same vector (determinism)
- Different text → different vector
- Empty text handling
- Unicode text handling

#### For GraphAlgorithms

- PPR from seed nodes returns scores
- PPR scores sum to ~1 (within tolerance)
- Get neighbors by edge type
- Get all neighbors regardless of type

### Critical rule

**In-memory fake MUST also pass the contract suite.**

Otherwise application tests confirm behaviour absent in the real adapter.
Common fake defects:
- Does not model version conflicts
- Always returns stable order
- Does not account for transactional isolation
- Does not check uniqueness
- Does not reproduce eventual consistency
- Does not model partial write error

### Pattern

```rust
// crates/kg-storage/tests/contract_graph_store.rs

pub fn graph_store_contract_suite<T: DirectGraphStore + GraphStore>(store: &T) {
    test_create_node_returns_unique_id(store);
    test_set_and_get_string_property(store);
    test_set_and_get_int_property(store);
    // ... full list
}

#[test]
fn mock_passes_graph_store_contract() {
    let store = MockGraphStore::new();
    graph_store_contract_suite(&store);
}
```

For inter-service HTTP contracts, consider Pact (consumer-driven).
For internal Rust traits, generic Rust contract suite is simpler and
more precise.

---

## 7. Contour 5: Adapter Integration Tests

**Scope:** da-adapters against real external systems.

### Samyama embedded

```rust
#[tokio::test]
async fn test_samyama_lifecycle_export_import() {
    let store = SamyamaGraphStore::new();
    let nid = store.create_node_direct("Paper").await;
    store.set_property_direct(nid, "arxiv_id",
        PropertyValue::String("2507.19457".into())).await;

    let snapshot = store.export_snapshot().await.unwrap();
    drop(store);

    let store2 = SamyamaGraphStore::new();
    store2.import_snapshot(&snapshot).await.unwrap();
    assert_eq!(store2.node_count().await, 1);
}
```

### HTTP adapters (GROBID, FdApi, OpenAlex) — wiremock

```rust
use wiremock::{MockServer, Mock, ResponseTemplate};

#[tokio::test]
async fn test_grobid_parser_handles_timeout() {
    let server = MockServer::start().await;
    Mock::given(any())
        .respond_with(ResponseTemplate::new(504)
            .set_delay(Duration::from_secs(30)))
        .mount(&server).await;
    // Test parser handles timeout gracefully
}
```

Test not only happy path: timeout, connection reset, HTTP 429,
retry-after, 5xx, malformed JSON, unknown enum variants, partial
response, duplicate response, out-of-order messages, cancellation
during streaming.

wiremock validates **your adapter**, not that mock matches real vendor
API. For critical integrations, add a small compatibility suite
against sandbox/real provider.

### Testcontainers (for future PostgreSQL/RuVector)

Custom Testcontainers image containing: production-compatible version,
required extension, same index configuration, migrations, minimal
deterministic fixture.

Why rollback transaction is NOT sufficient:
- Application opens its own transaction
- Multiple connections used
- Visibility between transactions tested
- Advisory locks
- Outbox read by another worker
- Optimistic concurrency checked
- Migration/extension creates database-level objects

---

## 8. Contour 6: System Journey E2E

**Scope:** full business scenarios through CLI or API.
**Oracle:** observable system result.

### Use-case executable specs (borrowed from Samyama UC1-5)

```rust
// tests/uc1_paper_ingest_test.rs

const PAPER_ARXIV_ID: &str = "2507.19457";
const EXPECTED_ENTITIES: &[&str] = &["BERT", "Transformer", "Attention"];

#[tokio::test]
async fn uc1_ingest_paper_and_verify_graph() {
    let fx = GraphFixture::with_papers(&[PAPER_ARXIV_ID]).await;
    let result = ExtractEntities::new()
        .execute(&fx.store, &fx.embedder, PAPER_ARXIV_ID)
        .await.unwrap();

    assert!(result.graph_node_ids.len() >= EXPECTED_ENTITIES.len());
    let paper_nodes = fx.store.nodes_by_label("Paper").await;
    assert!(!paper_nodes.is_empty());
}
```

### 8.1 Semantic graph task pipeline (Samyama case-study pattern)

**Problem this solves:** unit/contract/extraction-F1 tests do not prove that a
**durable graph artifact** answers domain questions. Samyama treats a case study
as Done only when a pinned `.sgsnap` is imported and **every** showcase query
returns ≥1 row (machine gate). daily-archive adopts the same **task-through-graph**
shape for scientific KG readiness.

#### Canonical pipeline (must not be short-circuited)

```text
┌──────────────┐    ┌──────────────┐    ┌────────────────────┐
│ 1. Materialise│ →  │ 2. Task suite│ →  │ 3. Import + execute │
│ durable graph │    │ (tasks file) │    │ in ONE process      │
│ (.sgsnap)     │    │              │    │                     │
└──────────────┘    └──────────────┘    └──────────┬─────────┘
                                                   │
                     ┌─────────────────────────────▼─────────┐
                     │ 4. Per-task verdict                     │
                     │    PASS  = rows ≥ min_rows (default 1) │
                     │    EMPTY = 0 rows                       │
                     │    ERROR = Cypher/runtime failure       │
                     └─────────────────────────────┬─────────┘
                                                   │
                     ┌─────────────────────────────▼─────────┐
                     │ 5. Suite gate                           │
                     │    FAIL if any EMPTY or ERROR           │
                     │    Report: pass / empty / error + ms    │
                     └─────────────────────────────────────────┘
```

**Operator mapping (daily-archive CLI today):**

| Step | Command / artifact |
|------|--------------------|
| Materialise | `da batch-ingest … --output artifacts/…/hub.sgsnap` |
| Task suite | `data/graph_tasks/<suite>.tasks.cypher` (or YAML; see format below) |
| Import+query | **Same process:** `da query --snapshot <file.sgsnap> --kind …` or suite runner |
| Verify-only | `da load-snapshot` — does **not** feed a later process (process-local store dies) |
| Stats | `da graph-stats --snapshot <file.sgsnap>` |

**Hard rule (M334 lesson):** never treat `load-snapshot` in process A + `query`
in process B as a valid E2E chain. Contour 6 disk→query **must** use
`--snapshot` (or a future live daemon client when `DA_GRAPH_MODE=remote` is real).

#### Task suite file format (Samyama-compatible markers)

Primary format: Cypher file with showcase markers (same spirit as
`case_studies/*/queries.cypher`):

```cypher
// @query Paper count (retrieval-eligible) | Scale check — GEPA hub batch has 24 papers
MATCH (n:Paper) WHERE n.retrieval_eligible = true RETURN count(n) AS papers;

// @query Hub paper by arxiv_id | Orient — GEPA hub 2507.19457 exists
MATCH (n:Paper {arxiv_id: "2507.19457"}) RETURN n.arxiv_id, n.title LIMIT 1;

// @query Citation edges from hub | Multi-hop readiness — CITES fact edges present
MATCH (p:Paper {arxiv_id: "2507.19457"})-[:CITES]->(c)
RETURN count(c) AS cites;

// @query Structural hasPart | Composition — sections/refs hang off papers
MATCH (p:Paper)-[:hasPart]->(x) RETURN count(x) AS parts;
```

Rules:

1. **`// @query Title | insight`** — title for the report; insight is what an SME
   learns (not a restate of the Cypher).
2. **Project properties**, not bare nodes (`RETURN n.arxiv_id`, not only `RETURN n`)
   when the query engine/columnar store may blank bare-node renders.
3. **Use ontology wire strings** for relationship types (`hasPart`, `CITES`, not
   inventing `HAS_PART`). Source: `data/ontology/edge_contracts.yaml`.
4. **String literals double-quoted** in Cypher; numeric bounds match property types.
5. **min_rows default = 1** for showcase tasks (Samyama DoD). Scale/count tasks may
   set `// @min_rows N` or suite-level thresholds when asserting exact counts.

Optional YAML companion (for expected counts / temporal as-of):

```yaml
# data/graph_tasks/gepa_hub.tasks.yaml
suite_id: gepa_hub_v1
snapshot: artifacts/gepa_hub_production/hub.sgsnap
import_eligible: false   # D127 — suite never unlocks import
tasks:
  - id: paper_count
    title: Paper count
    insight: GEPA hub batch scale
    cypher: |
      MATCH (n:Paper) WHERE n.retrieval_eligible = true RETURN count(n) AS papers
    min_rows: 1
    # optional oracle (Contour 6 only when deterministic):
    expect:
      papers_gte: 24
```

#### Suite runner contract (Contour 6 harness)

A suite runner (CLI example or `cargo test` harness) must:

1. Load snapshot bytes → `import_snapshot` into **this** process store.
2. Refuse non-empty `DA_GRAPH_DATA_PATH` (M332) and keep `import_eligible=false` (D127).
3. Parse tasks; for each task run Cypher via GraphStore WARM path (`query_readonly`)
   or domain builders (`PaperQueries`, etc.) — **same port**, not a parallel oracle.
4. Classify PASS / EMPTY / ERROR; record wall ms.
5. Exit non-zero if any task is EMPTY or ERROR (showcase gate), unless task is
   marked `allow_empty: true` (rare; must justify in insight).
6. Emit a machine-readable summary (`*.json`) + human table for CI artifacts.

Reference implementations to mirror (vendor, not runtime deps):

| Upstream | Role |
|----------|------|
| `samyama-graph/examples/test_kg_queries.rs` | Import snap → loop CSV queries → pass/empty/error tallies |
| `samyama-graph/case_studies/_lib/validate_queries.py` | DoD gate: every `// @query` ≥ min_rows |
| `samyama-graph/case_studies/DEFINITION_OF_DONE.md` | Independent verification, pinned snap, SME narrative |
| `samyama-graph/tests/uc*.rs` | Executable use-case specs (UC1–UC5) |

#### Relation to extraction evaluation (`eval_batch`)

| Lane | Input | Oracle | Proves |
|------|-------|--------|--------|
| **Extraction eval** (`eval_batch`) | PDF/HTML + `data/gold_standard/*.gold.json` | Gold entity labels | RuleBasedExtractor P/R/F1 |
| **Semantic graph task suite** | `.sgsnap` + `data/graph_tasks/*` | Rows ≥ min_rows (+ optional count bounds) | Graph answers domain questions after ingest/extract |

Both are required. High extraction F1 with empty graph tasks = **not ready**.
Graph tasks with no extraction gold = structure/citation readiness only.

#### Definition of Done for a semantic suite (daily-archive)

A suite is **Done** when:

- [ ] Snapshot path is pinned (repo artifact or content-addressed); D127 locked.
- [ ] `da graph-stats --snapshot <snap>` shows non-zero nodes/edges.
- [ ] Suite runner imports that snap and runs **all** tasks in one process.
- [ ] Every showcase task is PASS (no EMPTY/ERROR).
- [ ] At least one multi-hop or cross-label task (graph-native strength).
- [ ] At least one task uses a **fact** edge wire (`CITES`, `MENTIONS`, …) and one
      **structural** edge (`hasPart`, `FROM_SOURCE`, …) from meta-ontology.
- [ ] Report lists pass/empty/error counts and per-task latency.
- [ ] CI: PR may run a **tiny** suite (≤5 tasks, fixture snap); Merge/Nightly runs
      full GEPA hub (or successor) suite.

#### Anti-patterns specific to this pipeline

1. **Banner-only honesty** — documenting process-local death without a working
   `--snapshot` read path (fixed M334; do not regress).
2. **Cross-process load-then-query** as “E2E”.
3. **Using extraction F1 as sole KG readiness metric.**
4. **Hardcoding edge type names in tasks** that disagree with
   `data/ontology/edge_contracts.yaml`.
5. **Building a second oracle** that reimplements ingest/extract to “expect”
   graph contents (common-mode failure) — prefer independent gold tasks and
   row-count/structure oracles.

---

## 9. Concurrency and Resilience Testing

Standard async test reruns barely explore interleaving space.

### Loom

Exhaustive exploration of concurrent code for small primitives:
lock-free structures, atomic state machines, channels, caches, shared
ownership, custom synchronisation.

```rust
#[test]
fn test_concurrent_node_creation() {
    loom::model(|| {
        let store = Arc::new(SamyamaGraphStore::new());
        let store2 = store.clone();
        let t1 = loom::thread::spawn(move || {
            store2.create_node_direct("Paper")
        });
        let id2 = store.create_node_direct("Paper");
        let id1 = t1.join().unwrap();
        assert_ne!(id1, id2); // unique IDs under concurrency
    });
}
```

### Shuttle

Randomized scheduling. No proof of correctness, but scales to larger
scenarios than Loom, with deterministic replay of found schedules.

Suitable for: multiple workers, task queues, deduplication, concurrent
projections, timeout-vs-completion race, lease renewal, cancellation,
competing promotion attempts.

### Turmoil

Deterministic simulation testing for distributed async Rust. Executes
multiple hosts in one thread, injects: latency, packet loss, partitions,
crashes, torn writes, network and filesystem failures.

For agent/event runtime, verifies:
- Crash after recording event but before acknowledgment
- Retry after unknown outcome
- Split-brain
- Lease loss
- Delayed verifier response
- Duplicate command delivery
- Worker crash between verify and promote
- Projection recovery after torn write
- Safety invariant under partition

---

## 10. Coverage, Mutation, Fuzzing, Formal Tools

These do NOT replace semantic tests — they answer different questions.

| Tool | Question |
|------|----------|
| `cargo-llvm-cov` | What code was executed? |
| `cargo-mutants` | Do tests catch artificially introduced changes? |
| `cargo-fuzz` | What unexpected inputs/sequences break the implementation? |
| Miri | Is there undefined behaviour? |
| Kani | Does a bounded correctness property hold for all explored inputs? |

### Coverage

Use as a gap map, not a quality gate. Track domain/application
separately. Do NOT require 100%. Exclude generated glue, derive
output, trivial wiring.

### Mutation testing

```bash
cargo mutants -p kg-ontology --test-tool=nextest
```

Run on: domain and application crates, changed crates, nightly or
pre-release — not entire workspace in every PR.

### Fuzzing

Priority targets: scientific document parser, markdown/HTML/PDF IR,
DSL/query parser, graph import format, event deserialization, schema
migrations, temporal interval normalisation, canonicalisation and
dedup, binary/vector formats, API codecs, unsafe wrappers.

### Miri

For crates with `unsafe`, FFI, and low-level structures — NOT for
business correctness.

### Kani

Model checking for compact pure algorithms and unsafe abstractions.
Concurrency support incomplete; function contracts experimental.

---

## 11. Snapshot and BDD — Selectively

### Snapshot testing (`insta`)

Useful for: canonical events, validation errors, AST, query plans,
normalised graph fragments, CLI output, stable serialised contracts.

NOT for: entire HTTP response without semantic assertions, approximate
vector ordering, LLM text, large JSON where reviewer stops seeing
changes, data with mass non-deterministic part.

Snapshot complements targeted assertions, does not replace them.

### BDD (Cucumber)

For solo-dev Rust project creates unnecessary translation layer.
Prefer: scenario builders, domain test names, Given/When/Then inside
regular Rust, `rstest` for tabular inputs.

Cucumber justified only when Gherkin is a shared specification between
business, QA, and development.

---

## 12. Domain-Specific Semantic Suites

### 12.1 Temporal knowledge graph

**Mandatory properties:**

1. **No future knowledge:** `query(as_of=t)` cannot use a fact that
   became known after `t`.
2. **Valid time ≠ transaction time:** correcting a historical fact
   must not change the old audit view.
3. **Current projection reproducible:** rebuild from authoritative
   event/fact log yields the same canonical current state.
4. **Interval boundaries unambiguous:** intervals defined as `[from, to)`,
   no different interpretations across adapters.
5. **Correction ≠ destructive overwrite:** provenance and supersession
   relation preserved.
6. **Canonicalisation idempotent:** repeat import does not create a new
   entity or relation.
7. **Irrelevant expansion invariant:** isolated nodes/edges do not
   affect local semantic query.
8. **Projection order:** independent events may commute; dependent must
   preserve order.

### 12.2 Vector and hybrid retrieval

**Hard invariants (always checked):**
- No forbidden tenant results
- No facts outside temporal window
- Every result has provenance
- Score is not NaN
- Deleted documents not returned
- Mandatory filters strictly enforced

**Quality relations (tolerance-based):**
- recall@k vs exact/brute-force oracle on small corpus
- Set equivalence on ties
- Score tolerance
- Resilience to adding irrelevant documents
- Key evidence ID preservation on paraphrase
- Differential comparison: dense-only vs graph-only vs hybrid

Application tests use `DeterministicEmbedder`. Real embedding model and
approximate index checked in separate retrieval-quality lane.

### 12.3 Event sourcing and replay

- `fold(events)` is deterministic
- Snapshot + tail equivalent to full replay
- Fork from one position is reproducible
- Schema upcasting preserves semantics
- Projection handles repeated event safely
- Failed projection does not advance checkpoint
- Checkpoint and materialised state change atomically
- Replay does not re-trigger external side effects

### 12.4 Candidate → verify → promote

**Critical invariants:**

```text
candidate ≠ authoritative state
promote allowed only after independent verification
failed verification never changes authoritative state
retry does not create double promotion
worker cannot self-confirm own result
audit log sufficient to reproduce decision
promotion and evidence fixation are atomic
```

For critical verifier, do NOT use the same production helper that
candidate builder used to compute the verified property — common-mode
failure.

### 12.5 Agent/LLM components

**Bad oracle:** `output_text == expected_text`

**Robust oracle:**
- Valid action selected
- Tool arguments match schema
- Evidence IDs relevant and exist
- Mandatory citations present
- Budget not exceeded
- Policy gate not bypassed
- State transition allowed
- Missing evidence → no promotion
- Paraphrase/reordering does not change actual decision
- Misleading context does not replace authoritative evidence

---

## 13. Architecture Invariant Tests (Contour 7)

### Crate dependency graph check

`cargo metadata --format-version=1` provides machine-readable workspace
metadata and resolved dependency graph.

Existing: `scripts/verify_rust_architecture.sh` + governor CLI with
`grep_deps` / `grep_deps_prod` helpers checking Cargo.toml.

Optional future: `cargo-depgraph-check` (new, July 2026) or custom
`xtask` on `cargo metadata`.

### Compile-time checks

- `cargo-hack` — feature combinations (`--feature-powerset`)
- `trybuild` — expected compile errors and type-level API contracts
- Compile-fail tests — inability to create invalid object
- Visibility rules — no public access to adapter internals

### Governor spec-driven naming (borrowed from EdgeQuake)

```yaml
specs:
  - id: spec001_no_da_in_kg
    description: "kg-* crates must not depend on da-*"
  - id: spec002_contract_suite_exists
    description: "Every port trait must have a contract test file"
  - id: spec003_serde_roundtrip_coverage
    description: "All public Serialize types must have roundtrip tests"
```

### Flaky test detection (borrowed from EdgeQuake)

```bash
# governor --check-flaky 5
# Runs each test 5 times, compares pass/fail across iterations
```

---

## 14. Governor CLI Integration

New governor commands to enforce testing contours:

```bash
# Check contract suite coverage
PYTHONPATH=scripts python3 -m governor --check-contracts

# Check serde roundtrip coverage
PYTHONPATH=scripts python3 -m governor --check-serde-roundtrip

# Detect flaky tests (run N times)
PYTHONPATH=scripts python3 -m governor --check-flaky 5

# Full test matrix overview
PYTHONPATH=scripts python3 -m governor --test-matrix
```

---

## 15. Workspace Test Layout

```text
crates/
├── kg-ontology/src/*.rs         # Domain unit tests (inline #[cfg(test)])
├── kg-storage/
│   ├── src/mock.rs              # MockGraphStore (must pass contract suite)
│   └── tests/
│       └── contract_graph_store.rs  # Port contract suite
├── kg-embeddings/
│   └── tests/
│       └── contract_embedder.rs     # Port contract suite
├── da-domain/src/*.rs           # Domain unit tests (inline)
├── da-application/
│   ├── src/lib.rs               # Fixture builder
│   └── tests/
│       ├── extraction_test.rs   # Application functional
│       └── batch_ingest_test.rs
├── da-adapters/
│   └── tests/
│       ├── samyama_graph.rs     # Adapter integration
│       └── grobid_parser.rs     # Adapter integration (wiremock)
├── da-cli/
│   └── tests/
│       └── uc1_paper_ingest.rs  # System journey E2E
└── testkit/                     # Shared test utilities (dev-dep only)
    ├── deterministic_vector()
    ├── roundtrip<T>()
    └── GraphFixture
```

`testkit` connects only as dev-dependency. Each file in `tests/`
compiles as a separate crate — group by bounded context, not hundreds
of tiny files.

---

## 16. Recommended Rust Test Stack (2026)

### Basic

- `cargo test` (built-in)
- `cargo-nextest`
- `proptest`
- `proptest-state-machine`
- `rstest` (tabular cases)
- `testcontainers`
- `wiremock`
- `cargo-llvm-cov`
- `cargo-mutants`

### Concurrency and resilience

- Tokio paused time
- Loom
- Shuttle
- Turmoil

### Parsers and low-level code

- `cargo-fuzz`
- Miri
- Kani
- sanitizers

### Architecture and API

- custom `xtask` on `cargo metadata` (or `cargo-depgraph-check`)
- `cargo-hack`
- `trybuild`
- `cargo-semver-checks` (for public crates)

### Snapshots

- `insta` — only for stable canonical representations

---

## 17. CI Profiles

### Per PR

```text
format
clippy
workspace check
architecture dependency check
domain semantic tests
application functional tests
port contracts
basic adapter integration tests
doctests
```

Budget: <30s using per-crate checks (MEM468/MEM514).

### Nightly

```text
feature powerset (cargo-hack)
full Samyama contracts
mutation testing (domain + application)
fuzzing (cargo-fuzz, time-boxed)
Miri (unsafe crates)
Shuttle/Turmoil (multiple seeds)
extended retrieval-quality tests
```

Budget: <15min.

### Pre-release

```text
production-version dependency images
forward migrations
upgrade from previous release schema
projection rebuild
critical system journeys
backup/restore
crash recovery
compatibility contracts
cargo-semver-checks (public crates)
```

Budget: <30min.

---

## 18. Anti-patterns

1. **Mock every trait** — tests mock choreography, not use-case meaning.
2. **Fake without contract suite** — app works only against simplified memory.
3. **Functional = HTTP E2E only** — slow, hard to diagnose suite.
4. **Shared test database** — race conditions, order dependence, `serial_test`.
5. **`sleep` for synchronisation** — slow and flaky tests.
6. **Retry hides flaky test** — should mark as flaky, not pass.
7. **100% line coverage as quality gate** — covered line ≠ tested semantic.
8. **Snapshot entire result** — reviewer stops seeing real changes.
9. **Same code in SUT and oracle** — common-mode false confirmation.
10. **Exact assertions for approximate/LLM output** — unstable form checked.
11. **Testing only happy path** — no retries, cancellation, partial commit.
12. **Architecture boundaries only in documentation** — domain starts
    depending on Tokio, SQLx, HTTP types, config framework over time.
13. **Disk load in process A + query in process B as “E2E”** — process-local
    store dies on exit; Contour 6 requires same-process `--snapshot` (or real remote).
14. **Extraction F1 as sole scientific KG readiness metric** — must pair with
    semantic graph task suite (§8.1) on a durable `.sgsnap`.

---

## 19. Implementation Priorities

### P0 — Foundation

1. Split workspace into domain/application/adapters/bootstrap. ✅ DONE
2. Add executable dependency rules. ✅ DONE (governor + guardrail)
3. Extract Clock, ID, RNG, Scheduler, external deps into ports.
4. Define semantics of each port in text.
5. Create contract suites.
6. Run them against fake and real adapters.

### P1 — Semantic depth

1. Property tests for domain value objects.
2. State-machine model for GraphStore and candidate lifecycle.
3. Temporal/no-future invariants.
4. Replay and projection equivalence.
5. Metamorphic tests for graph/vector retrieval.
6. Mutation testing domain/application.
7. **Semantic graph task suite (Contour 6 §8.1)** — pin `.sgsnap` +
   `data/graph_tasks/*` + same-process runner (PASS/EMPTY/ERROR gate),
   separate from extraction `eval_batch`.

### P2 — System reliability

1. Loom for low-level synchronisation.
2. Shuttle for worker concurrency.
3. Turmoil for network/filesystem/crash simulation.
4. Fuzzing parsers, import, event formats.
5. Miri/Kani for unsafe and critical algorithms.
6. Release-level recovery and migration scenarios.
7. Live remote GraphStore client + task suite against daemon (only when
   `DA_GRAPH_MODE=remote` is real — not `RemoteNotConfigured`).

### Core principle

> Every production adapter must be replaceable by a test adapter, every
> test adapter must satisfy the same contract, and every critical
> business invariant must have an independent semantic oracle.

---

## Consequences

### Positive

- Port contract suites catch Mock/Samyama semantic drift before production
- Serde roundtrip prevents silent wire-format breakage
- Property tests explore edge cases humans miss
- Model-based state-machine testing verifies lifecycle correctness
- Metamorphic testing handles approximate/LLM output robustly
- Concurrency testing (Loom/Shuttle/Turmoil) catches rare interleavings
- Mutation testing validates test suite quality
- Deterministic infrastructure eliminates flaky tests at source
- Domain-specific suites codify temporal, retrieval, and agent invariants
- Semantic graph task pipeline (Samyama case-study DoD) proves durable graph
  answers domain questions, not only extractor F1 or empty-store CLI smoke

### Negative

- Initial investment: ~3-4 sessions for full P0+P1 implementation
- Contract suites add maintenance surface (trait change = suite update)
- `proptest` adds dependency and CI time (mitigated by regressions files)
- Mutation/fuzz testing require nightly CI budget

### Neutral

- Governor gains 4 new commands (--check-contracts, --check-serde-roundtrip,
  --check-flaky, --test-matrix)
- `testkit` crate added as dev-dependency only
- Test file count increases but each file is smaller and more focused

## LLM Reading Notes

- This ADR is the authoritative source for "how we test" in daily-archive
- When adding a new port trait: write the contract suite in the same PR
- When adding a new public Serialize type: add roundtrip test
- When a test is flaky: fix the determinism issue, do not add retry
- MockGraphStore must pass the same contract suite as SamyamaGraphStore
- All E2E tests use Fixture builder + DeterministicEmbedder, never real network
- Governor `--test-matrix` gives a live overview of all 7 contours
- Contour 6 disk→query: use `da query|graph-stats --snapshot <file.sgsnap>` in
  **one** process; never `load-snapshot` then a second CLI process
- Semantic readiness = extraction eval **and** graph task suite (§8.1), not either alone
- Task Cypher must use ontology wire strings from `data/ontology/edge_contracts.yaml`
- Reference research (2026-07-28): full 16-section analysis in project archive
- Vendor pattern sources: Samyama `case_studies/DEFINITION_OF_DONE.md`,
  `examples/test_kg_queries.rs`, `_lib/validate_queries.py`
