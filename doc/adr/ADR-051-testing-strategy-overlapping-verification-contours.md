# ADR-051: Testing Strategy — Overlapping Verification Contours

**Status:** Accepted
**Date:** 2026-08-02
**Deciders:** collaborative
**Related:**
- ADR-040 (Samyama sole KG+vector+persist store)
- ADR-050 (kg-* universal crate family — traits must have contract suites)
- ADR-034 (hexagonal/onion overlay — crate boundaries are test boundaries)
- Research: Cockburn Hexagonal Architecture, proptest-state-machine,
  RuVector lifecycle E2E, Samyama use-case specs, grok-build serde roundtrip,
  EdgeQuake spec-driven + flaky detection, iwe fixture builder

## Context

daily-archive has 303+ tests across 10 crates, but they form a **flat
collection** rather than a **layered verification matrix**. Specifically:

1. **No port contract suites** — GraphStore, Embedder, GraphAlgorithms
   traits lack a reusable test suite that runs against every
   implementation (Mock + Samyama + future RuVector). A bug in the real
   adapter that the Mock does not reproduce will pass tests but fail in
   production.

2. **No serde roundtrip tests** — public types (TemporalEdge,
   RetryPolicy, ProvenanceEvent, EdgeContract) are Serialize+Deserialize
   but have no automated wire-format verification. A rename or enum
   variant change can silently break graph persistence.

3. **No lifecycle E2E tests** — SamyamaGraphStore snapshot export/import
   round-trip is not verified. Data loss across restart is invisible
   until a real crash.

4. **No use-case specs** — extraction tests check mechanics (entity
   count, F1) but not end-to-end domain scenarios ("ingest paper
   2507.19457 → verify MENTIONS edges → time-travel query").

5. **No deterministic test infrastructure** — tests rely on system
   time, random UUIDs, and execution-order-dependent shared state.

6. **No flaky test detection** — non-deterministic failures are caught
   manually, not through repeated execution and comparison.

7. **No property-based testing** — bi-temporal intervals, VID hashing,
   retry backoff, and canonicalization are ideal for property-based
   verification but use only example-based tests.

## Decision

Adopt a **seven-contour testing model** mapped to hexagonal architecture
layers. Each contour has a specific oracle, scope, and toolset. The
model is inspired by Cockburn's Ports & Adapters (the test harness is a
driving adapter; the in-memory fake is a driven adapter), research on
Rust hexagonal testing (2025-2026), and patterns observed in vendor-source
projects (RuVector, Samyama, grok-build, EdgeQuake, iwe).

### Testing contour matrix

Two orthogonal axes: **hexagonal layers** (rows) × **test types** (columns).
Each cell describes what that test type looks like at that layer.

#### Axis 1: Hexagonal layers (7 contours)

```text
┌─────────────────────────────────────────────────────────┐
│ Contour 7: Architecture Invariant (governor + guardrail) │
├─────────────────────────────────────────────────────────┤
│ Contour 6: System Journey E2E (lifecycle, use-case specs)│
├─────────────────────────────────────────────────────────┤
│ Contour 5: Adapter Integration (real Samyama, GROBID)    │
├─────────────────────────────────────────────────────────┤
│ Contour 4: Port Contract Suite (GraphStore, Embedder)    │
├─────────────────────────────────────────────────────────┤
│ Contour 3: Application Functional (use cases, mock ports)│
├─────────────────────────────────────────────────────────┤
│ Contour 2: Domain Semantic (invariants, property, serde) │
├─────────────────────────────────────────────────────────┤
│ Contour 1: Determinism Infrastructure (Clock, ID, RNG)   │
└─────────────────────────────────────────────────────────┘
```

#### Axis 2: Test types (8 categories)

| Type | What it verifies | Oracle | Tool | Speed |
|------|------------------|--------|------|-------|
| **Unit** | Single function/method/struct | Direct input→output | `#[test]` inline | <1ms |
| **Property** | Algebraic invariants over random inputs | Metamorphic relation | `proptest!` | ~50ms |
| **Serde roundtrip** | Wire-format stability | encode→decode≡original | `roundtrip<T>()` macro | <1ms |
| **Contract** | Trait semantics across all impls | Shared suite | `contract_*.rs` | <10ms |
| **Integration** | Real external system behaviour | Observable effect | `cargo test` + real deps | ~100ms |
| **Business logic** | Domain policy/invariant enforcement | Domain expert rule | use-case test | ~50ms |
| **Mutation** | Test suite catches code mutations | Mutant killed by tests | `cargo-mutants` | ~minutes |
| **E2E journey** | Full business scenario | Observable result | CLI/HTTP test | ~seconds |

