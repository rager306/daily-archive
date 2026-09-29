# ADR Index

Architecture Decision Records for daily-archive v2 (Rust). Python-era ADRs
(ADR-001..036) remain valid for the frozen `legacy/` codebase but are not
listed here — see `legacy/doc/adr/`.

| ADR | Title | Status | Date |
|-----|-------|--------|------|
| [037](ADR-037-rust-architecture-ruvector-nebula.md) | Rust Architecture — Daily-Archive v2 (RuVector + Samyama Graph) | Accepted (partially superseded by 040/041) | 2026-07-25 |
| [038](ADR-038-agents-k1-schema-operators-core-then-modes.md) | Agents K1: Schema, Operators, Core-then-Modes | Accepted (amended by 040) | 2026-07-25 |
| [039](ADR-039-grounded-architecture-lifecycle-validation-sources.md) | Grounded Architecture: Lifecycle, Validation, Sources | Accepted (amended by 040) | 2026-07-25 |
| [040](ADR-040-technology-stack-lock-samyama-ruvector-rvf.md) | Technology Stack Lock — Samyama + RuVector + RVF | Accepted (binding) | 2026-07-25 |
| [041](ADR-041-samyama-embedded-cypher-hybrid-agentruntime-sona.md) | Samyama Embedded Cypher + Hybrid AgentRuntime/SONA | Accepted (binding) | 2026-07-25 |
| [042](ADR-042-hycerag-hypergraph-evidence-chain.md) | Query-Local Evidence Activation Over Reified Evidence Bundles | Proposed (revised) | 2026-07-29 |
| [043](ADR-043-research-process-plane-execution-grounded.md) | Research Process Plane — Execution-Grounded Scientific Memory | Proposed | 2026-07-29 |
| [044](ADR-044-schema-lifecycle-versioning-migration-healing.md) | Graph Schema Lifecycle — Versioned Manifest, Migration Framework, Self-Healing | Proposed | 2026-07-29 |
| [045](ADR-045-schema-validator-logic-relations-invariants.md) | Schema Validator — Logic, Relations, and Invariant Checks | Proposed | 2026-07-24 |
| [046](ADR-046-bitemporal-fact-model.md) | Temporal Edge Model — Bi-Temporal Validity on Edges | Proposed (revised) | 2026-07-24 |
| [047](ADR-047-conflict-detection-resolution.md) | Temporal Edge Invalidation (revised — simplifies Conflict approach) | Proposed (revised) | 2026-07-24 |
| [048](ADR-048-decision-intelligence.md) | Decision Intelligence — First-Class Decision Records | Proposed | 2026-07-24 |
| [049](ADR-049-pipeline-dsl-execution-engine.md) | Pipeline DSL and Execution Engine | Proposed | 2026-07-24 |
| [050](ADR-050-universal-graph-subsystem-kg-crate-family.md) | Universal Graph Subsystem — kg-* Crate Family | Accepted | 2026-07-24 |
| [051](ADR-051-testing-strategy-overlapping-verification-contours.md) | Testing Strategy — Overlapping Verification Contours | Accepted | 2026-08-02 |
| [052](ADR-052-structured-logging-tracing-dual-layer.md) | Structured Logging — Tracing Dual-Layer JSON+Stderr | Accepted | 2026-08-05 |
| [053](ADR-053-article-preparation-multi-layer-document-model.md) | Article Preparation 2.0 — Multi-Layer Document Model for Agent Readiness | Proposed | 2026-08-05 |
| [054](ADR-054-mcp-server-knowledge-base-access.md) | MCP Server for Knowledge Base External Access | Proposed | 2026-08-07 |
| [055](ADR-055-article-context-assurance-plane.md) | Article Context Assurance Plane | Accepted (amended M402/D276: data-driven `acap_layer`) | 2026-08-09 |
| [056](ADR-056-universal-article-acquisition-load-plane.md) | Universal Article Acquisition and Load Plane | Accepted | 2026-08-10 |
| [057](ADR-057-non-article-knowledge-unit-ontology.md) | Non-Article Knowledge Unit Ontology (Textbook / Chapter) | Proposed | 2026-08-11 |
| [058](ADR-058-article-revision-expression-ontology.md) | Article Revision Expression Ontology | Accepted | 2026-08-11 |
| [059](ADR-059-prompt-execution-optimizer-fsm.md) | Provider-neutral Prompt Execution and Optimizer Candidate FSM | Accepted | 2026-08-12 |
| [060](ADR-060-scientific-figure-understanding-pipeline.md) | Two-stage Figure Understanding Pipeline (classify → per-type extract) | Partially implemented (M388) | 2026-08-12 |
| [061](ADR-061-readonly-identity-graph-plane-and-agent-fsm.md) | Read-only Identity Graph Plane and Agent FSM (kg-storage bounded RESP server, DA.IDENTITY semantic handshake) | Accepted (M401-itcqhx/S01) | 2026-08-18 |
| [062](ADR-062-figure-quote-policy-window.md) | Figure Quote Policy Window (soft target + absolute schema ceiling + measurement loop) | Accepted (M413-1x7x3o) | 2026-08-20 |
| [063](ADR-063-graphwrite-deferred-by-design.md) | GraphWrite stays deferred-by-design (no honest producer; governor asserts no fake executor) | Accepted (M417-36ebrj/S02, D278) | 2026-08-20 |
| [064](ADR-064-kg-pipeline-honest-execution-engine.md) | kg-pipeline Honest Execution Engine (real retry/failure policies, observable FSM, injected Sleeper) | Accepted (M436-q9cuc1) | 2026-08-21 |
| [065](ADR-065-tei-declared-revision-binding.md) | TEI-declared revision binding for versionless canonical assets (document-declared arXiv version; fail-closed without it) | Accepted (M437-p43udw) | 2026-08-21 |
| [066](ADR-066-pipeline-orchestrator-stays-product-side.md) | PipelineOrchestrator stays product-side — engine migration deferred-by-design (durable vs in-process retry are complementary) | Accepted (D280) | 2026-08-21 |
| [067](ADR-067-normalized-data-layout.md) | Normalized data layout — flattened article catalog, one home per data class, legacy to archive/, Governor enforces | Accepted (M447) | 2026-08-22 |
| [068](ADR-068-schema-manifest-hierarchy.md) | Schema/Manifest hierarchy — every directory carries SCHEMA (what is possible) + MANIFEST (what exists); meta-prompt principle; all repo dirs, not just data/ | Proposed (M451) | 2026-08-23 |
| [069](ADR-069-isc-pattern-application.md) | ISC Pattern Application — exact-quote evidence gate, compilation depth (R*), genre detection (position/vision papers), parse-integrity signals | Accepted (M451) | 2026-08-24 |
| [070](ADR-070-domain-knowledge-profiles.md) | Domain knowledge profiles — per-domain SCHEMA.yaml (entity types, structural types, thresholds), multi-domain routing, genre-aware extraction | Accepted (M451) | 2026-08-24 |
| [071](ADR-071-survey-spine-and-hypergraph-knowledge-ranking.md) | Survey spine extraction (numbered environments M1..M39), table-cells as extraction surface, taxonomy nodes, EVALUATED_ON edges, Artifact type, hyperedge claim modeling, centrality ranking frame (degree/walk/contextual) | Accepted (M451) | 2026-08-24 |
| [072](ADR-072-xy-cut-reading-order.md) | XY-Cut reading-order reconstruction over neutral line boxes — ported from opendataloader XYCutPlusPlusSorter + dedoc block-boundary discipline; fallback grammar for TOC-less PDFs; figure-zone pre-mask | Accepted (M451) | 2026-08-24 |
| [073](ADR-073-upstream-resilience-and-hybrid-search.md) | Upstream resilience (batched DOI 50/call, negative-cache purge, circuit breaker — citracer) + hybrid search patterns (temporal filters, RRF rerankers — graphiti); normative follow-ups: incremental nav index (iwe), content_ref (edgequake), schema snapshot-diff (semantica) | Accepted (M451) | 2026-08-25 |
| [074](ADR-074-wasm-microlora-policy-modules.md) | WASM MicroLoRA as swappable FSM policy modules — hot-swap learned routing/admission/scheduling; SONA three-loop lifecycle; D294 opens training while promotion remains explicit | Accepted (amended D294) | 2026-08-25 |
| [075](ADR-075-scholarly-parse-plane-staged-platform.md) | Scholarly parse plane as staged native Rust platform — da-parse ownership, conservation-first GROBID replacement, ruVector CPU/SMP, active SONA/MicroLoRA/EWC++ and ruVLLM learning | Accepted (M463/M464 platform; amended ADR-076: cascade still Java; §9 dated amendment 2026-09-25: native cascade independently qualified, Java remains oracle/rollback; §10 dated amendment 2026-09-27: qualification census 610=531+79, five-document pilot is dated history) | 2026-08-27 |
| [076](ADR-076-grobid-cascade-three-automata-graph-freeze.md) | GROBID cascade honesty — three automata, waves W0–W6, graph frozen until family receipts exist | Accepted (M465-sqt7w0, D321, R118–R124; §9 dated corrections 2026-09-10: ledger counts, post-M465 shadow families, M502-2w7y8k registry; §23 dated correction 2026-09-25: document reconciliation, corpus digest re-pin, revision-label resolution, independent qualification; §24 dated correction 2026-09-27: census reconciliation, cross-surface qualification, locks unchanged; §25 dated correction 2026-09-28: parity ledger required-row measurement link closed, typed named-gap refusal, locks unchanged; §26 dated correction 2026-09-28: Java-CPU remainder measured against grobid_sidecar_cgroup_cpuacct, H05 closed in the horizon register, locks unchanged; §27 dated correction 2026-09-29: absent-capability fate registry (14 out-of-product-scope / 3 in-horizon project-native), fail-closed DATA judge plus negative matrix, carrier re-pin and regenerated route-recovery artifact, H21 remainder explicitly accepted, locks unchanged) | 2026-09-04 |

