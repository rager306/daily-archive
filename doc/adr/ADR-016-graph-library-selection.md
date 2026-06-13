# ADR-016: Graph Library Selection for M060b-M064+

**Status:** Accepted (binding)  
**Amendment Status:** Amended 2026-06-13 by M062-b4porb S01  
**Date:** 2026-06-13  
**Deciders:** agent  
**Milestone:** M061-0fib2i; amended by M062-b4porb S01  
**Scope:** graph-layer / algorithm-backend / benchmark / diagnostics / graphdb-selection  
**Binding Level:** binding supplement to ADR-010, ADR-011, ADR-013  
**Revisable:** yes, after M063 GraphDB-selection evidence or M064+ production-gate evidence shows a different scale or substrate requirement

> Amendment note: Originally accepted with rustworkx; amended 2026-06-13 to drop rustworkx per M062-b4porb S01 follow-up. User rationale: keep graph layer simple (1 primary + 1 supplementary, not 3 libs).

## Amendment Log

| Date | Milestone | Change | Rationale |
|---|---|---|---|
| 2026-06-13 | M062-b4porb S01 | Dropped rustworkx from the adopted graph-library set and kept NetworkX + igraph only. | Keep the graph layer simple: one primary library plus one supplementary accelerator, not three graph libraries. |

## 0. One-line Decision

> We will keep NetworkX as the primary graph representation and correctness baseline, and adopt igraph as the single supplementary read-only accelerator for algorithm-heavy graph operations.

We will not replace NetworkX as the canonical graph layer in M060b-M062, adopt rustworkx, adopt graph-tool/PyG/DGL/NetworkX-Temporal for runtime integration now, or treat GraphScope as authorized outside a future M063 GraphDB-selection evaluation.

## 1. Context

The project has a manifest-first graph layer for diagnostics and algorithms, not a production graph import path. M060c S01 benchmarked NetworkX, igraph, and rustworkx across BFS, PageRank, shortest path, and connected components using the M058 4-layer graph and synthetic graphs. The benchmark showed both igraph and rustworkx can be faster than NetworkX on some heavy operations.

The follow-up decision in M062-b4porb S01 deliberately narrows the adopted set. The project needs the graph layer to stay easy for future agents to reason about: one readable primary representation and one supplementary accelerator are enough at the current scale. Adding a third library would increase conversion, parity, dependency, and API-shape obligations without a proven need that igraph cannot satisfy.

The next milestones need a safe answer to two questions:

- which library should author and validate the graph representation;
- which library may accelerate read-only algorithm workloads without authorizing writes or production import.

The safety posture remains unchanged: Graph writes are not authorized. Production import is not authorized. Fact promotion is not authorized. External network default is disabled. LLM calls default is disabled.

### Context Map

```mermaid
flowchart TD
    A[M060c S01 benchmark] --> B[NetworkX baseline]
    A --> C[igraph speed evidence]
    A --> D[rustworkx speed evidence]
    B --> E[M060b intermediate layer]
    C --> F[M061 algorithm-heavy reads]
    D --> G[M062 simplification review]
    G --> H[rustworkx not adopted]
    F --> I[M063 GraphDB selection]
    I --> J[M064+ production gate]
```

## 2. Decision

We will keep NetworkX as the primary graph representation, fixture surface, and correctness baseline.

We will adopt igraph as the only supplementary read-only accelerator for algorithm-heavy operations where conversion cost is justified by benchmark evidence. In compatibility terms, this preserves the binding phrase: igraph as the supplementary read-only accelerator. M060b and M061 may use igraph for heavy diagnostic/algorithm paths while preserving NetworkX parity checks.

We will not adopt rustworkx in the M060b-M064+ graph layer at this time. Its performance evidence is useful background, but igraph already covers the supplementary accelerator role with a broader algorithm surface. rustworkx also adds overlap with igraph, a less mature ecosystem fit for this project, and an integer-index API mismatch that makes future agent maintenance harder.

### Adoption Table

| Library | Decision | Authorized role | Rationale |
|---|---|---|---|
| NetworkX | PRIMARY | Read-only control-plane graph representation, manifest validation, fixture surface, simple algorithms, and correctness baseline. | Most readable and already aligned with project diagnostics. |
| igraph | ADOPTED supplementary | Heavy algorithm operations such as Leiden, PageRank, and BFS at 10k+ nodes when benchmark evidence justifies conversion. | Covers the needed high-scale algorithm role while keeping the supplementary set to one library. |
| rustworkx | NOT ADOPTED | None for runtime adoption in M060b-M064+. Historical benchmark evidence only. | Overlaps with igraph, is less mature for this project context, and has an integer-index API mismatch with the manifest-first graph layer. |
| graph-tool | DEFER | Future high-scale comparator only. | Conda/system packaging friction is greater than the speedup value at the current scale. |
| PyG / DGL / NetworkX-Temporal / GraphScope | DEFER to M065+ | Future ML, temporal, distributed, or GraphDB-selection research only. | Current work is deterministic read-only graph diagnostics, not GNN, temporal, or distributed graph processing. |