#### Full matrix: layer × type

| Layer | Unit | Property | Serde | Contract | Integration | Business | Mutation | E2E |
|-------|------|----------|-------|----------|-------------|----------|----------|-----|
| **1. Determinism** | ✅ Clock/RNG impls | ❌ | ❌ | Clock trait | ❌ | ❌ | ❌ | ❌ |
| **2. Domain** | ✅ 248 tests | ✅ vid/retry/temporal | ✅ all Serialize types | ❌ | ❌ | ✅ invariants | ✅ domain crates | ❌ |
| **3. Application** | ✅ use-case methods | ❌ | ❌ | ❌ | ❌ (mock) | ✅ policy enforcement | ✅ application crates | ❌ |
| **4. Port contract** | ❌ | ❌ | ❌ | ✅ GraphStore/Embedder | ❌ | ❌ | ❌ | ❌ |
| **5. Adapter** | ❌ | ❌ | ❌ | ✅ (suite re-run) | ✅ Samyama/wiremock | ❌ | ❌ | ❌ |
| **6. E2E journey** | ❌ | ❌ | ❌ | ❌ | ✅ (real deps) | ✅ scenario oracle | ❌ | ✅ UC1-5 |
| **7. Architecture** | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ governor |

**Legend:** ✅ = required, ❌ = not applicable, blank = optional

#### Priority matrix (what to build first)

| Priority | Test type | Where | Why |
|----------|-----------|-------|-----|
| **P0** | Unit (domain) | kg-* + da-domain | Foundation — catches regressions in pure logic |
| **P0** | Serde roundtrip | kg-ontology public types | Wire-format stability — prevents silent persistence breakage |
| **P0** | Contract (GraphStore) | kg-storage/tests/ | Mock vs Samyama semantic drift |
| **P1** | Property | kg-ontology (vid, retry, bitemporal) | Edge cases humans miss |
| **P1** | Business logic | da-application (extraction, healing) | Policy enforcement (D127, schema, temporal) |
| **P1** | Mutation (domain) | kg-ontology + da-domain | Test quality — do tests catch real bugs? |
| **P2** | Integration (wiremock) | da-adapters (GROBID, FdApi) | Real protocol behaviour |
| **P2** | E2E journey | da-cli/tests/ | End-to-end business value |
| **P3** | Mutation (application) | da-application | Harder to run, lower ROI |
| **P3** | Flaky detection | all crates | Governor `--check-flaky N` |

#### Detailed type definitions

**Unit test** — tests a single function, method, or struct in isolation.
No I/O, no external dependencies. Lives inline (`#[cfg(test)]`) or in
`tests/` directory. Runs in <1ms.

```rust
#[test]
fn test_sha256_vid_produces_hex() {
    let vid = sha256_vid("paper", "1206.6423");
    assert_eq!(vid.len(), 64);
    assert!(vid.chars().all(|c| c.is_ascii_hexdigit()));
}
```

**Property test** — verifies an algebraic property holds for ALL inputs
in a domain, using random generation + shrinking.

```rust
proptest! {
    #[test]
    fn vid_idempotent(prefix: String, key: String) {
        prop_assert_eq!(sha256_vid(&prefix, &key), sha256_vid(&prefix, &key));
    }
}
```

**Serde roundtrip test** — verifies a type survives serialization and
deserialization without semantic loss.

```rust
#[test]
fn test_temporal_edge_serde_roundtrip() {
    let edge = TemporalEdge { valid_at: ts(1000), invalid_at: None, ... };
    roundtrip(&edge);
}
```

**Contract test** — a shared suite that verifies trait semantics.
Runs against every implementation (Mock, Samyama, future RuVector).

```rust
#[test]
fn mock_passes_graph_store_contract() {
    let store = MockGraphStore::new();
    graph_store_contract_suite(&store);
}
```

**Integration test** — tests against a real external system
(Samyama embedded, GROBID HTTP, FdApi HTTP).

```rust
#[tokio::test]
async fn test_samyama_export_import_roundtrip() {
    let store = SamyamaGraphStore::new();
    // ... create nodes, export, drop, import, verify
}
```

**Business logic test** — verifies a domain policy or business rule is
enforced correctly.

```rust
#[test]
fn test_d127_import_eligible_remains_false() {
    let violations = validate_node_properties("Paper", &props);
    assert!(violations.iter().any(|v| v.rule == "import-eligible"));
}
```

