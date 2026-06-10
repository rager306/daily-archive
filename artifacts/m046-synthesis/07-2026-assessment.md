# 07 — 2026 Best Practices Assessment and Recommendations

> **Synthesis layer:** 7 of 7 (final)
> **Scope:** M033-M045 (M033 = research, M034 = ADR package, M035-M045 = delivery)
> **Lens:** 2026 best practices in ADR, modular architecture, observability, fail-closed defaults, AI-assisted coding governance
> **Method:** each claim anchored to a synthesis layer (00-06), an ADR, or a GSD decision

## 0. Reading Order

This layer is the **external view**. It assesses the project against 7 categories of 2026 best practice and gives actionable recommendations. Each recommendation is **anchored** to a specific R/ADR/verifier so it can be picked up by a future milestone.

## 1. Overall Verdict

daily-archive's M033-M045 output is **above industry average** for a small research / evidence-pipeline project on the dimensions of:
- ADR as code (Mermaid-assisted, LLM Reading Notes)
- Modular boundaries (C4 + DDD)
- Fail-closed defaults (5× false, explicit safety contract)
- AI-assisted coding governance (GSD + trajectory check + governance memory bridge)

The project has a **clean separation** between canonical (GSD, ADRs) and derived (trajectory report, governance mirror) sources, which is rare. The trajectory check (M045) as a **single planning/closeout preflight** (D080) is a forward-looking pattern that goes beyond 2026 best practice.

**Main gaps:** limited 2026-style automated governance (CI/CD integration of trajectory check, M044 guardrail in pre-commit), no formal fitness functions beyond M045, and the operations procedure is documented but not yet enforced by automation.

## 2. Category Assessment

### 2.1 ADR as Code — STRONG

