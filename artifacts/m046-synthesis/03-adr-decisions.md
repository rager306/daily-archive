# 03 — ADR Index and Decision Traceability

> **Source files:** [ADR-INDEX](../doc/adr/m034/ADR-INDEX.md), [DECISIONS.md](../.gsd/DECISIONS.md), milestone summaries M033-M045
> **Synthesis layer:** 3 of 7
> **Scope decisions:** 8 ADRs (M034) + D072-D080 (M035-M045)

## 0. Reading Order

This layer answers three questions:

1. **What is bound?** — the 8 ADRs that constrain every future agent
2. **What was chosen lately?** — 9 GSD decisions (D072-D080) that operationalized M033-M045
3. **What enforces each decision?** — verifier, test, artifact, or governance mirror

## 1. The 8 ADRs (M034, binding)

| ADR | Title | Status | Binding Level | Source | Key Code |
|---|---|---|---|---|---|
| ADR-000 | Universal KB North Star | Accepted | binding | [doc/adr/m034/ADR-000](../doc/adr/m034/ADR-000-universal-kb-north-star.md) | `src/arxiv_archive/universal_kb_contracts.py` |
| ADR-001 | Scientific Papers as First Domain | Accepted (M046 QW-1, D081) | binding | [doc/adr/ADR-001](../doc/adr/ADR-001-scientific-papers-as-first-domain.md) | n/a (paper adapters) |
| ADR-002 | Defer Final GraphDB Selection | Deferred | binding non-lock-in | [doc/adr/m034/ADR-002](../doc/adr/m034/ADR-002-defer-final-graphdb-selection.md) | `universal_kb_substrate_rehearsal.py` |
| ADR-003 | Durable Lazy Async Evidence Pipeline | Accepted | directional | [doc/adr/m034/ADR-003](../doc/adr/m034/ADR-003-durable-lazy-async-evidence-pipeline.md) | `universal_kb_queue.py` |
| ADR-004 | Sidecars as Candidate Evidence Producers | Accepted | binding | [doc/adr/m034/ADR-004](../doc/adr/m034/ADR-004-sidecars-as-candidate-evidence-producers.md) | `universal_kb_sidecar_boundary.py` |
| ADR-005 | No Direct Extractor to GraphDB Path | Accepted | binding | [doc/adr/m034/ADR-005](../doc/adr/m034/ADR-005-no-direct-extractor-to-graphdb-path.md) | `scripts/verify_m044_sidecar_architecture_guardrail.py` |
| ADR-006 | Agent Boundary | Accepted | binding | [doc/adr/m034/ADR-006](../doc/adr/m034/ADR-006-agent-boundary.md) | `universal_kb_review_assistance.py` |
| ADR-007 | Quant-mind Pattern Source, Not Runtime Dependency | Accepted | directional | [doc/adr/m034/ADR-007](../doc/adr/m034/ADR-007-quantmind-pattern-source-not-runtime-dependency.md) | n/a (pattern source only) |

### 1.1 Supersedes / Superseded-by

| ADR | Supersedes | Superseded by |
|---|---|---|
| ADR-000 | narrows R058 (paper-only → universal KB with papers first) | — |
| ADR-001 | narrows ADR-000 (isolates first-domain framing); M046 03-adr-decisions.md row (Planned → Accepted, D081) | future second-domain ADR (after R024 threshold) |
| ADR-002 | narrows D061/D062 (LadybugDB early substrate, not final) | future GraphDB ADR (deferred) |
| ADR-003 | n/a | — |
| ADR-004 | n/a | — |
| ADR-005 | n/a | — |
| ADR-006 | n/a | future agent-runtime ADR (deferred) |
| ADR-007 | n/a | — |

## 2. Recent GSD Decisions (D072–D080, M035–M045)

| ID | Milestone | Scope | Choice | Revisable |
|---|---|---|---|---|
| D072 | M035 S02 T01 | architecture | stdlib frozen dataclasses for core contracts; Pydantic v2 boundary-only | Yes |
| D073 | M035 S03 | architecture | SQLite local durable queue with WAL/lease/heartbeat | Yes — revisit for multi-host/prod |
| D074 | M035 S06 | library | MiniMax-M3-512k on Anthropic-compatible, MiniMax-M3 on OpenAI-compatible | Yes — model id matrix |
| D075 | M038 planning | governance-memory | hybrid: GSD canonical, GitNexus mandatory, codebase-memory fast mirror | Yes — after M038 |
| D076 | M039 planning | governance-memory | typed governance graph projection artifact, defer native MCP graph edges | Yes — when MCP supports typed nodes |
| D077 | M041 planning | real-corpus-expansion | mixed 20-30 article batch, 10 baseline + reference-linked + Hermes | Yes — after M041 |
| D078 | M043 | sidecar-evidence | candidate-only sidecar packets with explicit ready/replay/blocker | Yes — through future parser-quality milestone |
| D079 | M044 | sidecar-architecture-guardrail | M044 guardrail preflight required before future sidecar work | Yes — if sidecar governance changes |
| D080 | M045 | trajectory-governance | unified trajectory check as planning/closeout preflight | Yes — if GSD provides equivalent dashboard |

## 3. Traceability Matrix (decision → R/D → verifier → artifact)