**Mutation test** — mutates production code and checks if any test
fails. Surviving mutants indicate weak tests.

```bash
cargo mutants -p kg-ontology --test-tool=nextest
```

**E2E journey test** — full business scenario through external interface
(CLI, HTTP).

```rust
#[tokio::test]
async fn uc1_ingest_paper_verify_mentions() {
    let fx = GraphFixture::with_papers(&["2507.19457"]).await;
    // ... extract, verify graph state, time-travel query
}
```

#### CI execution tiers

| Tier | When | What runs | Budget |
|------|------|-----------|--------|
| **PR** | Every push | Unit + Property + Serde + Contract (Mock) | <30s |
| **Merge** | On merge to main | + Integration (Samyama) + Business logic | <2min |
| **Nightly** | 1×/day | + Mutation (domain) + E2E journey + Flaky(5) | <15min |
| **Release** | Pre-release | + Mutation (application) + Crash recovery + Migration upgrade | <30min |

### Contour 1: Determinism Infrastructure

**Principle:** Every non-deterministic source (time, randomness, ID
generation) must be injectable as a port. Tests must not depend on
system clock, thread scheduling, or global state.

**Ports to inject:**

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
```

**Test implementation:**

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

**LCG deterministic vector** (borrowed from RuVector + Samyama):

```rust
/// Deterministic pseudo-random vector using an LCG.
/// No rand dependency — fully reproducible.
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

**Tokio paused time** for async timeout/backoff tests:

```rust
#[tokio::test(start_paused = true)]
async fn test_retry_backoff_progression() {
    let policy = RetryPolicy::fast();
    // Time is auto-advanced; no real sleep
}
```

**Rule:** A test is flaky if it can pass or fail without code changes.
The only acceptable sources of non-determinism are: explicit random seed,
explicit time injection, or declared interleaving tests (Loom/Shuttle).

### Contour 2: Domain Semantic Tests

**Scope:** kg-ontology + da-domain (pure logic, no I/O)

**Oracle:** domain invariants, algebraic properties, idempotency,
metamorphic relations.

**Existing coverage:** 248 unit tests across kg-ontology (57) +
da-domain (171) + kg-storage (11) + kg-algorithms (10).

**Add:**

1. **Serde roundtrip macro** (borrowed from grok-build):

```rust
/// Verify a type survives serialize → deserialize without loss.
fn roundtrip<T>(value: &T)
where
    T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let json = serde_json::to_value(value).expect("serialize");
    let parsed: T = serde_json::from_value(json.clone()).expect("deserialize");
    assert_eq!(&parsed, value, "round-trip mismatch");
}
```

Applied to: TemporalEdge, RetryPolicy, ProvenanceEvent, EdgeContract,
QueryResult, VectorSearchResult, YamlNodeSchema, FieldDef.

2. **Property-based tests** via `proptest`:

```rust
proptest! {
    #[test]
    fn vid_deterministic(prefix: String, key: String) {
        let v1 = sha256_vid(&prefix, &key);
        let v2 = sha256_vid(&prefix, &key);
        prop_assert_eq!(v1, v2);
        prop_assert_eq!(v1.len(), 64);
    }

    #[test]
    fn retry_delay_monotonic(retries in 0u32..20) {
        let p = RetryPolicy::default();
        let d1 = p.delay_for_retry(retries);
        let d2 = p.delay_for_retry(retries + 1);
        prop_assert!(d2 >= d1, "delay must be monotonically non-decreasing");
    }

    #[test]
    fn bTemporal_no_future_knowledge(
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

3. **Metamorphic relations:**

| Transformation | Expected relation |
|---------------|-------------------|
| Reorder independent facts | Same query result |
| Repeat idempotent command | Same externally visible state |
| Add isolated graph component | Relevant result unchanged |
| Paraphrase query (same embedding) | Same evidence IDs |
| Shift query time within validity interval | Result unchanged |
| Serialize + deserialize | Semantic equivalence |

### Contour 3: Application Functional Tests

**Scope:** da-application use cases (IngestPDF, ExtractEntities, HealGraph)

**Oracle:** use-case result (entities created, edges materialised,
violations detected).

**Pattern:** Fixture builder (borrowed from iwe):

```rust
pub struct GraphFixture {
    store: SamyamaGraphStore,
    embedder: DeterministicEmbedder,
}

