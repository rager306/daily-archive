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

## Supersession chain

```
ADR-037 (NebulaGraph + nGQL)
  ├── superseded §3 (graph choice) ──► ADR-040 (Samyama Graph)
  ├── superseded §5 (schema nGQL)  ──► ADR-040 (Cypher)
  └── refined (embedded + HOT)     ──► ADR-041

ADR-038 (NebulaGraph nGQL)  ──amended──► ADR-040 (Samyama Cypher)
ADR-039 (graph lifecycle)   ──amended──► ADR-040 (updated lifecycle)
```

## Template

New ADRs follow [ADR-TEMPLATE.md](ADR-TEMPLATE.md). Status values:
`Proposed` → `Accepted` → `Deprecated`/`Superseded`. Binding ADRs are
enforced by the architecture-guardrail CI workflow.
