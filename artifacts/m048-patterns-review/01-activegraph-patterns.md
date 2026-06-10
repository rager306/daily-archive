# 01 — ActiveGraph Patterns + Track A Adaptation

> **Source:** yoheinakajima/activegraph on GitHub; production guide at docs.activegraph.ai
> **Scope:** ActiveGraph architecture analysis + explicit adaptation to daily-archive Track A (M049-M053)
> **Verdict:** patterns adopted; runtime not adopted (bounded scope)

## 0. Reading Order

This file is the **deep-dive** on ActiveGraph and the **Track A adaptation map**. Read it fully when planning any of M049-M053.

Sections:

1. ActiveGraph core model (what it is)
2. Bottleneck analysis (where it costs)
3. Pattern extraction (what we adopt)
4. **Track A adaptation** (concrete mapping to M049-M053)
5. What we don't adopt (Postgres, distributed, full runtime)
6. LLM Reading Notes

## 1. ActiveGraph Core Model

ActiveGraph is an **event-sourced reactive graph runtime** designed for long-running agents. Core elements:

- **Append-only event log** (Postgres typically) — single source of truth
- **Reactive graph projection** — materialized view of events, updated as events arrive
- **Behaviors** — registered handlers that subscribe to events and emit new events
- **Replay** — given an event log, re-run behaviors deterministically to rebuild the graph
- **Fork/diff** — copy event rows under a new `run_id`, change one parameter, replay from cache
- **Cache replay** — shared prefix between fork and original is reused (no re-execution)
- **Single in-process FIFO queue** — runtime is single-threaded
- **No async/priority/parallelism in the runtime** — behaviors register in order, executed FIFO

Per the public CONTRACT.md, behaviors **cannot** perform non-deterministic I/O (network, file, time) directly — they must emit a `work.requested` event and rely on a separate worker. This preserves replay property.

## 2. Bottleneck Analysis (per production guide and runtime analysis)

| Rank | Bottleneck | Why |
|---:|---|---|
| 1 | LLM / external tool calls | Latency, cost, rate limits. Especially `FETCH`, `DRAFT`, `SUMMARIZE`, `CLASSIFY`, `REVIEW`, `REWRITE`. |
| 2 | ActiveGraph single-threaded event loop | FIFO processing; if behavior blocks on I/O, run halts. |
| 3 | Candidate explosion in skill recombination | Permutation-style search grows factorially without semantic prefilter. |
| 4 | Event-log amplification | Fork/eval can produce millions of events; replay/storage/diff cost grows. |
| 5 | Pattern/view construction | Wider views = more subscriptions, more context, slower. |
| 6 | Behavioral eval + canalization | Cost is `candidates × testcases × n_runs`. |
| 7 | Human approval / policy gates | Non-technical delay for `SEND`, `TRANSACT`, `DELETE`, side effects. |

Production guide numbers (per docs.activegraph.ai):

- Single Postgres connection: ~thousands of event writes/sec
- Pool of 10 connections with writes across runs: tens of thousands/sec
- Replay 100k-event run on warm cache: single-digit seconds
- Storage: ~1-2 KB per event in JSONB
- Run concurrency limited by connection pool size, not framework
- Runtime itself is single-threaded

## 3. Pattern Extraction (what we adopt)

### 3.1 Pattern A: Serial audit, parallel workers

```text
ActiveGraph runtime → serial deterministic event log, replay, fork, audit
Workers (async)   → LLM, API, MCP, sandbox, embeddings, fetch

Behavior emits `skillgenome.work.requested` (deterministic work_id)
Worker returns `skillgenome.work.completed` (with same work_id)
ActiveGraph reducer behavior merges result deterministically
```

**Why we adopt this:** daily-archive's Track A involves LLM calls (M050, M053), eval (M051-M053), and bounded agent-like operations. A serial audit trail + parallel workers gives us deterministic replay and bounded latency.

**What we don't adopt:** the full Postgres event store and distributed runtime. daily-archive already has M035 SQLite queue with leases/heartbeats/retry, which is the bounded-scope equivalent.

### 3.2 Pattern B: Cascaded gates

```text
cheap screening outside full trace:
  Schema → Type → Safety → Plausibility

full audit trace only for candidates that matter:
  Behavioral eval (sandbox mock) → Behavioral eval (sandbox real) → ActiveGraph fork/diff
```

**Why we adopt this:** our current M035 contracts and M044 guardrail already implement Schema/Type/Safety. Adding Plausibility as a cheap gate (post-type, pre-behavioral) is a 1-day addition. Behavioral eval tiers (mock first, real second) is a 1-2 day addition in M051 or M053.

### 3.3 Pattern C: Race / successive halving

