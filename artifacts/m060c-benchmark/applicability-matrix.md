# M060c S02 Applicability Matrix

This artifact compares 8 graph libraries across 5 future milestones. It is a decision aid only; it does not authorize graph writes or production imports.

## Safety defaults

- `graph_writes_authorized=false`
- `production_import_authorized=false`
- `fact_promotion_authorized=false`
- `external_network_enabled=false`
- `llm_calls_enabled=false`

Safety statements:

- Graph writes are not authorized.
- Production import is not authorized.
- Fact promotion is not authorized.
- External network default is disabled.
- LLM calls default is disabled.

Loopback host for any local-only checks: `127.0.0.1`.

## Aggregate score counts

Count of milestone cells with `applicability_score >= 2`.

| Library | Cells with score >= 2 | Decision posture |
|---|---:|---|
| NetworkX | 5 | Primary baseline |
| igraph | 5 | Adopt supplementary |
| rustworkx | 3 | Adopt optional supplementary |
| graph-tool | 0 | Defer / do not adopt now |
| PyG | 0 | Defer / do not adopt now |
| DGL | 0 | Defer / do not adopt now |
| NetworkX-Temporal | 0 | Defer / do not adopt now |
| GraphScope | 1 | Defer except M063 evaluation |

## 8 libraries x 5 milestones matrix