This decision authorizes:

- deterministic conversion from the canonical NetworkX/control graph into igraph for read-only analysis;
- benchmark-backed use of igraph in M060b and M061;
- NetworkX as the default library for read-only control-plane operations;
- historical comparison against rustworkx benchmark artifacts without adopting it as a runtime dependency.

This decision does not authorize:

- graph writes;
- production graph import;
- fact promotion;
- GraphDB selection;
- adoption of rustworkx, graph-tool, PyG, DGL, NetworkX-Temporal, or GraphScope as runtime dependencies for M060b-M062.

### Decision Boundary

```mermaid
flowchart LR
    IN[In scope] --> D[ADR-016 amended]
    D --> OUT[Out of scope]
    IN --> I1[Read-only algorithms]
    IN --> I2[Library applicability matrix]
    IN --> I3[Benchmark-backed igraph acceleration]
    OUT --> O1[Production graph import]
    OUT --> O2[GraphDB writes]
    OUT --> O3[Runtime rustworkx adoption]
    OUT --> O4[GNN runtime adoption]
```

## 3. Applies To

This decision applies to:

- M060b intermediate graph layer work;
- M061 2-hop BFS and algorithm-heavy graph diagnostics;
- M062 fd hardening where graph operations remain deterministic and read-only;
- M063 GraphDB-selection comparison methodology;
- M064+ production gating for graph acceleration;
- future agents choosing graph libraries for read-only operations.

### Applicability Diagram

```mermaid
flowchart TB
    ADR[ADR-016 amended] --> A[NetworkX primary]
    ADR --> B[igraph supplementary]
    ADR --> C[rustworkx not adopted]
    ADR --> D[Deferred libraries]
    D --> E[graph-tool]
    D --> F[PyG/DGL]
    D --> G[NetworkX-Temporal]
    D --> H[GraphScope]
```

## 4. Requirements and Decisions Impacted

### Requirements

| Requirement | Impact | Notes |
|---|---|---|
| M060c benchmark evidence | supports | S01 produced benchmark and research evidence; S02 binds library applicability after simplification. |
| M061 2-hop BFS proof | constrains | Must keep NetworkX parity while allowing igraph acceleration where needed. |
| M062 fd hardening | supports | Defaults to NetworkX for clear read-only diagnostics; igraph only for measured hot paths. |
| M063 GraphDB selection | constrains | igraph is a comparator, not a GraphDB substitute; rustworkx is not an adopted comparator backend. |
| M064+ production | constrains | Production use requires explicit gate approval and packaging proof. |

### Decisions

| Decision | Impact | Notes |
|---|---|---|
| ADR-010 | consistent | Keeps graph traversal and diagnostic semantics read-only. |
| ADR-011 | consistent | Maintains fd/content-graph safety boundaries. |
| ADR-013 | consistent | Keeps manifest-driven ingest as the canonical source of graph evidence. |
| ADR-016 original acceptance | narrowed | Removes rustworkx from the adopted set while preserving NetworkX and igraph. |

## 5. Options Considered

### Option A — NetworkX only

| Dimension | Assessment |
|---|---|
| Local-first fit | High |
| Safety fit | High |
| Complexity | Low |
| Reversibility | High |
| GraphDB portability | Medium |
| Agent/tooling dependency | Low |
| Human review compatibility | High |

**Pros**

- Already present and readable.
- Lowest conversion risk.
- Best correctness baseline.

**Cons**

- Benchmark evidence shows slower heavy algorithms.
- Risks making M061+ scale work artificially slow.

### Option B — NetworkX primary with igraph supplementary

| Dimension | Assessment |
|---|---|
| Local-first fit | High |
| Safety fit | High |
| Complexity | Medium-Low |
| Reversibility | High |
| GraphDB portability | High |
| Agent/tooling dependency | Medium-Low |
| Human review compatibility | High |

**Pros**

- Keeps the readable NetworkX baseline.
- Uses S01 benchmark evidence for heavy operations.
- Preserves fallback and parity checks.
- Keeps the graph layer to one primary and one supplementary library.
- Avoids premature GraphDB or distributed-system adoption.

**Cons**

- Requires conversion boundaries.
- Requires igraph availability checks.
- Adds a parity-test obligation for accelerated paths.

