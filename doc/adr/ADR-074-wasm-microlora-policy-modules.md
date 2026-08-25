# ADR-074: WASM MicroLoRA as Swappable FSM Policy Modules

- Status: Accepted (W1 record + W2 patterns + W3 host shipped; LoRA still D136)
- Date: 2026-08-25
- Updated: 2026-08-25 (M453: parse-pdf consults grammar_route)
- Deciders: repo owner + agent
- Tags: wasm, microlora, sona, fsm, self-learning, policy-modules, sandbox, hot-swap
- Sources: ruvector/crates/sona (1638 nodes), ruvector rvf-* family,
  ruflo harness docs, micro-hnsw-wasm

## 0. One-line Decision

**Learned routing/admission policies for the parsing FSM are packaged as versioned WASM MicroLoRA modules — hot-swappable, sandboxed, rollback-able artifacts that plug into defined FSM decision points without restarting the pipeline or recompiling the host.**

## 1. Context

The audit + RLM projection established that our FSM needs learned policies at
specific decision points (routing, admission, scheduling). The question was:
Rust-native SONA vs WASM-packaged SONA. The initial assessment dismissed WASM
as unnecessary; the user correctly pushed back.

### 1.1 What Rust-native gives us

- Direct function calls, zero overhead
- Type safety at compile time
- Simple build (one Cargo.toml)

### 1.2 What Rust-native CANNOT give us

- **Hot-swap without restart**: a learned policy update requires recompilation
  and process restart. Our batch runner processes 639 articles; restarting
  mid-batch to apply a new routing policy is unacceptable.
- **Sandboxed execution of LLM-proposed plans**: the RLM pattern has LLM
  proposing operations. If those operations are compiled into the binary,
  adding new ones = recompile. If they're WASM modules, the LLM can propose
  a plan referencing any registered module.
- **Versioned policy artifacts with rollback**: "the policy that routed this
  article was v3" is a reproducible fact only if the policy is an immutable
  artifact. Compiled-in policies change with every release.
- **Multi-domain isolation**: math/CS/biology parsers may need different
  routing policies. Native code = one policy for all. WASM = per-domain
  modules loaded on demand.
- **Resource isolation**: a buggy adapter corrupting shared state is
  impossible in WASM linear memory.

## 2. Decision

### 2.1 Architecture: three layers

```
┌─────────────────────────────────────────────────────────┐
│ Layer 3: POLICY ARTIFACTS (.wasm modules)               │
│   data/policies/<domain>/<task>.wasm                    │
│   Each module: SONA MicroLoRA weights + inference fn    │
│   Versioned by content hash (sha256) in MANIFEST.yaml   │
│   Rollback = point manifest at previous hash            │
├─────────────────────────────────────────────────────────┤
│ Layer 2: FSM DECISION POINTS (Rust trait objects)       │
│   trait FsmPolicy {                                     │
│     fn route(&self, features: &ArticleFeatures) -> Route│
│     fn admit(&self, quality: &QualitySignal) -> bool    │
│     fn schedule(&self, load: &LoadProfile) -> Concurr   │
│   }                                                     │
│   Implementation delegates to loaded WASM module        │
├─────────────────────────────────────────────────────────┤
│ Layer 1: FSM CORE (deterministic Rust)                  │
│   Stage transitions, lifecycle events, guards           │
│   Calls Layer 2 at defined decision points              │
│   Never trusts Layer 3 directly (sandbox boundary)      │
└─────────────────────────────────────────────────────────┘
```

### 2.2 Policy lifecycle (SONA Loop A/B/C mapped to ours)

| SONA loop | Our equivalent | Artifact |
|---|---|---|
| Loop A (instant): per-trajectory MicroLoRA | grounding_hint after each summary → immediate route adjustment | in-memory, not persisted |
| Loop B (background): hourly pattern extraction | hourly: extract patterns from accumulated CVF trajectories → train MicroLoRA → export .wasm | `data/policies/<domain>/route.v{N}.wasm` |
| Loop C (deep): weekly EWC++ consolidation | weekly: consolidate all domain policies → full retrain → validate on frozen eval → promote | `data/policies/<domain>/route.v{N+1}.wasm` |

### 2.3 Decision points in the parsing FSM

| Decision point | Input features | Output | Learned from |
|---|---|---|---|
| **grammar_route** | page_count, toc_bookmarks, text_layer_quality | page_outline / xy_cut / grobid_structural | historical parse outcomes per grammar |
| **batch_admission** | article_age, openalex_status, citation_count, domain | accept / defer / skip | enrichment success rate over time |
| **concurrency_tune** | doc_size, grobid_latency_p50, error_rate_1h | workers: 1..16 | throughput measurements |
| **retry_policy** | upstream, hour_of_day, recent_error_rate | backoff_ms, max_attempts | GROBID/OpenAlex failure patterns |
| **summary_depth** | section_count, avg_section_chars, domain | full / per-section-lazy / skip | summary quality × cost tradeoff |

Each decision point gets its own MicroLoRA adapter trained on historical
trajectories. Small adapters (rank-2, ~few KB) → fast inference (<3μs).

### 2.4 Why WASM specifically (not just "separate process")