```text
Stage 1: 1 testcase × 1 run для всех plausible candidates
Stage 2: 3 testcases × 1 run для top 30%
Stage 3: full test suite для top 10%
Stage 4: canalization n_runs=5 только для finalists
Stage 5: ActiveGraph fork/diff только для top 3-5
```

**Why we adopt this:** M053 (RLM S10 comparative benchmark) runs vector-only vs one-hop graph vs heuristic BFS. With ~5-10 fixtures, the full cost is small. But the **pattern** of "cheap screen first, expensive only for survivors" is directly applicable: M051 eval fixtures can use this to test fixture sufficiency before M052/M053.

**Note on scale:** at our bounded scale (5-20 articles per batch, dozens of fixtures), race/successive halving is **overkill** as a runtime optimization. We adopt it as a **methodology**, not a runtime pattern.

### 3.4 Pattern D: Content-addressed artifacts

```text
Event payload:
  artifact_uri: cas://sha256...
  artifact_hash: sha256...
  mime_type: ...
  provenance: [...]

NOT:
  full_pdf_base64: ...
  raw_text: ...
```

**Why we adopt this:** this is **already** our pattern. M035 safety flags, M043 sidecar packets, M044 TEI summary hash, M045 trajectory report. No changes needed; we maintain this discipline.

### 3.5 Pattern E: Deterministic work_id + idempotency

```text
work_id = sha256(
  run_id
  + candidate_fingerprint
  + primitive_index
  + input_hash
  + binding_id
  + testcase_id
)
```

**Why we adopt this:** for M050 (LLM helper v2), MiniMax calls need cache-friendly keys. Same `(model_id, prompt_hash, input_hash, binding_id, tool_version)` should produce the same result. **Already partially** in M023 MiniMax helper; **fully formalized** in M049 (models.yaml) + M050.

### 3.6 Pattern F: Deterministic merge of parallel worker results

```text
sort by:
  candidate_fingerprint,
  testcase_id,
  primitive_index,
  work_id

Reducer accepts first `work.completed` with expected work_id; duplicates are no-op.
```

**Why we adopt this:** M035 SQLite queue already has lease/heartbeat semantics; M046 reverse_adr_audit dimension already has explicit ordering. We formalize this in M050 (LLM helper v2 worker pool) — workers can fail and retry, the ActiveGraph-side reducer is idempotent.

### 3.7 Pattern G: Mini event log for bounded scope

```text
SQLite (or DuckDB) event log
  + work_id, primitive, input_hash, binding, model_id, prompt_hash
  + status: requested/started/completed/failed
  + result_uri: cas://sha256...
  + worker_id
  + duration_ms
```

**Why we adopt this:** M035 already has a queue with events. We **don't** need Postgres. The bounded-scope SQLite event log is sufficient for daily-archive's M050-M053.

## 4. Track A Adaptation (concrete mapping to M049-M053)

This is the **core deliverable** of this deep-dive. Each Track A milestone is mapped to specific ActiveGraph patterns and concrete code changes.

### 4.1 M049: Models Registry (1-2 days)

**ActiveGraph pattern:** deterministic work_id (3.5) requires a stable model registry.

**Adaptation:**

- `models.yaml` schema includes fields needed for `work_id`:
  ```yaml
  models:
    - id: minimax-m3-512k-anthropic
      provider: anthropic
      endpoint: https://api.minimax.io/anthropic
      model_name: MiniMax-M3-512k
      tool_version: 2026-05-15
      policy_version: m049-v0.1
    - id: minimax-m3-openai
      provider: openai
      endpoint: https://api.minimax.io/v1
      model_name: MiniMax-M3
      tool_version: 2026-05-15
      policy_version: m049-v0.1
  ```
- `scripts/validate_models_yaml.py` runs at pre-commit (M044 mandatory) — orphan ids, duplicate ids, missing fields
- `src/arxiv_archive/models_registry.py` — `load_models_registry()` returns dict; helpers reference by `id`, not hardcoded strings

**Files:** `models.yaml` (new), `src/arxiv_archive/models_registry.py` (new), `scripts/validate_models_yaml.py` (new), `tests/test_models_registry.py` (new), `minimax_*.py` (modify to use registry)

**Anchor:** D074, R045, Recommendation 6 (M046 07-2026-assessment).

### 4.2 M050: Bounded LLM Helper v2 (1-2 days)

**ActiveGraph pattern:** serial audit, parallel workers (3.1); deterministic work_id (3.5); deterministic merge (3.6); mini event log (3.7).

**Adaptation:**