### Option C — NetworkX primary with igraph and rustworkx supplementary

| Dimension | Assessment |
|---|---|
| Local-first fit | High |
| Safety fit | Medium |
| Complexity | Medium-High |
| Reversibility | Medium |
| GraphDB portability | High |
| Agent/tooling dependency | Medium-High |
| Human review compatibility | Medium |

**Pros**

- Maximizes optional speed choices for traversal and path workloads.
- Preserves the original benchmark-backed broad accelerator set.

**Cons**

- Adds a third graph library without current necessity.
- Overlaps heavily with igraph.
- Introduces integer-index API mismatch and additional parity/fallback obligations.
- Makes future agent reasoning and dependency handling harder.

### Option D — Replace NetworkX with a faster library

| Dimension | Assessment |
|---|---|
| Local-first fit | Medium |
| Safety fit | Medium |
| Complexity | High |
| Reversibility | Medium |
| GraphDB portability | Medium |
| Agent/tooling dependency | High |
| Human review compatibility | Medium |

**Pros**

- Maximizes speed on some graph operations.
- Simplifies hot-path selection if one backend dominates.

**Cons**

- Discards the clearest correctness surface.
- Forces conversion of existing diagnostics and tests.
- Premature before M061 and M063 evidence.

### Option E — Adopt graph-tool, GNN, temporal, or distributed graph libraries now

| Dimension | Assessment |
|---|---|
| Local-first fit | Low to Medium |
| Safety fit | Low to Medium |
| Complexity | High |
| Reversibility | Low to Medium |
| GraphDB portability | Varies |
| Agent/tooling dependency | High |
| Human review compatibility | Medium |

**Pros**

- Some options may matter for future high-scale, temporal, distributed, or ML graph work.

**Cons**

- Current use case is deterministic read-only graph analytics.
- Packaging and operational cost are not justified now.
- graph-tool has conda/system friction greater than the speedup value at the current scale.
- PyG/DGL are GNN-focused, not primary graph diagnostics layers.
- GraphScope is too heavy except as a future GraphDB-selection candidate.

## 6. Trade-off Analysis

| Trade-off | Chosen side | Why |
|---|---|---|
| Correctness baseline vs speed | NetworkX baseline plus igraph accelerator | Keeps reviewability while using benchmark evidence for hot paths. |
| One library vs multiple backends | One primary plus one supplementary backend | Captures most current value without three-library complexity. |
| Maximum optional speed vs maintenance clarity | Maintenance clarity | Future agents need a small, obvious graph stack more than another overlapping accelerator. |
| Runtime breadth vs dependency safety | Defer extra runtimes | graph-tool, PyG, DGL, NetworkX-Temporal, GraphScope, and rustworkx need stronger future evidence before adoption. |

## 7. Consequences

### Positive

- Future agents have a binding default: NetworkX first, igraph only for read-only acceleration.
- M061 can optimize heavy algorithms without changing graph semantics.
- M063 can compare GraphDB candidates against a stable in-process baseline.
- The graph layer avoids a third runtime dependency and its parity surface.

### Negative

- rustworkx benchmark wins remain historical evidence rather than an adopted runtime path.
- Conversion/parity tests are still required when using igraph.
- Binary-package availability can affect optional accelerator behavior.
- The project must avoid treating speed benchmarks as authorization for writes.

### New obligations

- Any igraph path must keep a NetworkX parity or fallback check.
- Any production use must prove packaging, fallback, and diagnostic behavior first.
- Applicability decisions must remain in artifacts before runtime adoption.
- Future reconsideration of rustworkx requires new evidence that igraph cannot cover the need.

### What becomes harder

- Some traversal/path hot spots may need igraph tuning instead of switching to rustworkx.
- Agents must distinguish graph library selection from GraphDB selection.

## 8. Safety and Non-Authorization

This ADR does not authorize graph writes, production graph import, fact promotion, GraphDB selection, external network calls, or LLM calls.

This ADR does **not** authorize:

- production graph import;
- graph writes;
- LadybugDB, FalkorDB, HelixDB, or other GraphDB writes;
- fact promotion;
- parser output as graph-ready truth;
- external network calls;
- LLM calls;
- adoption of rustworkx, graph-tool, PyG, DGL, NetworkX-Temporal, or GraphScope for M060b-M062 runtime work.

Required safety defaults:

```text
graph_writes_authorized=false
production_import_authorized=false
fact_promotion_authorized=false
external_network_enabled=false
llm_calls_enabled=false
```

Safety statements:

- Graph writes are not authorized.
- Production import is not authorized.
- Fact promotion is not authorized.
- External network default is disabled.
- LLM calls default is disabled.