impl GraphFixture {
    /// Create a fixture pre-loaded with papers.
    pub async fn with_papers(arxiv_ids: &[&str]) -> Self {
        let store = SamyamaGraphStore::new();
        let embedder = DeterministicEmbedder::new();
        // Pre-populate papers
        for id in arxiv_ids {
            let nid = store.create_node_direct("Paper").await;
            store.set_property_direct(
                nid, "arxiv_id",
                PropertyValue::String((*id).into())
            ).await;
        }
        Self { store, embedder }
    }
}

// Usage in test:
#[tokio::test]
async fn test_extract_creates_mentions_edges() {
    let fx = GraphFixture::with_papers(&["2507.19457"]).await;
    let result = ExtractEntities::new()
        .execute(&fx.store, &fx.embedder, "2507.19457")
        .await
        .unwrap();
    assert!(result.graph_node_ids.len() > 0);
}
```

**Deterministic embedder** (no network, LCG vectors):

```rust
pub struct DeterministicEmbedder { seed: u64 }
impl Embedder for DeterministicEmbedder {
    async fn embed(&self, text: &str) -> EmbedResult {
        let hash = sha256_vid("embed", text);
        let seed = u64::from_str_radix(&hash[..16], 16).unwrap();
        Ok(EmbedResult { vector: deterministic_vector(768, seed) })
    }
}
```

### Contour 4: Port Contract Suite — NEW

**Scope:** Universal traits (GraphStore, Embedder, GraphAlgorithms)

**Oracle:** a single test suite that runs against every implementation.

**Problem it solves:** MockGraphStore has different semantics from
SamyamaGraphStore (e.g., Mock does not enforce uniqueness, does not
model transactional isolation, always returns stable order). Tests that
pass against Mock can fail against real Samyama.

**Pattern:** Contract test module in a shared testkit:

```rust
// crates/kg-storage/tests/contract_graph_store.rs

/// Generic contract suite for GraphStore + DirectGraphStore.
/// Run against: MockGraphStore, SamyamaGraphStore, future RuVectorAdapter.
pub fn graph_store_contract_suite<T: DirectGraphStore + GraphStore>(store: &T) {
    test_create_node_returns_unique_id(store);
    test_set_and_get_string_property(store);
    test_set_and_get_int_property(store);
    test_create_edge_returns_edge_id(store);
    test_find_node_by_string_property(store);
    test_get_outgoing_edges(store);
    test_get_incoming_edges(store);
    test_node_count_after_creates(store);
    test_edge_count_after_creates(store);
    test_missing_node_property_returns_none(store);
    test_duplicate_create_node_different_ids(store);
    // Temporal-specific
    test_temporal_edge_create_and_query(store);
    test_edge_property_set_and_get(store);
    test_get_edges_between_filters_by_type(store);
}
```

**Critical rule:** MockGraphStore MUST also pass the contract suite. If
it cannot, the Mock is too lenient and must be tightened.

**Contract test for Embedder:**

```rust
pub fn embedder_contract_suite<E: Embedder>(embedder: &E) {
    test_embed_returns_correct_dimension(embedder);
    test_embed_same_text_same_vector(embedder);
    test_embed_different_text_different_vector(embedder);
    test_embed_empty_text(embedder);
    test_embed_unicode_text(embedder);
}
```

### Contour 5: Adapter Integration Tests

**Scope:** da-adapters (real external systems: Samyama embedded,
GROBID HTTP, FdApi HTTP, OpenAlex HTTP)

**Oracle:** real external behaviour (not mock).

**Existing coverage:** 4 SamyamaGraphStore tests in da-adapters.

**Add:**

1. **wiremock** for HTTP adapters (GROBID, FdApi):

```rust
use wiremock::{MockServer, Mock, ResponseTemplate};