| 2026 best practice | daily-archive status |
|---|---|
| Use MADR-style or enhanced ADRs with Mermaid | YES — M034 Mermaid-assisted template, Mermaid context maps in ADR-000 |
| LLM Reading Notes in every ADR | YES — per `doc/adr/m034/ADR-TEMPLATE.md` |
| ADRs as executable, not narrative | PARTIAL — most ADRs are exercised by verifiers, but ADR-001 is "planned" and never drafted |
| Decision register (D###) alongside ADRs | YES — D072-D080 in `.gsd/DECISIONS.md` |
| Mermaid diagrams bounded and audited | YES — verifier-enforced per M034 lessons learned |

**Anchors:** `doc/adr/m034/ADR-000`, `doc/adr/m034/ADR-TEMPLATE.md`, `.gsd/DECISIONS.md` (D072-D080)

**Strengths:** the M034 template is a model other projects could copy. The LLM Reading Notes section is increasingly a 2026 standard.

**Gaps:** ADR-001 (Scientific Papers as First Domain) is **Planned** since M034 but never drafted. This is a real gap because the claim "scientific articles as first domain" is implicit in ADR-000 but not isolated in its own decision. **Resolved in M046-3b7gp0 QW-1 (D081):** `doc/adr/ADR-001-scientific-papers-as-first-domain.md` accepted, all 8 ADRs now binding or deferred (zero Planned).

**Recommendation 1:** Draft ADR-001 (Scientific Papers as First Domain) in a future milestone. The risk of not having it is that the implicit claim is not auditable, and the supersedes chain from ADR-000 is weakened. **Priority: medium.** **Anchor: R060, R058.**

### 2.2 Modular Boundaries — STRONG

| 2026 best practice | daily-archive status |
|---|---|
| C4 model (System Context, Containers, Components, Code) | YES — `02-architecture-layers.md` synthesizes 4 levels |
| Bounded contexts (DDD) | YES — 8 bounded contexts in `02-architecture-layers.md` |
| Anti-corruption layer at external boundaries | YES — `universal_kb_sidecar_boundary.py` (Adaptix) |
| Explicit inter-context contracts | YES — bounded context table in `02-architecture-layers.md` |
| Deep modules (Ousterhout) | PARTIAL — most modules are well-shaped but a few (`graph_readiness_*.py`) may be over-decomposed |

**Anchors:** `02-architecture-layers.md`, `04-module-map.md`, ADR-004 (sidecar boundary)

**Strengths:** the C4 + DDD split is unusual in a project of this size. The Anti-corruption layer at the sidecar boundary is well-designed.

**Gaps:** `graph_readiness_*.py` has 6 files (manifest, export, extraction_gate, persistence, retrieval_validation, review). It may be over-decomposed; consolidation is a candidate.

**Recommendation 2:** Audit `graph_readiness_*.py` for over-decomposition. Consolidate `graph_readiness_*.py` if inter-module complexity is low. **Priority: low.** **Anchor: ADR-005, R029.**

### 2.3 Observability — STRONG

| 2026 best practice | daily-archive status |
|---|---|
| Per-job events with structured payloads | YES — M035 SQLite queue with events |
| Aggregate summary per batch | YES — `summary.json` per batch in `artifacts/m036-*/`, `m040-*/`, `m041-*/` |
| Audit JSON + Markdown | YES — `audit.{json,md}` per smoke |
| Drift detection (continuous) | YES — M045 trajectory check (7 dimensions, 3 severity) |
| OpenTelemetry-style traces | NO — not yet; logs and structured JSON are the current level |

**Anchors:** `06-trajectory-ops.md`, M035, M040-M045 summaries

**Strengths:** the audit + summary + trajectory chain is comprehensive. Per-article `continuity.json` (M040) and per-batch `summary.json` are reproducible.

**Gaps:** no OTel integration, no live process metrics. The trajectory check is the strongest 2026 pattern; it could be enriched with empirical health metrics (e.g., average artifact production rate, queue depth).

**Recommendation 3:** Add a thin metrics dimension to the trajectory check: per-milestone artifact count, queue lease distribution, audit pass rate. **Priority: low.** **Anchor: R065, D080.**

### 2.4 Fail-Closed Defaults — STRONG (LEADING)

| 2026 best practice | daily-archive status |
|---|---|
| Default-deny security posture | YES — 5× false safety defaults |
| Prohibited-claim scan | YES — M045 trajectory check (4 regex + 11 counterterms) |
| Reverse ADR audit | YES — manual in S04, suggested in S05 follow-ups |
| No unsafe state in error paths | YES — queue has leases, heartbeats, stale detection |
| Bounded failure surface | YES — `FAILURE-TAXONOMY.md` in `doc/contracts/m034-universal-kb/` |

**Anchors:** `05-evidence-safety.md`, M044 architecture guardrail, M045 prohibited-claim scan

**Strengths:** this is the **leading** dimension. The combination of 5× false defaults + prohibited-claim scan + architecture guardrail + reverse ADR audit is rare in any project, let alone a research one.

**Gaps:** reverse ADR audit is manual. The M044 guardrail is a one-shot check, not continuous. **Resolved in M047-96puxn:** reverse ADR audit is now an 8-rule dimension in M045 trajectory check; M044 guardrail runs on every pre-commit (mandatory) and on every push/PR via GitHub Action.

**Recommendation 4:** Wire the M044 architecture guardrail as a **pre-commit hook** and add reverse ADR audit to the M045 trajectory check as a routine dimension. **Priority: medium.** **Anchor: D079, D080, ADR-005.** **Resolved in M047-96puxn:** `.pre-commit-config.yaml` (M044 mandatory, M045 advisory), `.github/workflows/architecture-guardrail.yml` (CI), `scripts/install-precommit.sh`, and `scripts/check_project_trajectory.py` extended with `reverse_adr_audit` dimension (8 rules anchored to ADR-002/005/007/R029). Tests: `tests/test_m045_project_trajectory.py` covers clear baseline + 2 violation cases.

### 2.5 Trajectory Check vs Per-Topic Guardrails — LEADING (2026 PATTERN)

| 2026 best practice | daily-archive status |
|---|---|
| Single drift detector (vs many per-topic) | YES — D080 explicitly chose this |
| Composable, derived, non-canonical | YES — trajectory report derived from canonical sources |
| 7+ dimensions | YES — architecture, functionality, module_code, evidence, safety, operations, next_gate |
| Drift flags with severity | YES — high / medium / info |
| Counterterm handling (avoid false positives) | YES — 11 counterterms, 4 regex |

**Anchors:** `06-trajectory-ops.md`, `scripts/check_project_trajectory.py`, M045, D080

**Strengths:** D080 explicitly chose **one trajectory check over proliferating guardrails**. This is a 2026-leading pattern. The counterterm handling is unusual and prevents over-sensitivity.

**Gaps:** trajectory check is local, not CI-enforced. The M046 chore-commit showed that even with 326 dirty files, the trajectory check still reports `on_track` — because the dirty-files flag is info-level. This is a tuning opportunity (consider promoting `uncommitted_changes_present` to medium in some contexts).

**Recommendation 5:** Tune trajectory check drift severities per phase. During active milestone execution, promote `uncommitted_changes_present` to medium (so agents commit more often). During closeout, demote to info. **Priority: medium.** **Anchor: D080, R065.** **Resolved in M048-8bhn38:** `scripts/check_project_trajectory.py` now accepts `--phase {preflight,active,closeout}` (default preflight = current behavior). `PHASE_SEVERITY_OVERRIDES` dict: `active` promotes `uncommitted_changes_present` to medium (verdict=drift_risk), `closeout` demotes to info (verdict=on_track). 5 new tests cover all phases. Trajectory report shows phase explicitly.

### 2.6 AI-Assisted Coding Governance — LEADING (2026 PATTERN)

| 2026 best practice | daily-archive status |
|---|---|
| Canonical vs derived source separation | YES — GSD canonical, codebase-memory derived |
| LLM Reading Notes in artifacts | YES — ADRs and milestone summaries |
| Drift detection on AI-generated state | YES — M045 trajectory check |
| Hybrid memory (fast recall + canonical truth) | YES — D075 hybrid model (GSD + GitNexus + codebase-memory) |
| Typed graph projection (vs untyped recall) | YES — D076 artifact-first typed graph |

**Anchors:** `06-trajectory-ops.md`, D075, D076, D080

**Strengths:** the hybrid model is exactly the 2026 emerging pattern: canonical (GSD, GitNexus) + fast mirror (codebase-memory) + derived (trajectory). The LLM Reading Notes convention is a leading practice.

**Gaps:** no formal model registry or prompt versioning. MiniMax helper paths are tested (M014-M017) but not versioned in a registry.

**Recommendation 6:** Add a small `models.yaml` registry at repo root that records the canonical model id per helper path (currently scattered in `D074`, README, and helper source files). **Priority: low.** **Anchor: D074, R045.**

### 2.7 Architecture as Executable — STRONG

| 2026 best practice | daily-archive status |
|---|---|
| Architecture decisions enforced by verifiers | YES — M044 guardrail, M035 contracts |
| Schema/versioning on contracts | YES — `m022-final-gate.v1`, `m036-real-corpus-no-write-smoke.v1`, etc. |
| Stale detection | YES — queue lease/heartbeat, mirror freshness check |
| Architecture as code (ADRs in repo, not wiki) | YES — `doc/adr/m034/` |

**Anchors:** `05-evidence-safety.md`, M044 guardrail, `04-module-map.md`

**Strengths:** this is what makes the project a 2026 example. Architecture is not just documented; it is enforced by verifiers and gate checks.

**Gaps:** the ADRs are local to `doc/adr/m034/`. Future ADRs (e.g., the deferred GraphDB ADR) would also live in this directory, but the convention is not yet explicit for non-M034 ADRs.

**Recommendation 7:** Document a top-level ADR directory convention (e.g., `doc/adr/` with per-milestone subdirs or a flat convention) for future ADRs. **Priority: low.** **Anchor: ADR-INDEX, M034 lessons learned.**

## 3. Anti-Patterns Audit (2026 lens)

| Anti-pattern | Status in daily-archive | Notes |
|---|---|---|
| Magic numbers | LOW RISK | estimates (`est:1h`) are explicit, not magic; no hidden thresholds |
| Swallowed errors | LOW RISK | M045 counterterm list catches "disabled"/"blocked"/"prohibited"; queue has stale detection |
| Implicit dependencies | LOW RISK | M035 queue has explicit `DependencyRecord`; contracts declare fields |
| Leaky abstractions | LOW RISK | Adaptix boundary is enforced by `universal_kb_sidecar_boundary.py` |
| Hotspots (modification hotspots) | MEDIUM RISK | `graph_readiness_*.py` may be a hotspot (see Recommendation 2) |
| Coupling to implementation | LOW RISK | contracts are stdlib dataclasses (D072) |
| Hidden global state | LOW RISK | SQLite queue is explicit, no globals in contracts |
| Implicit authorship of facts | LOW RISK | M035 review assistance has no LLM approval authority (ADR-006) |
| Documentation rot | MEDIUM RISK | 326-file template-rerender (fixed in M046 chore commit) is evidence of rot risk |
| Architecture drift | LOW RISK | M044 guardrail (now pre-commit + CI), M045 trajectory check (8 dimensions, 0 violations) |

## 4. Known Gaps and Risks

| Gap | Risk | Mitigation |
|---|---|---|
| ADR-001 (Scientific Papers as First Domain) never drafted | Implicit claim not auditable | Recommendation 1, **resolved in M046 QW-1 (D081)** |
| M044 guardrail not pre-commit-enforced | Drift possible between commits | Recommendation 4 |
| codebase-memory MCP does not implement runtime edge creation | Typed graph is artifact-only | D076; monitor MCP roadmap |
| DSPy optimizer remains disabled | No benchmark-driven improvement | R020, R021, R041 — gate is intentional |
| GROBID live probe produced 1/6 success | 5 target articles lack local PDFs | next gate text in M045 follow-ups |
| M036 verifier rerun can dirty tracked files | Tracked evidence can be modified by verifier | M036 lesson learned; future design |
| M035 verifier unstable against extended `.gsd/DECISIONS.md` | Verifier needs stable M034 snapshot | M035 lesson learned; future fix |

## 5. Recommendations (Actionable, Prioritized)

| # | Recommendation | Priority | Anchor | Effort |
|---|---|---|---|---|
| 1 | Draft ADR-001 (Scientific Papers as First Domain) | medium | R060, R058 | small |
| 2 | Audit `graph_readiness_*.py` for over-decomposition, consolidate if low coupling | low | ADR-005, R029 | small |
| 3 | Add metrics dimension to M045 trajectory check (artifact count, queue depth, audit pass rate) | low | R065, D080 | medium |
| 4 | Wire M044 architecture guardrail as pre-commit hook + add reverse ADR audit to M045 | medium | D079, D080, ADR-005 | medium |
| 5 | Tune trajectory check drift severities per phase (active vs closeout) | medium | D080, R065 | small |
| 6 | Add `models.yaml` registry for canonical MiniMax helper paths | low | D074, R045 | small |
| 7 | Document top-level ADR directory convention for future ADRs | low | ADR-INDEX, M034 | small |

**Effort estimates:** small = 1-2 hours, medium = 1-2 days.

## 6. Strengths to Preserve

1. **5 safety defaults as single contract** — clear, auditable, non-negotiable.
2. **Mermaid-assisted ADRs with LLM Reading Notes** — model 2026 best practice.
3. **Trajectory check (D080) instead of proliferating guardrails** — leading 2026 pattern.
4. **Hybrid governance memory (D075, D076)** — canonical + fast mirror, no drift.
5. **C4 + DDD split in synthesis package** — uncommon at this project size.
6. **Per-milestone verifier scripts** — architecture as executable.
7. **Prohibited-claim scan with counterterm handling** — pragmatic over-claim prevention.

## 7. Cross-References

- North star: `01-north-star.md`
- Architecture: `02-architecture-layers.md`
- Decisions: `03-adr-decisions.md`
- Modules: `04-module-map.md`
- Safety: `05-evidence-safety.md`
- Trajectory: `06-trajectory-ops.md`

## 8. LLM Reading Notes (binding)

- **This layer is the external view.** It is allowed to be opinionated, but each opinion is anchored to a synthesis layer, ADR, or GSD decision.
- **Recommendations are not binding.** A future milestone may decline to follow them, but should record a GSD decision explaining why.
- **Strengths are as important as gaps.** Preserve what is working before fixing what is not.
- **Priorities are relative, not absolute.** A small-priority recommendation can become high-priority if the project changes direction.
- **2026 best practices evolve.** This assessment is current as of mid-2026; revisit annually.
