# Synthesis

<!-- Written by the decider role. The planner treats Decisions as settled. -->

Milestone: M001 "orientation" (single-milestone flow, no CHARTER.md). Inputs: INTENT.md, evidence-codebase.md, evidence-pitfalls.md, evidence-retirement-gates.md. Domain/stack/similar dimensions skipped per RESEARCH.md — no findings invented for them.

## Settled

<!-- Choices closed by an intent constraint or the existing codebase. -->
- Evidence budget — static reading + codebase-memory-mcp toolset + recorded logs only (/tmp/arch-20261123-pass2.log, /tmp/preflight-20261122.log); no new cargo/test/eval runs (INTENT Constraints "Method" + Scope-out "Re-derivation of extraction eval metrics").
- No promotion moves in M001 — import/graph-write/deploy/auto-promote/production_route flips vetoed verbatim (INTENT Scope-out, D127 + ADR-076); grobid-state report may only enumerate gates (SC3d), never flip one (Prohibition N2).
- Report set and surfaces — exactly three doc/review/ reports (architecture-map, functionality-ledger, grobid-state), .md-only commit on gsd-path/M001 (INTENT SC1–SC4, Surfaces).
- Functionality-ledger enumeration method — closed over the dispatch match in crates/da-cli/src/main.rs plus da-application use-cases, verdict vocabulary exactly {работает, кандидат, заглушка, unverified}, proof-weight tag per entry (INTENT SC2, E3/E4, Scope-in).
- «работает» proof standard — only with ran-in-gate or recorded-run evidence, otherwise downgrade (INTENT Prohibition N5).
- Doc-only vs documentary claims — implementation claims cite code paths at HEAD 0f06b764c; Proposed ADRs 042–049 and removed crates (da-graph, legacy/) may appear only explicitly marked proposed/removed (INTENT Prohibition N4; evidence-codebase.md § "designed-but-unbuilt and stale areas exist and are marked unevenly", § "AGENTS.md and ADR-INDEX reference a legacy/ tree that no longer exists").
- Fix-doc wave scope — DOCS-AUDIT rows 1–9, 11–14, 16–22 mechanical corrections plus row 10 (.project/ committed wording); rows 15/23/24 accept-drift, no action (INTENT Constraints "Docs-audit rulings bundle").
- Method tools for reports — codebase-memory-mcp full toolset (owner instruction 2026-11-22); graph at 675,624 nodes refreshed (INTENT Summary, Constraints).
- GROBID identity — GROBID 0.9.1 current, 0.9.0-crf rollback target; D572–D575, D576 carry (INTENT Constraints).
- R123 ladder family set for SC3b — exactly the R123 families; anything beyond gets its own candidate row; absent family gets a typed-absent row (INTENT E5/E6).

## Decisions

### SC3d gate inventory source of truth

- **Decision**: Enumerate the SC3d exhaustive gate list from the machine-checkable ledger in `artifacts/parse_compat/grobid_retirement_acceptance_package.json` (`mandatory_gates` + `blocking_gates`, incl. `threshold.extraction_weakest_family`, `body_axis.body_not_assessable`, `named_gap_learning_ruvllm_ablation`, `h20_frozen_external_holdout_decision`, `h21_revision_label_residual`, `learning.s07_measured_no_gain`), cross-checked against the fail-closed readiness receipt `artifacts/parse_compat/grobid_retirement_readiness.json` (four refusal codes, `claim_scope: blocked_pending_human_authorization`).
- **Runner-up**: Reconstructing the gate list from ADR-INDEX / core ledgers prose — lost because evidence-codebase.md § "designed-but-unbuilt and stale areas" shows doc statuses are mixed/stale (ADR-042–049 Proposed, da-graph mentions stale), while the acceptance package is fail-closed-pinned by a test (`crates/da-parse/tests/grobid_retirement_package_contract.rs:156`).
- **Evidence**: evidence-retirement-gates.md § "the SC3d gate vocabulary maps onto a concrete, already-coded gate set in the acceptance package"; § "the blocked-readiness receipt machinery is complete, executed, and source-bound".
- **Confidence**: high

### RQ1 — retirement-gate infra-vs-execution classification (roadmap input, not M001 build scope)

- **Decision**: Record in grobid-state §(d) per-gate feasibility notes as: all gates except two are **execution of existing packages** (conservation receipts, same-n suites, dual-metric/val_gap machinery, R135/R130 maturity classifier, authorization contract, governor DATA checks — all already coded); `body_axis.body_not_assessable` needs **new measurement infrastructure** (an independent body-localization oracle resolving `p1_localization_uncertain`; none exists in tree); `named_gap_learning_ruvllm_ablation` needs either a **Java-donor ablation build** (new infrastructure, contradicts retirement direction) or an **owner scope decision**. Both flagged routes are roadmap-input decisions for after M001 — INTENT vetoes any promotion-scoping work in M001.
- **Runner-up**: Treat the ablation question as an M001 task — lost, INTENT Scope-out vetoes promotion research/build in this milestone; M001 is assessment + fix-doc only.
- **Evidence**: evidence-retirement-gates.md § Assigned questions — answers; § "dual task metrics and val_gap measurement machinery already exist"; § "the blocked-readiness receipt machinery is complete"; § "explicit promotion authorization is a human-record gate"; evidence-retirement-gates.md Finding "governor-side retirement checks are DATA-rule based and already exercised".
- **Confidence**: high

### RQ2 — whether the 146 non-CI da-parse suites join CI before any promotion milestone