#[tokio::test]
async fn test_grobid_parser_handles_timeout() {
    let server = MockServer::start().await;
    Mock::given(any())
        .respond_with(ResponseTemplate::new(504).set_delay(Duration::from_secs(30)))
        .mount(&server)
        .await;
    // Test parser handles timeout gracefully
}
```

2. **Lifecycle test** (borrowed from RuVector):

```rust
#[tokio::test]
async fn test_samyama_lifecycle() {
    // create → ingest → query → export → drop → import → query → verify
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

### Contour 6: System Journey E2E

**Scope:** End-to-end business scenarios through CLI or API.

**Oracle:** observable system result (entities in graph, query results).

**Pattern:** Use-case executable specs (borrowed from Samyama UC1-5):

```rust
// tests/uc1_paper_ingest_test.rs

const PAPER_ARXIV_ID: &str = "2507.19457";
const EXPECTED_ENTITIES: &[&str] = &["BERT", "Transformer", "Attention"];

#[tokio::test]
async fn uc1_ingest_paper_and_verify_graph() {
    let fx = GraphFixture::with_papers(&[PAPER_ARXIV_ID]).await;
    let result = ExtractEntities::new()
        .execute(&fx.store, &fx.embedder, PAPER_ARXIV_ID)
        .await
        .unwrap();

    assert!(result.graph_node_ids.len() >= EXPECTED_ENTITIES.len());
    // Verify MENTIONS edges exist
    let paper_nodes = fx.store.nodes_by_label("Paper").await;
    assert!(!paper_nodes.is_empty());
    // Time-travel: active_at(ingestion_time) == true
    let props = /* read paper node properties */;
    assert!(is_current(&props));
}
```

### Contour 7: Architecture Invariant

**Scope:** crate dependency graph, feature flags, hexagonal direction.

**Existing coverage:** scripts/verify_rust_architecture.sh + governor
CLI (16 ADR rules).

**Add to governor:**

1. **Spec-driven naming** (borrowed from EdgeQuake):

```yaml
# scripts/governor/checks/spec_rules.yaml
specs:
  - id: spec001_no_da_in_kg
    description: "kg-* crates must not depend on da-*"
    rule: kg_crates_must_not_depend_on_da  # existing ADR rule

  - id: spec002_contract_suite_exists
    description: "Every port trait must have a contract test file"
    rule: check_contract_suite_exists  # NEW

  - id: spec003_serde_roundtrip_coverage
    description: "All public Serialize types must have roundtrip tests"
    rule: check_serde_roundtrip_coverage  # NEW
```

2. **Flaky test detection** (borrowed from EdgeQuake):

```bash
# governor --check-flaky 5
# Runs each test 5 times, compares pass/fail across iterations
```

### Governor CLI integration

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

### Test naming conventions

| Contour | Convention | Example |
|---------|-----------|---------|
| Domain semantic | `{module}_test::{property}` | `bitemporal::test_no_future_knowledge` |
| Application functional | `{usecase}_{scenario}_test` | `extract_creates_mentions_edges` |
| Port contract | `contract_{trait}.rs` | `contract_graph_store.rs` |
| Adapter integration | `{adapter}_{feature}_test` | `samyama_lifecycle_test` |
| System journey | `uc{N}_{scenario}_test` | `uc1_paper_ingest_test` |
| Architecture | governor spec | `spec001_no_da_in_kg` |

### Anti-patterns to avoid

1. **Mock every trait** — test choreography of mocks, not semantics
2. **Fake without contract suite** — Mock is too lenient
3. **Functional = HTTP E2E only** — slow, hard to diagnose
4. **Shared test database** — race conditions, order dependence
5. **`sleep` for synchronisation** — slow and flaky
6. **Retry hides flaky test** — should mark as flaky, not pass
7. **100% line coverage as quality gate** — covered line ≠ tested semantic
8. **Snapshot entire result** — reviewer stops seeing real changes
9. **Same code in SUT and oracle** — common-mode failure
10. **Exact assertions for approximate output** — use tolerance, not equality

### Workspace test layout

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

## Consequences

### Positive

- Port contract suites catch Mock/Samyama semantic drift before production
- Serde roundtrip prevents silent wire-format breakage
- Lifecycle test verifies snapshot persistence integrity
- Property tests explore edge cases humans miss
- Deterministic infrastructure eliminates flaky tests at the source
- Use-case specs serve as executable documentation

### Negative

- Initial investment: ~2-3 sessions to build contract suites + testkit
- Contract suites add a maintenance surface (change trait = update suite)
- proptest adds a dependency and CI time (mitigated by `proptest-regressions`)

### Neutral

- Governor gains 3 new commands (--check-contracts, --check-serde-roundtrip,
  --check-flaky)
- Test file count increases but each file is smaller and more focused

## LLM Reading Notes

- This ADR is the authoritative source for "how we test" in daily-archive
- When adding a new port trait: write the contract suite in the same PR
- When adding a new public Serialize type: add roundtrip test
- When a test is flaky: fix the determinism issue, do not add retry
- MockGraphStore must pass the same contract suite as SamyamaGraphStore
- All E2E tests use Fixture builder + DeterministicEmbedder, never real network
- Governor `--test-matrix` gives a live overview of all 7 contours