- `src/arxiv_archive/article_artifact_minimax.py` becomes a **work requester**, not a synchronous LLM caller:
  ```python
  def classify_artifact(artifact_id, *, model_id="minimax-m3-512k-anthropic"):
      work_id = compute_work_id(artifact_id, model_id, ...)
      return emit("skillgenome.work.requested", {
          "work_id": work_id,
          "kind": "classify",
          "input_hash": sha256(artifact_id),
          "model_id": model_id,
          "binding_id": "article-artifact-classify",
      })
  ```
- `src/arxiv_archive/article_artifact_worker.py` (new) — bounded ProcessPoolExecutor (1-2 workers, not distributed). Consumes work requests, calls MiniMax via models registry, emits work.completed with same work_id, persists result in `artifacts/m050-work-requests/<work_id>.json` (content-addressed).
- `src/arxiv_archive/article_artifact_reducer.py` (new) — merges work.completed into article_artifact result. Idempotent: re-receiving same work_id is no-op.
- Worker pool config: `concurrent.futures.ProcessPoolExecutor(max_workers=2)` — bounded, not distributed.
- **No `Any` type** in result schema — explicit `Classification {label, confidence, evidence_refs}` dataclass.

**Safety contract (per ADR-006):**

- Outputs are diagnostic, not promotion authority
- No graph writes, no fact promotion, no event log side effects beyond work events
- All 5 safety defaults remain false in any artifact

**Files:** `src/arxiv_archive/article_artifact_minimax.py` (refactor), `article_artifact_worker.py` (new), `article_artifact_reducer.py` (new), `tests/test_*.py` (new), `artifacts/m050-work-requests/` (new directory)

**Anchor:** R051, R052, ADR-006, M046 Recommendation 4 (reverse_adr_audit), M047 (M044 pre-commit + CI).

### 4.3 M051: Eval Fixtures for RLM (1 day)

**ActiveGraph pattern:** cascaded gates (3.2); deterministic work_id (3.5); content-addressed artifacts (3.4); race/successive halving (3.3) as **methodology**.

**Adaptation:**

- `tests/fixtures/rlm/v0.1/` — 5-10 sample articles with:
  - `article.json` (paper metadata, content hash)
  - `expected_classification.json` (typed, deterministic)
  - `expected_evidence_paths.json` (paper_id → PageIndexNode → chunk path)
  - `fingerprint.json` (sha256 hashes for each expected output)
- `scripts/score_rlm_fixtures.py` — loads fixtures, runs M052 RLM on each, scores against expected, produces `artifacts/m051-rlm-fixtures/score-report.{json,md}`
- **Cascaded gates (methodology):**
  - Tier 1: fixture validity (sha256, schema)
  - Tier 2: M052 RLM run produces Classification
  - Tier 3: scored against expected with deterministic metric
  - Tier 4: (only if Tier 3 fails) full trajectory capture for debugging
- All output is deterministic on same fixtures (R020).

**Files:** `tests/fixtures/rlm/v0.1/`, `scripts/score_rlm_fixtures.py`, `tests/test_score_rlm_fixtures.py`, `artifacts/m051-rlm-fixtures/`

**Anchor:** R020, R022, ADR-006.

### 4.4 M052: RLM S09 Fixture Tests (1-2 days)

**ActiveGraph pattern:** cascaded gates (3.2); deterministic work_id (3.5); mini event log (3.7) for trajectory capture.

**Adaptation:**

- `src/arxiv_archive/rlm_workflow.py` (extend) — bounded RLM tools (read-only, no mutation), typed draft output (e.g., `ArticlePatch` dataclass), trajectory capture (sequence of tool calls + results with work_ids).
- `tests/test_rlm_workflow.py` (extend) — on M051 fixtures, assert typed output, trajectory captured, deterministic.
- **Gate (R020 enforcement):** M052 cannot pass without M051 eval fixtures existing. Verifier asserts `M051 fixtures exist` before M052 test run.

**Safety:** RLM outputs are draft, not committed. ADR-006 binding.

**Files:** `src/arxiv_archive/rlm_workflow.py` (extend), `tests/test_rlm_workflow.py` (extend)

**Anchor:** R022, ADR-006.

### 4.5 M053: RLM S10 Comparative Benchmark (1-2 days)

**ActiveGraph pattern:** race/successive halving (3.3) as **methodology**; parallel forks for finalists (3.1); deterministic work_id (3.5).

**Adaptation:**

- `scripts/benchmark_rlm_baselines.py` (new) — three baselines:
  - **vector-only** (one-shot similarity)
  - **one-hop graph expansion** (read-only LadybugDB)
  - **heuristic BFS** (in-memory)