Any local-only check must bind to `127.0.0.1`.

### Safety Gate

```mermaid
flowchart LR
    A[NetworkX graph] --> B[Optional conversion]
    B --> C[igraph read-only algorithm]
    C --> D[Parity/fallback check]
    D --> E{Explicit production authorization?}
    E -- no --> F[No-write boundary]
    E -- yes --> G[Future production gate]
```

## 9. Contract Impact

Affected contracts:

- `BenchmarkResult`
- `GraphLayerDiagnostic`
- `GraphAlgorithmBackend`
- `SafetyDefaults`
- `GraphReadinessHandoff`

Required contract changes or drafts:

- Add an explicit backend label when algorithm outputs come from igraph.
- Preserve NetworkX fallback metadata for accelerated outputs.
- Preserve safety defaults in every decision artifact that cites this ADR.
- Do not define rustworkx as an adopted `GraphAlgorithmBackend` without a future amendment.

### Contract Relationship Map

```mermaid
classDiagram
    class GraphLayerDiagnostic {
      +graph_id
      +backend
      +algorithm
      +input_hash
    }

    class GraphAlgorithmBackend {
      +name
      +mode
      +fallback_backend
    }

    class SafetyDefaults {
      +graph_writes_authorized
      +production_import_authorized
      +fact_promotion_authorized
      +external_network_enabled
      +llm_calls_enabled
    }

    GraphLayerDiagnostic --> GraphAlgorithmBackend
    GraphAlgorithmBackend --> SafetyDefaults
```

## 10. Validation and Evidence

Evidence already produced:

- `artifacts/m060c-benchmark/benchmark.json`
- `artifacts/m060c-benchmark/benchmark.md`
- `artifacts/m060c-benchmark/library-research/*.md`
- `artifacts/m060c-benchmark/applicability-matrix.json`
- `artifacts/m060c-benchmark/applicability-matrix.md`

Future validation required:

- M061 parity tests for NetworkX vs igraph where accelerators are used;
- M062 diagnostic checks that keep safety defaults false;
- M063 GraphDB-selection matrix that does not confuse algorithm libraries with databases;
- M064+ production proof before any accelerator becomes a production dependency.

### Validation Path

```mermaid
flowchart TD
    A[ADR-016 amended] --> B[M061 parity tests]
    B --> C[M062 hardening diagnostics]
    C --> D[M063 GraphDB matrix]
    D --> E{Production authorization?}
    E -- no --> F[Keep read-only supplementary use]
    E -- yes --> G[M064+ production gate]
```

## 11. Open Questions

| Question | Owner | Needed by | Blocking? |
|---|---|---|---|
| Does M061 2-hop BFS perform adequately with NetworkX plus igraph only? | agent | M061/M062 | no |
| Does M063 require a separate GraphDB benchmark suite? | agent | M063 | yes |
| What package constraints apply before M064+ production use? | agent | M064+ | yes |
| What new evidence would justify reconsidering rustworkx after M064? | agent | M065+ | no |

## 12. Follow-up Actions

- [ ] In M061, add parity tests for any accelerated 2-hop BFS path.
- [ ] In M062, keep NetworkX as the default for fd hardening diagnostics unless a measured hot spot appears.
- [ ] In M063, evaluate GraphDB candidates separately from in-process graph algorithm libraries.
- [ ] In M065+, reconsider rustworkx only if igraph cannot satisfy a measured algorithm requirement.
- [ ] Before M064+ production, require explicit authorization and packaging/fallback proof.

## 13. Supersedes / Superseded By

### Supersedes

- Original ADR-016 adopted rustworkx as an optional supplementary accelerator. This amended ADR narrows that choice and supersedes only that part of the original text.

### Superseded By

- Empty until future ADR.

## 14. LLM Reading Notes

This section is intentionally explicit for future agents.

- Binding decision:
  - NetworkX remains the primary graph representation and correctness baseline.
  - igraph is adopted as the single supplementary read-only accelerator for algorithm-heavy M060b and M061 work.
  - rustworkx is not adopted for runtime use in M060b-M064+.
- Do not infer:
  - This ADR does not authorize graph writes, production graph import, fact promotion, GraphDB selection, external network calls, or LLM calls.
  - This ADR does not authorize rustworkx, graph-tool, PyG, DGL, NetworkX-Temporal, or GraphScope runtime integration for M060b-M062.
  - GraphScope may be considered in M063 only as a GraphDB-selection candidate.
- Safe next action:
  - Use NetworkX for read-only control operations and add igraph only behind benchmark-backed parity checks.
- Blocked until:
  - Any production adoption is blocked until M064+ has explicit authorization, packaging proof, fallback proof, and safety-default verification.