## Phase completion status (ADR-050, 2026-08-02)

| Phase | Status | Milestone |
|-------|--------|----------|
| A | ✅ DONE | M285-z09px0/S03: da-domain re-exports kg-ontology types |
| B step 1 | ✅ DONE | M285-z09px0/S05: da-ports re-exports kg-storage traits |
| B step 2 | ✅ DONE | M285-z09px0/S09: SamyamaGraphStore impl in kg-storage behind feature `samyama` |
| C | ✅ DONE | kg-embeddings skeleton (Embedder + FdApi + RuVector stub) |
| D | ✅ DONE | M285-z09px0/S06+S07: YAML ontology export + OntologyRegistry loader |
| E | ✅ DONE | kg-algorithms (temporal resolution) + kg-pipeline (DSL + execution engine) |
| F | ✅ DONE | M288-avqgux: YamlSchemaAdapter + validate_node_properties_or_yaml + schema-list --from-yaml |

## Supersession chain

```
ADR-037 (NebulaGraph + nGQL)
  ├── superseded §3 (graph choice) ──► ADR-040 (Samyama Graph)
  ├── superseded §5 (schema nGQL)  ──► ADR-040 (Cypher)
  └── refined (embedded + HOT)     ──► ADR-041

ADR-038 (NebulaGraph nGQL)  ──amended──► ADR-040 (Samyama Cypher)
ADR-039 (graph lifecycle)   ──amended──► ADR-040 (updated lifecycle)
ADR-042 (evidence bundles)  ──extended──► ADR-043 (research process plane)
ADR-038 ExperimentSetup     ──specialized──► EvidenceBundle subtype + env_lite bridge (043)
ADR-040 (Samyama schemaless) ──complemented──► ADR-044 (schema lifecycle management)
ADR-043 (28 node types)      ──requires──► ADR-044 (versioned schema registry)
ADR-044 (schema lifecycle)   ──enforced──► ADR-045 (schema validator)
ADR-040 (Samyama store)      ──validated──► ADR-045 (mock contract gap: MEM495)
ADR-042 (Claim fact value)   ──temporal──► ADR-046 (bi-temporal fact model)
ADR-043 (process plane)      ──temporal──► ADR-046 (bi-temporal fact model)
ADR-045 (validator)          ──extended──► ADR-046 (temporal-consistency rule)
ADR-040 (Samyama store)      ──conflicts──► ADR-047 (conflict detection)
ADR-042 (Claim competition)  ──conflicts──► ADR-047 (conflict detection)
ADR-044 (healing actions)    ──decisions──► ADR-048 (decision intelligence)
ADR-047 (conflict resolved)  ──decisions──► ADR-048 (decision intelligence)
ADR-043 (use cases)          ──orchestrated──► ADR-049 (pipeline DSL)
ADR-045 (validator)          ──preflight──► ADR-049 (pipeline validator)
ADR-048 (decision recorded)  ──orchestrated──► ADR-049 (failure handler)
ADR-045 (validator)          ──replaced──► ADR-050 (da-ontology crate)
ADR-040 (Samyama schemaless) ──layered──► ADR-050 (ontology as data)
ADR-044 (schema lifecycle)   ──hosted──► ADR-050 (ontology crate)
ADR-046/047/048/049 (data)   ──moved──► ADR-050 (YAML ontology files)
ADR-037 (paper versions)      ──specified──► ADR-058 (revision expressions)
ADR-056 (source revision)     ──projected──► ADR-058 (revision-aware graph)
ADR-060 (figure understanding) ──preceded──► ADR-061 (read-only plane for agent reads)
ADR-061 (read plane)          ──supersedes D145/D146/D148──► vendor RespServer no longer used for the daemon path
ADR-061 (read plane)          ──refines D149/D150──► typed DA_SERVE_GRAPH_* / DA_REMOTE_GRAPH_* / DA_GRAPH_ENDPOINT env contract
ADR-075 (parse platform)      ──amended──► ADR-076 (cascade honesty, three automata, graph freeze)
ADR-074 (policy WASM)         ──amended──► ADR-076 (parse-pdf stamp; sona-policy CLI is demo)
ADR-055 (ACAP)                ──amended──► ADR-076 (ACAP does not own GROBID cascade or parse graph writes)
ADR-072 (XY-Cut)              ──amended──► ADR-076 (XY-Cut is a layout lane, not a product parser CLI)
```

## Template

New ADRs follow [ADR-TEMPLATE.md](ADR-TEMPLATE.md). Status values:
`Proposed` → `Accepted` → `Deprecated`/`Superseded`. Binding ADRs are
enforced by the architecture-guardrail CI workflow.