- Run on M051 fixtures with M053 metrics (recall, evidence_path_hit_rate, grounding proxy, RLM tool usage cost, trajectory capture completeness).
- **Race/successive halving methodology:**
  - Tier 1: 1 testcase × 1 run для всех candidates (all 3 baselines × 5-10 fixtures = 15-30 evals)
  - Tier 2: full test suite для top 30% (3-9 evals)
  - Tier 3: canalization n_runs=5 только для top 1-2 (2-10 evals)
- **Cross-track dependency:** the "one-hop graph" baseline requires M057 (hybrid retrieval production-corpus pilot). If M057 is delayed, M053 falls back to fixture-level one-hop (read-only LadybugDB) per M003 S06.
- `artifacts/m053-rlm-benchmark/benchmark-report.{json,md}` — comparative results with explicit per-tier breakdown.

**Files:** `scripts/benchmark_rlm_baselines.py`, `artifacts/m053-rlm-benchmark/`, `tests/test_rlm_benchmark.py`

**Anchor:** R023, R020 (gate), D080 (trajectory check rerun post-benchmark).

### 4.6 Track A integration summary

```mermaid
flowchart LR
    A[M049: models.yaml] --> B[M050: LLM helper v2]
    B --> C[M051: eval fixtures]
    C --> D[M052: RLM S09 tests]
    D --> E[M053: RLM S10 benchmark]

    B -.uses.-> F[ActiveGraph pattern:<br/>serial audit + parallel workers]
    B -.uses.-> G[Pattern: deterministic work_id]
    C -.uses.-> H[Pattern: cascaded gates]
    C -.uses.-> I[Pattern: content-addressed artifacts]
    D -.uses.-> J[Pattern: mini event log for trajectory]
    E -.uses.-> K[Pattern: race/successive halving]

    A -.provides.-> G
    G -.required.-> F
    F -.integrates.-> M[M035 SQLite queue<br/>(existing)]
    M -.integrates.-> N[M045 trajectory check<br/>(existing)]
```

## 5. What We Don't Adopt

| ActiveGraph feature | Why we don't adopt | What we use instead |
|---|---|---|
| Postgres event store | Overkill for bounded scope | M035 SQLite queue |
| Distributed runtime | Single-assistant / single-machine scope | Local ProcessPoolExecutor (1-2 workers) |
| Full replay/fork/diff infrastructure | We don't have per-candidate graphs | M043 sidecar packets with ready/replay/blocker |
| Replay-100k-events in single-digit seconds | We have ~1000 events per batch | M045 trajectory check on 8 dimensions |
| Fork from cache with shared prefix | Bounded candidate count (10s, not 1000s) | Recompute is fine |
| Graph algorithms (PageRank, CDLP, BFS) | We don't have a graph yet | M056 GraphDB comparison; if FalkorDB, then algorithms |
| Native GraphBLAS / custom semiring | Same as above | Deferred to post-M056 |

## 6. Risks

| Risk | Severity | Mitigation |
|---|---|---|
| M050 worker pool становится bottleneck (1-2 workers, network-bound) | medium | bounded scale, fallback to synchronous if pool unavailable |
| M051 fixtures insufficient to gate M052 | low (this is correct behavior per R020) | M052/M053 deferred if so |
| M053 one-hop baseline blocked by missing M057 | low | fallback to M003 S06 fixture-level one-hop |
| Work_id collisions (extremely unlikely with sha256) | very low | collision detection in reducer |
| Async worker pool hides determinism violations | medium | deterministic merge in reducer; trajectory capture |

## 7. LLM Reading Notes

- **Pattern adoption ≠ runtime adoption.** We borrow the patterns, not the system.
- **M049-M053 form a self-contained Track A** that can be implemented without ActiveGraph runtime.
- **Bounded scale:** daily-archive's M049-M053 operate on 5-50 articles, not thousands of candidates. Patterns are scaled accordingly.
- **Safety contract preserved:** ADR-006 (diagnostic-only) + 5× false safety defaults + reverse_adr_audit dimension (M047) all apply.
- **Cross-references:** see `04-applicability-matrix.md` for the full pattern × milestone matrix.

## 8. Cross-References

- SkillGenome patterns: `02-skillgenome-patterns.md`
- FalkorDB evaluation: `03-falkordb-evaluation.md`
- Applicability matrix: `04-applicability-matrix.md`
- Track A roadmap: M046-3b7gp0 summary; M049-M053 in roadmap
- ADR-006 (Agent Boundary): `doc/adr/m034/ADR-006-agent-boundary.md`
- M044 architecture guardrail: `artifacts/m044-grobid-architecture-guardrail/`
- M047 reverse_adr_audit: `artifacts/m046-synthesis/05-evidence-safety.md`