| Decision | Requirements Impacted | Verifier / Checker | Artifact | Notes |
|---|---|---|---|---|
| D072 | R054, R055, R056 | `scripts/verify_m035_universal_kb_prototype.py` | `artifacts/m035-universal-kb-prototype/rehearsal/summary.json` | stdlib dataclass for `SafetyFlags` |
| D073 | R054, R055 | M035 verifier (queue submodule) | `artifacts/m035-universal-kb-prototype/rehearsal/queue_inspect.json` | SQLite local, not distributed |
| D074 | R042, R043, R044, R045 | M014-M017 verifiers, `verify_m017_minimax_safe_helper.py` | `artifacts/m017-minimax-safe-helper/*.json` | endpoint-specific model id |
| D075 | R062 | `scripts/sync_codebase_memory_governance.py` | `.codebase-memory/adr.md` | non-canonical mirror |
| D076 | R063 | M039 verifier, M045 trajectory | `.codebase-memory/governance-graph.json` | typed graph projection |
| D077 | R064 | `scripts/select_m041_mixed_connectivity_batch.py`, M041 verifier | `artifacts/m041-mixed-connectivity-smoke/manifest.json` | mixed batch |
| D078 | R056 | M043 verifier, M044 guardrail | `artifacts/m043-combined-sidecar-probe/sidecar-packets.json` | candidate-only |
| D079 | R056, R059 | `scripts/verify_m044_sidecar_architecture_guardrail.py` | `artifacts/m044-grobid-architecture-guardrail/architecture-context-pack.json` | preflight required |
| D080 | R065 | `scripts/check_project_trajectory.py` | `artifacts/m045-project-trajectory/current/trajectory-report.json` | 7 dimensions |

**Coverage:** 9 of 9 recent decisions have a verifier and an artifact. **100% coverage** for D072-D080.

### 3.1 ADR-to-code traceability

| ADR | Enforced by | Test |
|---|---|---|
| ADR-000 | M035 contracts, M036 audit, M045 trajectory, M044 guardrail | `tests/test_universal_kb_architecture_guards.py` |
| ADR-001 | planned, no code yet | n/a |
| ADR-002 | M035 substrate rehearsal, no real GraphDB | `tests/test_universal_kb_substrate_rehearsal.py` |
| ADR-003 | M035 queue, M045 trajectory (evidence dimension) | `tests/test_universal_kb_rehearsal.py` |
| ADR-004 | M035 sidecar boundary, M043 sidecar packets | `tests/test_universal_kb_sidecar_boundary.py` |
| ADR-005 | M035 ADR-005 guards, M044 guardrail | `tests/test_m044_sidecar_architecture_guardrail.py` |
| ADR-006 | M035 review assistance (no LLM authority) | `tests/test_universal_kb_review_assistance.py` |
| ADR-007 | M033 quant-mind static study only, no runtime code | `tests/test_m033_quantmind_pattern_study.py` |

## 4. Reverse ADR Audit (preliminary)

Performed during S04 (module map). Code-level checks for ADR violations:

| ADR | Expected invariant | Code finding |
|---|---|---|
| ADR-004 | sidecar outputs as candidate evidence, no authority | `universal_kb_sidecar_boundary.py` enforces anti-corruption; Adaptix cannot widen domain authority — PASS |
| ADR-005 | no parser/LLM direct to GraphDB | No direct import of `ladybugdb` / `falkordb` / `helixdb` in src/arxiv_archive/ — PASS |
| ADR-006 | agents may assist, do not orchestrate | `universal_kb_review_assistance.py` produces diagnostic-only output, no LLM approval authority — PASS |
| ADR-007 | quant-mind as pattern source, not runtime | No `quantmind`/`quant_mind`/`llmquant` import in src/ or scripts/ — PASS |

**No reverse ADR violations found** at code level. Verifier-based audit is the durable check. Post-M046 QW-1 update: ADR-001 status now Accepted, so all 8 ADRs are binding or deferred (zero Planned).

## 5. Supersedes / Superseded-By Chains (D-level)

| Decision | Supersedes | Superseded by |
|---|---|---|
| D072 | implicit (pre-D072 used pydantic-light) | future schema-bounded ADR (deferred) |
| D073 | implicit (Redis/Celery rejected) | future distributed queue ADR (deferred) |
| D074 | M013-M016 model defaults | future MiniMax API surface change |
| D075 | implicit (codebase-memory = canonical) | future governance memory ADR |
| D076 | implicit (MCP graph edges) | when codebase-memory supports typed nodes |
| D077 | M006 single-fresh-article strategy | future connectivity milestone |
| D078 | M033 M033 reuse matrix | future parser-quality milestone |
| D079 | implicit (drift possible) | future sidecar governance change |
| D080 | implicit (no trajectory check) | future GSD first-class dashboard |
| D081 | M046 03-adr-decisions.md ADR-001 row (Planned → Accepted) | future second-domain ADR |

## 6. Cross-References

- Architecture: `02-architecture-layers.md` (ADRs → bounded contexts)
- Module map: `04-module-map.md` (ADRs → code, decisions → verifiers)
- Safety: `05-evidence-safety.md` (ADR-005 fail-closed invariants, M044 guardrail)
- Trajectory: `06-trajectory-ops.md` (D080 trajectory check, D075/D076 governance mirror)
- Assessment: `07-2026-assessment.md` (2026 best-practices view on ADR framework)

## 7. LLM Reading Notes (binding)

- **Treat ADRs as binding**, supersedes GSD decisions where they conflict, supersedes this synthesis package.
- **Read ADR-000 first** to ground north star. Then ADR-005 to understand the safety contract.
- **Decisions (D###) operationalize ADRs** — they are not in conflict, they are the implementation.
- **Future ADRs should follow** the M034 Mermaid-assisted enhanced template at `doc/adr/m034/ADR-TEMPLATE.md`.
- **Do not** infer binding level from this synthesis; always read the ADR itself.
- **Supersedes chains are auditable** — check before claiming "this was already decided".
