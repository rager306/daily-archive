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
| [046](ADR-046-bitemporal-fact-model.md) | BiTemporal Fact Model — Valid Time + Transaction Time | Proposed | 2026-07-24 |
| [047](ADR-047-conflict-detection-resolution.md) | Conflict Detection and Resolution | Proposed | 2026-07-24 |
| [048](ADR-048-decision-intelligence.md) | Decision Intelligence — First-Class Decision Records | Proposed | 2026-07-24 |
| [049](ADR-049-pipeline-dsl-execution-engine.md) | Pipeline DSL and Execution Engine | Proposed | 2026-07-24 |
| [050](ADR-050-universal-graph-subsystem-kg-crate-family.md) | Universal Graph Subsystem — kg-* Crate Family | Proposed | 2026-07-24 |

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
```

## Template

New ADRs follow [ADR-TEMPLATE.md](ADR-TEMPLATE.md). Status values:
`Proposed` → `Accepted` → `Deprecated`/`Superseded`. Binding ADRs are
enforced by the architecture-guardrail CI workflow.