| Library | Milestone | Score | Use-case fit | Integration cost | Decision |
|---|---|---:|---|---|---|
| NetworkX | M060b (intermediate layer) | 3 | Canonical authoring and readable control graph for the intermediate layer. | Low; already used as the baseline and requires no new dependency. | Keep as primary graph representation. |
| NetworkX | M061 (2-hop BFS) | 2 | Reliable correctness baseline for 2-hop BFS and regression checks. | Low; slower than igraph/rustworkx on heavy operations. | Use as control path; accelerate only measured hot spots. |
| NetworkX | M062 (fd hardening) | 3 | Best fit for deterministic read-only hardening and reviewable diagnostics. | Low; mature API and no conversion boundary. | Use as primary library. |
| NetworkX | M063 (GraphDB selection) | 2 | Portable reference model for comparing GraphDB candidates. | Low; does not solve persistence or query substrate choice. | Use as benchmark harness and semantic control. |
| NetworkX | M064+ (production) | 2 | Safe control-plane graph for production checks, not the only scaling path. | Low; performance limits remain for larger algorithm-heavy jobs. | Keep primary for read-only control operations. |
| igraph | M060b (intermediate layer) | 3 | Strong supplementary backend for PageRank/components and other algorithm-heavy reads. | Medium; requires conversion from the NetworkX/control representation. | Adopt as supplementary accelerator. |
| igraph | M061 (2-hop BFS) | 3 | Measured 5-10x-class speedups on heavy operations, with especially strong PageRank/components results. | Medium; keep NetworkX parity tests around conversion. | Use for algorithm-heavy 2-hop BFS adjacent analysis where benchmarks justify it. |
| igraph | M062 (fd hardening) | 2 | Useful for heavy diagnostic scans, but not needed for authoring or safety gates. | Medium; conversion adds another failure surface. | Use only for measured hot paths. |
| igraph | M063 (GraphDB selection) | 2 | Good in-process comparator before choosing an external GraphDB substrate. | Medium; remains an algorithm library, not a database. | Use as benchmark comparator, not as GraphDB replacement. |
| igraph | M064+ (production) | 2 | Candidate production accelerator after explicit performance and packaging proof. | Medium; binary packaging and parity checks required. | Allow as supplementary read-only accelerator after gate approval. |
| rustworkx | M060b (intermediate layer) | 2 | Useful low-latency traversal/path backend for selected heavy reads. | Medium; less direct authoring ergonomics than NetworkX. | Adopt as optional supplementary accelerator. |
| rustworkx | M061 (2-hop BFS) | 3 | Strong fit for BFS and shortest-path hot spots when available. | Medium; conversion and parity checks required. | Use for BFS/path hot paths if local availability remains stable. |
| rustworkx | M062 (fd hardening) | 1 | Niche fit for traversal diagnostics only. | Medium; not worth broadening unless a hot spot appears. | Defer except for measured traversal bottlenecks. |
| rustworkx | M063 (GraphDB selection) | 1 | Useful performance comparator, but not a GraphDB substrate. | Medium; does not address persistence/query requirements. | Use only in benchmark comparisons. |
| rustworkx | M064+ (production) | 2 | Production accelerator candidate for traversal/path workloads. | Medium; Rust extension packaging and fallback path required. | Allow as optional read-only accelerator after gate approval. |
| graph-tool | M060b (intermediate layer) | 0 | Potentially fast but not vendored or source-verified in S01. | High; conda/system-package friction is disproportionate now. | Do not adopt. |
| graph-tool | M061 (2-hop BFS) | 1 | May be valuable only if pip-installable accelerators miss latency targets. | High; runtime packaging risk remains unresolved. | Defer pending scale failure evidence. |
| graph-tool | M062 (fd hardening) | 0 | No hardening benefit over already available libraries. | High. | Do not use. |
| graph-tool | M063 (GraphDB selection) | 1 | Could be a performance reference, not a GraphDB decision. | High. | Revisit only as a later benchmark candidate. |
| graph-tool | M064+ (production) | 1 | Possible future high-performance backend if packaging becomes acceptable. | High; deployment complexity blocks adoption now. | Deferred. |
| PyG | M060b (intermediate layer) | 0 | GNN/tensor workflow mismatch for deterministic graph diagnostics. | High; model/data-loader stack is unnecessary. | Do not adopt. |
| PyG | M061 (2-hop BFS) | 0 | Not a direct BFS/read-only graph analytics surface. | High. | Do not use. |
| PyG | M062 (fd hardening) | 0 | No fit for fd hardening. | High. | Do not use. |
| PyG | M063 (GraphDB selection) | 1 | Only relevant if a future GNN requirement appears. | High. | Defer. |
| PyG | M064+ (production) | 1 | Possible future ML layer, not current graph substrate. | High. | Out of scope until ML requirement exists. |
| DGL | M060b (intermediate layer) | 0 | Deep-learning graph framework is mismatched for lightweight read-only analytics. | High; dependency and data-model overhead. | Do not adopt. |
| DGL | M061 (2-hop BFS) | 0 | Not a clean replacement for deterministic BFS diagnostics. | High. | Do not use. |
| DGL | M062 (fd hardening) | 0 | No direct fit for fd hardening. | High. | Do not use. |
| DGL | M063 (GraphDB selection) | 1 | Relevant only for a future GNN/heterograph evaluation. | High. | Defer. |
| DGL | M064+ (production) | 1 | Possible future ML substrate, not current production graph layer. | High. | Out of scope until ML requirement exists. |
| NetworkX-Temporal | M060b (intermediate layer) | 1 | Conceptually adjacent, but current graph is typed evidence rather than time-sliced state. | Low-to-medium; extends NetworkX but adds premature modeling. | Defer. |
| NetworkX-Temporal | M061 (2-hop BFS) | 0 | No direct acceleration for 2-hop BFS. | Medium. | Do not use. |
| NetworkX-Temporal | M062 (fd hardening) | 1 | Could model temporal hardening later, but not needed now. | Medium. | Defer. |
| NetworkX-Temporal | M063 (GraphDB selection) | 1 | Temporal semantics may inform future requirements, not substrate selection now. | Medium. | Defer. |
| NetworkX-Temporal | M064+ (production) | 1 | Possible future temporal layer if requirements become time-sliced. | Medium. | Defer until temporal requirement is explicit. |
| GraphScope | M060b (intermediate layer) | 0 | Distributed graph system is too heavy for the intermediate layer. | High; operational footprint is disproportionate. | Do not adopt. |
| GraphScope | M061 (2-hop BFS) | 0 | Distributed execution is unnecessary for current 2-hop BFS scale. | High. | Do not use. |
| GraphScope | M062 (fd hardening) | 0 | No fit for fd hardening. | High. | Do not use. |
| GraphScope | M063 (GraphDB selection) | 2 | Potentially relevant only if GraphDB selection requires distributed analytics comparison. | High; repo was not available in GitNexus during S01. | Evaluate as a candidate only during GraphDB selection. |
| GraphScope | M064+ (production) | 1 | Future distributed option if single-process libraries fail production scale. | High. | Defer until production scale proves need. |

## Binding recommendation

- NetworkX remains the primary graph representation and correctness baseline.
- igraph is adopted as a supplementary read-only accelerator for algorithm-heavy operations in M060b and M061.
- rustworkx is adopted as an optional supplementary read-only accelerator for traversal/path hot spots when available.
- graph-tool, PyG, DGL, NetworkX-Temporal, and GraphScope are not authorized for runtime integration by this artifact.
- GraphScope may be evaluated during M063 only as a GraphDB-selection candidate, not as a production write path.