- **Decision**: [owner ruling 2026-11-22, see ## User rulings] CI stays untouched — «мы не трогаем CI — пока не выйдем на устойчивый функционал — все прогоняем локально». The hand-run governor gate is the execution plane of record for all 146 da-parse suites; no workflow edit in any milestone until the owner declares stable functionality, at which point this ruling is revisited. Reports must state explicitly that GitHub CI does not execute the suites and the local gate is the only recorded execution plane (feeds the proof-weight honesty rule N5 and the residual-risk posture below). Confidence note: facts are high; the timing itself was settled by the owner ruling, not by evidence.
- **Runner-up**: Join CI before the first promotion milestone (one-line `--lib` → `--tests`, wall-time-only cost) — lost by owner ruling; the evidence-preferred posture is recorded here for the future revisit trigger.
- **Evidence**: evidence-pitfalls.md § "GitHub CI runs only `cargo test -p da-parse --lib`"; § "the in-repo architecture gate DOES run the da-parse integration suites since M514"; § "the 'compiled and never ran' failure mode is a trap this codebase has already fallen into once"; § "cost signal — the da-parse suites are service-free and feature-gate-free"; evidence-codebase.md § "the largest integration-test surface (da-parse, 146 files) is not run in CI".
- **Confidence**: high

### M001 residual-risk posture for static/graph verdicts

- **Decision**: Keep INTENT's accepted residual-risk posture: every report verdict carries a proof-weight tag (ran-in-gate / static-only / unverified); reports state explicitly where CI does not execute a surface (146 suites) and cite recorded evidence as-of its recorded date. No attempt in M001 to close the CI gap as a build task (see RQ2 NEEDS-USER).
- **Runner-up**: Make CI-closure a wave task now — lost, the owner reserved the steering decision and INTENT books the gap as accepted residual risk for this milestone.
- **Evidence**: INTENT Risks (read, cited in evidence-pitfalls.md § "the INTENT already books the non-CI gap as an accepted residual risk"); evidence-retirement-gates.md § "recorded run evidence covers the gate example surface but the parse suites themselves are not CI-recorded".
- **Confidence**: high

### Preflight measurability signal in grobid-state §(d)

- **Decision**: Report the recorded preflight axes (`measurability 5/21 (23.8%)`, `feature_gated_coverage 19/33 (57.6%)`) in the gate feasibility notes as a dated snapshot of gate measured-state, explicitly marked stale (log predates M001 doc edits).
- **Runner-up**: Omit preflight axes from SC3d — lost, they are the only recorded measurement of how many mandatory gates carry a measured mechanism, which is exactly what SC3d feasibility notes must convey.
- **Evidence**: evidence-retirement-gates.md § "recorded run evidence covers…"; evidence-pitfalls.md § "a measured preflight signal already exposes part of the gated-test exposure"; INTENT Risks ("recorded run evidence ages").
- **Confidence**: medium — verify the scorecard numbers against `/tmp/preflight-20261122.log` verbatim in wave 1 before they enter the report.

## For the planner

- **Wave-1 blockers**:
  1. Verify every cited path in the first drafted report exists at HEAD 0f06b764c (held-out check E1) — establishes the citation pipeline before the other two reports are drafted.
  2. Confirm the preflight scorecard numbers verbatim from `/tmp/preflight-20261122.log` (medium-confidence decision above) before writing grobid-state §(d).
  3. Confirm the da-parse tests feature-gate absence claim (grep-level, medium confidence in evidence-pitfalls.md § "feature gates inside da-parse test files…") before any report claims the 146 suites run as one ungated unit.
- **Walking skeleton**: One report end-to-end first — grobid-state.md (riskiest: gate inventory from acceptance-package JSON, family table, feasibility notes) drafted with full path citations, held-out checks E5/E6 applied, committed .md-only on gsd-path/M001 — proving SC3 + E1 + N2 machinery; then architecture-map and functionality-ledger in the same wave; fix-doc wave last (mechanical, low risk).
- **Pitfalls → tasks**:
  - "Compiled and never ran" CI/gate divergence → grobid-state and ledger must never claim a suite is CI-protected; state gate-vs-CI coverage explicitly wherever suite execution is cited (task owning SC3/SC2 verdicts).
  - Stale doc claims (legacy/, da-graph, "~86 examples", Proposed ADRs) → architecture-map marks them documentary/removed only (N4); the mechanical corrections land in the fix-doc wave (DOCS-AUDIT rows 1–9, 11–14, 16–22).
  - ".project/ is committed" wording vs pipeline reality → fix-doc row 10 task.
  - Stale CHANGELOG gsd-pi hook text → covered fix-doc row (mechanical correction, .md only).
  - Proof-weight honesty → every «работает» verdict must trace to the recorded gate log or another recorded run; a ledger task that cannot produce that evidence must downgrade the verdict (N5).
  - Service-dependent suites live in da-adapters/da-cli, not da-parse → reports drawing the "live-test boundary" must place it at the gate script's provisioning list, not at crates/da-parse (prevents a wrong boundary claim in the architecture map).

## User rulings

- U1 2026-11-22 (RQ2): "мы не трогаем CI — пока не выйдем на устойчивый функционал — все прогоняем локально" — CI stays untouched; the local governor gate is the execution plane of record; revisit at owner-declared stable functionality. RQ2 Decision revised to reflect this; NEEDS-USER tag removed.
- U2 2026-11-22 (.codebase-memory/adr.md mirror): keep accept-drift — the MCP-managed graph stays the live mirror; generator restoration is not planned (may re-surface as roadmap input on material divergence). NEEDS-USER tag removed.

## Still unknown

- RQ1 fork on `named_gap_learning_ruvllm_ablation` (Java-donor ablation build vs owner scope decision) — roadmap-input decision, recorded in grobid-state feasibility notes; not resolved here (D471 remains fail-closed typed_refusal).
- Feasibility notes for SC3d are judgments, not measurements (no evals run) — accepted risk per INTENT Risks; reports must say so explicitly.