| Property | WASM module | Separate process | Compiled-in |
|---|---|---|---|
| Hot-swap | ✓ (drop file, reload) | ✓ (restart proc) | ✗ (recompile) |
| Sandboxed memory | ✓ linear mem isolated | ✓ OS-level | N/A |
| Startup latency | ~μs (instantiate) | ~ms (fork+exec) | 0 |
| Versioned artifact | ✓ .wasm file + hash | ✓ binary + hash | ✗ |
| Per-domain loading | ✓ load on demand | ✓ spawn per domain | ✗ one for all |
| Resource limits | ✓ fuel/epoch interruption | ✓ cgroups | N/A |
| LLM can propose plans referencing it | ✓ (module registry lookup) | ✗ | ✗ |

### 2.5 Training data flow (SONA W1 → W2)

Phase W1 (now): record trajectories only. Every `run_summaries_agent`,
every `parse_pdf`, every `enrich_layer` writes a `TrajectoryRecord`
(already exists in da-ports). These accumulate in CVF memory.

M453: `da source parse-pdf` and FSM parsed-stage consult `grammar_route`
(fail-open). `GrammarRoute` maps to FSM `grammar_profile` via DATA
`profile_map` in `data/policies/registry.yaml`. Manifest stamps
`grammar_route` + `policy_hash`. `page_outline` + TOC observation cache
→ `parse-pdf-toc`; XY-Cut has no CLI yet and keeps `parse-pdf`.

Phase W2 (after ~1000 trajectories): ReasoningBank pattern extraction
(K-means++ clustering by article_features embedding). Patterns with
avg_quality > threshold become training inputs for MicroLoRA adapters.

Phase D136 gate: LoRA/fine-tuning requires frozen eval + explicit user
approval. The WASM packaging makes this concrete: the eval runs against
a specific `.wasm` hash, not against "the current code".

## 3. Pros and Cons (honest assessment)

### Pros

1. **Zero-downtime policy updates**: swap `.wasm` mid-batch; next article uses new policy
2. **Reproducible decisions**: `policy_hash` stamped into trajectory records → full replay
3. **Per-domain specialization**: math parser learns different routing than CS parser
4. **LLM-plan integration**: RLM plans reference named policy modules; no compile step
5. **Rollback safety**: bad adaptation detected by fidelity-audit regression → revert hash
6. **Sandbox security**: LLM-proposed policies execute in WASM jail, not native code
7. **SONA compatibility**: SONA already exports safetensors; WASM wrapper is thin
8. **Future browser deployment**: same .wasm runs in explorer UI for local inference

### Cons

1. **Compute overhead**: WASM ~1.2–2× slower than native for tight loops;
   MicroLoRA inference is <3μs natively → <6μs in WASM. At our scale (per-article
   decisions, not per-token), negligible.
2. **Build complexity**: two targets (native + wasm32); cargo-wasm setup;
   testing matrix doubles. Mitigated by `wasm-pack build --target web`.
3. **Ecosystem maturity**: wasm-bindgen/anyhow integration still evolving.
   SONA already ships wasm bindings — risk is low but nonzero.
4. **Debugging**: WASM stack traces less readable than native (v0 mangling helps).
5. **Overkill for Phase W1**: recording trajectories doesn't need WASM.
   Only Phase W2+ (learned policies deployed) justifies it.

## 4. What we do NOT adopt

- Full ruflo harness (100+ agents, swarm coordination): we have GSD.
- IPFS model distribution: premature; local filesystem suffices.
- Neuromorphic SNN (micro-hnsw-wasm spiking networks): fascinating but
  no production use case at our scale.
- Browser-only inference path: explorer UI is not our primary surface.

## 5. Staged adoption plan

| Phase | Gate | Deliverable |
|---|---|---|
| W1 (now) | none — record only | TrajectoryRecord accumulation (already shipping) |
| W2 (~1000 trajs) | D136 user approval | ReasoningBank pattern extraction; first MicroLoRA trained |
| W3 (W2 stable) | fidelity-audit PASS with learned policy ≥ hand-tuned | WASM packaging; hot-swap infrastructure; per-domain modules |
| W4 (multi-domain) | corpus >1000 objects across ≥4 domains | Domain-specialized routing; cross-domain transfer evaluation |

## 6. References

- Sergey Vasiliev, *Knowledge Graph, Memory Graph and Context Graph*
  (2026-08-17). Captured source:
  `data/sources/blog/vasiliev2026knowledge-graph-memory-graph/`.
  Mapping: WASM/SONA policies are memory-graph artifacts (experience),
  not governed KG truth. Context consult at parse-pdf is a context-graph
  projection (task-time), fail-open, hashed.
- SONA engine: begin_trajectory / end_trajectory / submit_trajectory /
  apply_micro_lora / find_patterns / force_learn (crates/sona/src/engine.rs)
- Three-tier loops: InstantLoop / BackgroundLoop / LoopCoordinator
  (crates/sona/src/loops/)
- StalenessSchedule w(d): k1=16, k2=64, λ=0.08 (auto_tuner.rs)
- SafeTensorsExporter: export_lora_safetensors (export/safetensors.rs)
- RVF agent-experience store: rvf-runtime (Ed25519-signed, COW branching)
- ruflo WASM neural training: `neural train --wasm --flash --contrastive`
  (MicroLoRA 2.6μs measured, FlashAttention 9127 ops/s)
- micro-hnsw-wasm: 7.2KB HNSW+SNN (not adopted, recorded for edge case)
