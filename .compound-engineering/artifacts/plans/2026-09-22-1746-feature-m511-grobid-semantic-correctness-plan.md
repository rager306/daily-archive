---
title: M511 GROBID Semantic Correctness Completion - Plan
type: feature
date: 2026-09-22
topic: m511-grobid-semantic-correctness
artifact_contract: ce-unified-plan/v1
product_contract_source: ce-brainstorm
execution: code
milestone: M511-j7wz2f
---

# M511 GROBID Semantic Correctness Completion - Plan

## Goal Capsule

- **Objective:** Close all remaining M511-j7wz2f scope — slices S02–S09 (stages H15–H21) plus the formal evidence-scope closure of H14 — so that the native fulltext composition is semantically correct on real TEI, corpus regression is numeric, resource ceilings are measured, and the Java→Rust parity ledger is complete. Production route, graph writes, import, and promotion stay locked (D127/D321 stand).
- **Product authority:** ADR-076 §14–15 (wave authorization, locks, H↔S mapping) + `doc/review/grobid-absorption-followup.json` (H14–H21 stage registries, `claim_scope: planning`) + `doc/review/grobid-absorption-roadmap-2026-09-21.yaml` (validation_commands, measured_state). GSD (`.gsd/`) is the execution-plan source: this document prepares the per-slice plans (`gsd_plan_slice`), it does not replace them.
- **Open blockers:** Owner stop of 2026-09-22 05:22 UTC (execution starts only on explicit permission); S02 canonical GSD plan not written (scout session `/root/.gsd/agent/sessions/--root-daily-archive--/2026-09-22T04-57-06-583Z_01a0c779-8b17-7fae-91e0-6d805024cc2a.jsonl` holds two completed scout runs); S08/GAR-015 human release decision (gate, not completable by agents).

---

## Product Contract

### Summary

Complete the M511 semantic-correctness wave: fix the bibliography ID/target integrity defect (S02/H15, P0), verify semantic field fidelity and lane fallback (S03/H16), visual typed-absence denominators (S04/H17), run independent corpus regression with fixed identity (S05/H18), measure global resource ceilings (S06/H19 = H05), complete the Java→Rust→independent-test parity ledger (S07/H21), execute the frozen external holdout and record a real human release decision (S08/H20 + GAR-015), and reconcile all documents (S09). Java GROBID remains the production oracle throughout; every slice runs inside the ADR-076 locks.

### Problem Frame

The 2026-09-21 audit measured that the native composer returned `Accepted` for documents an independent parser rejected (A-XML) and for documents with dangling bibliography targets (A-REF), while 1,351 green tests passed — milestone closure, registry integrity, and green tests are three different properties, and none of them proves the output TEI. S01 closed A-XML on measured scope; A-REF (P0), A-FIELDS (P1) and the rest of the horizon (H15–H21) remain. The wave exists so the native cascade can eventually replace Java per family without lying — and so the release decision that unlocks anything is made by a person on real numbers.

### Requirements

Correctness — S02 (H15), P0:

- R1. Every emitted reference target must resolve against the emitted `xml:id` set; a dangling body reference forces a typed rejection, never `Accepted`. (Registry H15 runtime row; audit A-REF.)
- R2. An empty supplied-anchors list with dangling body targets yields a typed rejection (negative test H15-N1). Current behavior: `references_xml` (native_fulltext_composition.rs:975–1002) validates only `c.candidate.anchors` against emitted `bibl-{ordinal}` ids, while preserved body keeps source targets (`#b0`, real fixtures use `target="#b42"` / `xml:id="b78"`).
- R3. A listBibl ID-space mismatch between emitted IDs and body targets is detected and typed, not silently tolerated (negative test H15-N2).
- R4. Ordinal and identifier parity of the existing suite is preserved: `cargo test -p da-parse --test native_bibliography_same_n` stays green (registry H15 command row).
- R5. Body anchors used by the resolution check are derived from the preserved body content itself, not solely from caller-supplied anchor lists: when body targets exist and supplied anchors are empty, the composition must still detect them. Preference (non-binding, for planning): extend the existing scan capability over a parallel mechanism — candidate surface in Decomposition Seeds unit 4.
- R6. The resolution guarantee holds on the final emitted document after splicing — no bypass of an intermediate guard may produce `Accepted` with dangling targets. Well-formedness and xml:id uniqueness alone do not satisfy this requirement. (Candidate placement is named in Decomposition Seeds, not mandated here.)

Field fidelity — S03 (H16), P1:

- R7. Semantic field divergence (header/body/gazetteer lanes) on preserved real TEI is visible as a typed absence or typed failure; a lane never silently substitutes Rust semantics for Java. (Registry H16 runtime row.)
- R8. Every lane remains selectable for Java fallback; a closed lane without fallback is a failure (negative test H16-N2).
- R9. The wrapper-attribute residue from S01/T03 (non-namespace attributes such as `xml:lang` on synthesized wrappers) is tracked under H16 and closed here or explicitly deferred with an owner — it is field fidelity, not well-formedness, and must not be folded into H14.

Visual — S04 (H17), P1:

- R10. Figure/table/formula families emit typed absence (not untyped zero); reference and executed denominators stay unmixed (negative tests H17-N1/N2). Command: `cargo test -p da-parse --test native_visual_family_same_n` green.

Corpus — S05 (H18), P1:

- R11. An independent semantic regression on real corpus documents expresses regressions as numbers (precision/recall/F1) with fixed corpus identity and fixed denominators; the oracle does not share code with the implementation (negative tests H18-N1/N2). Planning dependency: H18 depends on H14 and H15.
- R12. Metrics are reproducible against the fixed corpus; property invariants stay green (`native_fulltext_composition_properties`).

Budgets — S06 (H19), P1:

- R13. RSS / Java-CPU / latency-tail ceilings are measured under concurrent batches with a slow ordinal-0 document; bounded reorder and order determinism hold in that run (existing `max_reorder_depth_observed` gauge) and staging/scratch stay bounded without deadlock (H05 scope); a transport timeout is distinguished from a killed Java process by harness observation, never inferred from the timeout (H05 accepted open item, H19-N2). The batch budget contract (`cargo test -p da-parse --test batch_budget_contract`) stays green. Closure = the horizon H05 open item completed with `completion_evidence` (claim_scope: evidence) plus every followup H19 evidence row re-executed `pass`; the followup stage itself stays `pending` (`claim_scope: planning` never closes).

Parity ledger — S07 (H21), P2:

- R14. Every absorbed Java method maps in the ledger to three links — Java source, Rust surface, independent test — with explicit scope (required 15 / supporting 14 / not_compatibility_requirement 5 / deferred 4 per current registry counts); no method lacks a link, supporting never poses as required.

Holdout — S08 (H20), P2:

- R15. The frozen external holdout package is executed with acceptance margins and per-document uncertainty accounting (the 2026-09-12 pilot 5/5 is a pilot, not the frozen run); cost is accounted; pooled quality claims are forbidden.
- R16. The release decision is recorded by a named human reviewer with an explicit decision — `decision_record` filled, `reviewer != null`, `decision != pending` in `data/parse/acceptance_manifest.yaml`. No automated path may read or fabricate that decision (GAR-015). Auto may prepare the package only.

Integration — S09:

- R17. Documents reconcile with measured fact: ADR-076 (dated-correction convention §9–§15), `doc/review/README.md`, the audit, roadmap corrections C01–C08; the roadmap `validation_commands` set (`doc/review/grobid-absorption-roadmap-2026-09-21.yaml:312-331`; five entries as of 2026-09-21) passes at the closing revision (`governor review` exit=0 with only the two expected medium flags; `cargo test -p da-parse --tests` 1374+ passed / 0 failed / 0 ignored; the contract and properties suites green including the tests this wave adds (the pre-wave counts, contract 28 and properties 8, are a floor, not the closing bar); followup schema-ok). S09 never equates to GAR-015 closure.

H14 formal closure (evidence-scope, outside planning):

- R18. H14 closes only through an evidence-scope record kept outside the planning registry, in the vehicle selected under KTD2. Whatever the vehicle, the record requires: `claim_scope: evidence`, `status: complete` only with six-key `completion_evidence` (mode, ref, result: pass, artifacts, observed_at, source_revision) bound to the closing revision, with `tested_source.verified: true` at that same revision, `status: complete`, every cited verification row `pass`, and completion `artifacts` resolving to existing repository paths (exit 0 still includes the two unconditional medium flags; any `grobid_horizon_review_broken` flag is exit 2); all H14 verification rows re-executed at that revision flip to `pass`; `implementation_pass`/`verification_pass` flip together; planning-registry no-fabrication semantics preserved; D127/D321 and `this_wave_promotes=none` untouched. The S01 slice UAT alone does not close H14.

### Key Decisions

- Scope equals M511 only. Candidate-promote, learning W2 (ReasoningBank/MicroLoRA training beyond trajectory recording), ACAP admission, D127 import unlock, graph unfreeze, and production-route changes are out of scope and locked. (Per ADR-076 §14: "Разрешение M511 не является разрешением снять D127".)
- Predecessor decisions stand. The 2026-08-26 da-parse crate plan (M464 platform scope: crate seams, Java-as-oracle, conservation-first, cache split, GROBID-compatible REST) is closed context; its KTDs are inherited, not reopened.
- Numeric or independent oracles only. H18 requires an oracle that shares no code with the SUT; H14 closure requires re-executed evidence at the closing revision, not prose or historical gsd_exec rows alone.
- Two registries for H05. S06 completes the horizon H05 open item with evidence and re-executes every followup H19 evidence row `pass`; the followup stage status stays `pending` (planning scope cannot close — the checker flags any closure status there). A slice is not done while either remains unexecuted.
- Human gates stay human. S08/GAR-015: automation prepares, a named human decides.

### Acceptance Examples

- Covers R1, R2: Given a preserved TEI whose body has `target="#b0"` and supplied anchors list is empty, when `compose_native_fulltext` runs, then the result is a typed rejection (dangling target inventory), not `Accepted` — repro mirrored from `doc/review/grobid-composition-repro.rs:151–159`.
- Covers R3: Given a preserved-TEI body target set that cannot be assigned 1:1 onto the composed references (or an emitted id space that does not contain the body target set), when the post-splice validation runs, then the candidate is rejected with a typed divergence naming the offending targets (body bytes unchanged).
- Covers R13: Given a concurrent batch with a slow ordinal-0 document, when Java exceeds the transport budget, then the report distinguishes `transport_timeout` from `java_process_killed` by the harness-recorded `java_process_status` (never inferred from the timeout) and the measured ceilings (RSS, CPU, latency tail, bounded reorder, staging/scratch) are recorded in the budget contract.

### Scope Boundaries

- No production native ingest, no `parse-pdf` route changes, no new REST endpoints (quarantines rest_batch_asset/visual_heads/body_accounting/fulltext_blocks/callout_matching remain enforced by DATA + `architecture_contract.rs` scanners + governor).
- No edits to closed H01–H13/GAR closures (ADR-076 §14 forbids retroactive rewrites); no percentage-of-migration claims anywhere (C01).
- No model bytes in the repo outside `SYNTHETIC_MODEL_FIXTURE_DIRS` (D358 gate); no `std::fs`/`tracing` in `scalar_crf.rs`; R138=0 (`da-parse` not in da-application prod deps) — standing invariants from `.gsd/continue.md`. D204 (inherited via ADR-076): the `GrobidProfile` configuration fingerprints and the legacy fulltext wire request stay byte-identical; the U8 `FailureCode` change must not alter that dialect.
- Execution discipline: grep only narrow paths (never `target/`), one cargo per target/ at a time, per-crate commands only (rocksdb rebuild trap).

### Sources / Research

- ADR-076 §14–15 (wave map, S01/T01–T06 record, locks): `doc/adr/ADR-076-grobid-cascade-three-automata-graph-freeze.md:337-458`
- Stage registries (H14–H21 scopes, evidence rows, negative tests, matrices): `doc/review/grobid-absorption-followup.json`
- Measured state, corrections, validation commands: `doc/review/grobid-absorption-roadmap-2026-09-21.yaml`
- A-REF/A-XML/A-FIELDS defect anatomy: `doc/review/grobid-absorption-audit-2026-09-21.md:31-52`
- S02 code anatomy (scout-verified, current line numbers): `references_xml` :975–1002, `validate_emitted` :813–820, `compose_inner` :174–324 in `crates/da-parse/src/native_fulltext_composition.rs`; `compose_references` :512–648 and body hook :501–509 in `crates/da-parse/src/native_composition.rs`; `NativeCitationAnchorScanner` :187–256 in `crates/da-parse/src/native_citation.rs`; existing partial guard test `bibliography_anchor_must_resolve_to_composed_reference` :138–169 in `crates/da-parse/tests/native_fulltext_composition_contract.rs`; independent lexical oracle `BibliographyAnchor` :178–182 / `bibr_anchors` :292–319 / `anchor_topology_stays_two_denominators` :610–647 in `crates/da-parse/tests/native_bibliography_same_n.rs`.
- Endpoint control matrix, offline vs live split, holdout pilot separation: `data/parse/grobid_compatibility.yaml`, `data/parse/regression_cases.yaml`, `data/parse/external_holdout_results.yaml`, `data/parse/acceptance_manifest.yaml`, `crates/da-parse/tests/backend_contract.rs`, `endpoint_proxy_contract.rs`, `architecture_contract.rs`.
- Governor CLI map (trajectory/review/architecture/doctor; horizon-json-v1 dispatch; exit semantics 0/1/2): `scripts/governor/__main__.py`, `scripts/governor/adapters/checks/review_registry_dispatch.py`, `scripts/governor/checks/scholarly_parse_plane.yaml`.
- Corpus identity infrastructure: `data/parse/regression_cases.yaml` (three bound preserved TEI, sha256 + conservation_delta), `crates/da-parse/tests/cache_identity.rs` (`FROZEN_CORPUS_HASH`), `crates/da-parse/src/transport.rs` (`FailureCode::Timeout` :969–989).
- Parity triple-link precedent: `data/parse/grobid_compatibility.yaml` `native_surfaces` (:200–214).
- Predecessor plan (closed, inherited): `.compound-engineering/artifacts/plans/2026-08-26-1515-refactor-da-parse-crate-plan.md`.
- Index freshness: CBM full reindex 2026-09-22T06:45:53Z at 675f84f17 (hot files metadata_match); GitNexus up-to-date at 675f84f (flows partial by budget — CBM authoritative for callers). Documented limitation (checker rule: staleness is never silently fresh): the registry `index_freshness` blocks still record the 2026-09-21 capture at `bcdd0cfc5`; U14 refreshes them to the closing revision before any closure claim.
- Planning research: repo-research, learnings (solutions/ absent — none found; institutional sources distilled), and spec-flow gap analyses of 2026-09-22 (findings consolidated into KTDs and the Verification Contract).

---

## Planning Contract

### Technical Approach

Wave execution is slice-serial with parallelizable S03/S04/S07, following the per-slice loop: GitNexus impact check before editing any listed surface (HIGH/CRITICAL risk surfaced to the owner) → RED contract/property test → implementation on the decomposition surfaces → `node .gitnexus/run.cjs analyze --index-only` after structural symbol or test additions → re-execution of the slice's registry evidence rows → `detect_changes` before the slice commit → `governor review --registry doc/review/grobid-absorption-roadmap-2026-09-21.yaml` (exit=0) → GSD closeout through engine tools only. Cross-slice dependencies are registry-authoritative: H18 ← [H14, H15], H20 ← H18; S06 completes H05 and re-executes the followup H19 rows (stage stays `pending`); S09 requires everything else including the H14 record.

### Key Technical Decisions

- **KTD1. H18 oracle independence via an added independent preserved-corpus verifier row.** The existing source-bound H18 `eval_batch` command row is retained unchanged as supporting evidence; a separate command row for an independent preserved-corpus native oracle/same-n verifier (no shared code with the composer) is added through a governed registry amendment with governor re-review. The added row carries the numeric evidence for R11; `eval_batch` remains explicitly executed as supporting. Governs R11–R12. (session-settled: user-approved — chosen over rewriting or demoting the existing registry row: additive amendment preserves registry integrity while satisfying H18-N2 independence.)
- **KTD2. H14 closure vehicle = sibling evidence-scope JSON registry** following the `grobid-rust-horizon-review.json` pattern for H01–H13, not a patch of the planning registry. Governs R18. (session-settled: user-approved — chosen over patching `claim_scope` in place: ADR §15 wording requires the record outside planning-scope.)
- **KTD3. S05 soft-block sequencing.** Offline H18 harness work starts once S02 lands; numeric H18 evidence is committed only after the H14 evidence-scope record exists (registry depends_on [H14, H15]). Same rule as the wave-order sentence: U13 phase 1 precedes U7, and U13 phase 2 binds `source_revision` at the closing revision after U14. Governs R11. (session-settled: user-approved — chosen over strict full block: preserves parallelism without faking evidence order.)
- **KTD4. S08 role split.** M511/S08 prepares the decision package and executes the frozen probe; the GAR-015 record stays owned by `M502-2w7y8k/S09`; the named human fills `reviewer`/`decision` in `data/parse/acceptance_manifest.yaml`. Governs R16. (session-settled: user-approved — chosen over M511 owning GAR-015 closure: the human gate is not transferable.)
- **KTD5. Single bibliography authority.** Anchor resolution is one post-splice authority in the emitted-document validation; no second bibliography identity model. Identity rule: the emitted `listBibl` `xml:id` space is the preserved body's bibr target space, `bibl-{ordinal}` only for documents without preserved body targets, and body targets are never rebound. Scanner/validator placement follows the non-binding Decomposition Seeds (S02 #3–#4). Governs R1–R6.
- **KTD6. GSD task granularity.** One GSD task per negative test and per registry evidence command; per-crate verify commands only.
- **KTD7. Document cadence.** Per-slice ADR dated-correction lines only where measured facts change; one final reconciliation pass in S09.
- **KTD8. Mid-slice failure playbook.** Java unreachable or transport timeout mid-slice: typed `FailureCode::Timeout` plus the harness `java_process_status` observation — `java_process_killed` is reported only when the harness observed the process dead (H19-N2: never inferred from the timeout), slice stays open, no registry flip, no partial evidence; feature-gated targets missing a feature = rc=101 fail-visible (never treated as skip); heavy `da-parse` tests serialized on `target/`. Governs R13, R15 and the Verification Contract.

### Assumptions

- Owner permission precedes any execution step (standing stop of 2026-09-22); the plan is execution-ready but not execution-authorized.
- The H05 live probe (S06) runs idle-gated on the host with artifacts attached; if the host cannot support it, S06 stays open rather than closing on mocks alone.
- The unexplained historical full-gate FAIL (`/tmp/s01_arch2.txt`, outside the repo) is treated as unreproduced: the architecture gate is re-run at S09 close with gsd_exec evidence attached.
- GAR-015 ownership handoff (KTD4) is confirmed with the owner at S08 start; if declined, S08 stops at package preparation.

---

## Implementation Units

Order inside S02 is RED-first. Units are atomic-commit sized; each cites its registry rows and seed checklist (Decomposition Seeds below is the per-unit micro-checklist). Wave order (single authority, identical to KTD3): U1 before U2; U2's H15 command row green before any H18 numeric commit; U6 before U7; U7 only after U1, U2 and U6 have landed and U13 phase 1 exists (H14 record created, rows executed); U13 phase 2 (all H14 rows re-executed and `source_revision` bound) runs at the closing revision after U14, and U9's H05 evidence is re-executed at that same revision; S08 (U11, U12) after S05; U14 after every other unit. The closing revision is the revision after U14's document reconciliation. S03, S04, and S07 run in parallel with that chain.

### U1. Post-splice bibliography resolution gate (S02)

**Goal:** No `Accepted` with dangling local targets, including when supplied anchors are empty.
**Files:** `crates/da-parse/src/native_fulltext_composition.rs`; `crates/da-parse/tests/native_fulltext_composition_contract.rs`; `crates/da-parse/tests/native_fulltext_composition_properties.rs`.
**Approach:** RED tests for H15-N1 (empty anchors + `#b0`) and H15-N2 (id-space mismatch), then a post-splice resolution pass emitting a typed dangling-target finding that falls back to the byte-identical Java envelope (existing typed fallback pattern). Emitted-id rule: on an admitted reference splice the emitted `listBibl` `xml:id` space is the preserved body's bibr target space — body bytes stay unchanged (ADR-076 §14) and body targets are never rebound; documents without preserved body targets keep `bibl-{ordinal}` (the synthetic pin `bibl-0` belongs to that class only); a 1:1 assignment of composed references onto the body targets that fails is a typed Reject naming the offenders. Seeds S02 #1–#3.
**Verify:** contract suite green including new tests; properties green; governor roadmap review exit=0.
**Covers:** R1, R2, R3, R6. Execution note: start with the failing contract test for the empty-anchors case.

### U2. Body anchor derivation wiring (S02)

**Goal:** Anchors derive from the preserved body; the contract harness stops hardcoding empty outcomes.
**Files:** `crates/da-parse/tests/native_fulltext_composition_contract.rs` (harness :71–83); `crates/da-parse/src/native_citation.rs` (scanner :187–256); `crates/da-parse/src/native_composition.rs` (:501–509); preserved-topology fixture (real `#b42` / `xml:id="b78"`).
**Approach:** Wire the scanner outcome into `references()`; add the preserved-topology case as a data-contract/regression case. Seeds S02 #4–#6 (the #6 parity sweep is this unit's Verify).
**Verify:** `native_bibliography_same_n` parity unchanged (R4); contract green.
**Covers:** R4, R5.

### U3. Lane-typed field divergence and fallback matrix (S03)

**Goal:** Field divergence is a typed value; every lane stays Java-selectable; a closed lane without fallback fails typed.
**Files:** `crates/da-parse/src/native_header_fields.rs` (`project_native_header_fields` :80–135, `NativeHeaderFieldsError`); `native_body.rs`; gazetteer lane module; `crates/da-parse/tests/native_header_same_n.rs`.
**Approach:** Independent comparison on preserved TEI (header same-n oracle pattern); lane policy outcomes as closed enums with typed divergence. Negative tests H16-N1 (silent loss), H16-N2 (closed lane without fallback).
**Verify:** `cargo test -p da-parse --test native_header_same_n`; negatives.
**Covers:** R7, R8.

### U4. Wrapper-attribute residue disposition (S03)

**Goal:** The `xml:lang`-class residue is either closed or explicitly deferred with an owner.
**Files:** composition wrapper surface; followup registry note when deferred.
**Approach:** Extend the S01/T03 wrapper-attribute carrying narrowly to semantic fields, or record the deferral as an H16 item — never folded into H14. Negative H16-N3 disposition recorded.
**Verify:** disposition visible in registry + tests.
**Covers:** R9.

### U5. Visual typed absence and denominator guards (S04)

**Goal:** Absence is typed; denominators unmixed.
**Files:** `crates/da-parse/src/native_visual.rs` (`NativeVisualAbsence` :30–41, `lane_absence` :92); `crates/da-parse/tests/native_visual_family_same_n.rs`; `data/parse/native_visual_families.yaml`.
**Approach:** Negatives H17-N1 (untyped zero rejected) and H17-N2 (pooling detected) on preserved TEI; DATA semantics unchanged.
**Verify:** `cargo test -p da-parse --test native_visual_family_same_n`; governor review.
**Covers:** R10.

### U6. Corpus identity freeze and oracle boundary (S05)

**Goal:** Fixed real-corpus denominator; oracle independence satisfied by adding an independent preserved-corpus oracle command row — every existing source-bound H18 command row (including `eval_batch`) retained unchanged as supporting evidence.
**Files:** `data/parse/regression_cases.yaml`; `crates/da-parse/tests/cache_identity.rs` (`FROZEN_CORPUS_HASH`); `crates/da-parse/tests/native_fulltext_composition_oracle.rs`; `doc/review/grobid-absorption-followup.json` (H18 command row amendment).
**Approach:** Pin the preserved corpus as the regression denominator (offline lane separate from the live-Java holdout lane — no pooling); document producer separation (oracle ≠ SUT, GAR-011 lineage pattern); add an independent preserved-corpus oracle/same-n command row to the H18 evidence set via governed registry amendment with governor re-review; `eval_batch` stays in the executed set as supporting. Negative H18-N1 (metric softening under corpus change detected).
**Verify:** corpus identity freeze, governed registry amendment, oracle-independence review, and negative test H18-N1. The reproducible metric run is U7's gate, not this unit's.
**Covers:** R11, R12.

### U7. Numeric corpus regression run (S05)

**Goal:** Numbers committed with fixed denominators from the added independent oracle row; `eval_batch` explicitly executed as supporting.
**Files:** independent oracle suite outputs; `eval_batch` run log (supporting); plan/registry evidence notes.
**Approach:** Run after U6 lands (the added independent oracle row and harness) and after U1 and U2 land (H15's `native_bibliography_same_n` row green); the numeric evidence commit is gated on the U13 phase 1 H14 evidence-scope record (KTD3). Both command rows are executed: the added independent preserved-corpus oracle row (primary for R11) and the retained `eval_batch` row (supporting). Property sweep stays green.
**Verify:** H18 runtime rows (both); `native_fulltext_composition_properties` green.
**Covers:** R11, R12.

### U8. Budget measurement harness (S06)

**Goal:** Ceilings measured (RSS, Java CPU, latency tails, bounded reorder, staging/scratch without deadlock); timeout distinguished from killed Java by observation.
**Files:** `crates/da-parse/src/batch.rs` (`ParseBatchLimits` :89–95, `ParseBatchRunSummary` :1067–1264); `crates/da-adapters/src/parse_cursor.rs`; `crates/da-parse/tests/batch_budget_contract.rs`; `crates/da-parse/src/transport.rs` (`FailureCode` :969–989).
**Approach:** Live-Java concurrent batch with slow ordinal-0. RSS and Java CPU are new instrumentation: no current field on `ParseBatchRunSummary` or the cursor captures them, so the batch execution path grows the observation surface and the summary carries it. Latency tails are not a new probe: per-document latency already exists as `DocumentStats.backend_ms`, set in `fetch_timed` in `crates/da-parse/src/batch.rs`, and the tail is an aggregate of those samples onto `ParseBatchRunSummary`. The typed distinction is an S06 harness observation, not a transport classification: `FailureCode` stays the closed reqwest vocabulary (`Timeout` = client-side elapsed, `Transport` = connect/DNS/IO) and gains no variant — da-parse never observes a Java child exit. The harness that launches or probes GROBID records `java_process_status: alive|exited|unknown` on `ParseBatchRunSummary` (process supervision or a post-failure liveness probe); the report maps client-timeout + `exited` → `java_process_killed` and client-timeout + `alive` → `transport_timeout`, and H19-N2 forbids inferring a kill from the timeout alone. KTD8 playbook applies (Java down → slice open, no registry flip).
**Verify:** `cargo test -p da-parse --test batch_budget_contract`; H19 runtime row.
**Covers:** R13.

### U9. H05 evidence closure with followup H19 rows re-executed (S06)

**Goal:** H05 completed with evidence in the horizon evidence registry; every followup H19 evidence row re-executed `pass`, its stage staying `pending`.
**Files:** `doc/review/grobid-rust-horizon-review.json` (H05 `completion_evidence`); `doc/review/grobid-absorption-followup.json` (H19 evidence rows re-executed only — the stage stays `pending`: `claim_scope: planning` cannot close, KTD2 pattern).
**Approach:** Six-key evidence at the closing revision (after U14 — same rule as U13 phase 2) plus the checker preconditions (`tested_source.verified: true`, every cited row `pass`, completion artifacts resolving to existing paths); at closure extend `owner_allow` in `scripts/governor/checks/grobid_rust_horizon_review.yaml` with `M511-j7wz2f/S06` and set `H05.owner` to it (the contract's own precondition, executed here); no status or pass bit is flipped in the followup registry; governor reviews on both paths green.
**Verify:** both review commands exit=0.
**Covers:** R13.

### U10. Parity ledger audit and gap fill (S07)

**Goal:** Three links per absorbed method; scope honest.
**Files:** `data/parse/grobid_compatibility.yaml` (`native_surfaces` :200–214 pattern); new independent tests where links are missing.
**Approach:** Audit all rows (Java source → Rust surface → independent test); gaps closed by adding an independent test or by an honest registry-backed scope reclassification, never by cosmetic reclassification prose; scope counts re-verified (required/supporting/not_compatibility/deferred); after editing `data/parse/grobid_compatibility.yaml`, recompute and commit its `sha256` in both bindings of `scripts/governor/checks/scholarly_parse_plane.yaml` (`source_data` and `rest_batch_asset_quarantine`) in the same change — a DATA edit without the binding update fails the architecture gate on `evidence_substitution`. A property test pins the H21 three-link invariant: every absorbed row has Java source → Rust surface → independent test, and a supporting row never satisfies a required link.
**Verify:** H21 artifact + runtime rows including the three-link invariant property test; `native_bibliography_same_n` green.
**Covers:** R14.

### U11. Frozen holdout preparation and execution (S08)

**Goal:** The frozen external holdout executed with margins, per-document uncertainty and cost accounting.
**Files:** `data/parse/acceptance_manifest.yaml`; `data/parse/external_holdout_results.yaml`; holdout manifests; `artifacts/parse_compat/holdout_results.json`.
**Approach:** Freeze per the `external_holdout_contract` block of `data/parse/acceptance_manifest.yaml` (`external-holdout-readonly.v1`: identity binding, declared acceptance margins, transfer/cost bounds, result invalidation, citation rule); execute live (distinct from the 5/5 pilot); KTD8 playbook for Java-down; no pooled quality claims; after editing `data/parse/acceptance_manifest.yaml`, recompute and commit its `sha256` binding in `scripts/governor/checks/scholarly_parse_plane.yaml` (`source_data`) in the same change.
**Verify:** H20 runtime + artifact rows.
**Covers:** R15.

### U12. Human release decision package (S08)

**Goal:** Decision package complete; the human's decision belongs to wave terminal state (b) in Definition of Done, not to this unit.
**Files:** `data/parse/acceptance_manifest.yaml`; `doc/review/HUMAN-REVIEW-TEMPLATE.md`.
**Approach:** KTD4 role split — automation prepares (`pending_human_review`), the named human fills `reviewer`/`decision`/`decision_record`; GAR-015 record ownership stays `M502-2w7y8k/S09` (assumption, confirm at S08 start).
**Verify:** automation-prepared package state only — template attached and `pending_human_review` set, with `reviewer`, `decision`, and `decision_record` left unfilled by the agent. Filled human rows are the wave gate in Definition of Done, never this unit's.
**Covers:** R16.

### U13. H14 evidence-scope record

**Goal:** H14 formally closed outside the planning registry.
**Files:** new sibling evidence registry (KTD2); a new sibling governor contract `scripts/governor/checks/grobid_h14_evidence_review.yaml` (stage range H14, `review_schema` → the existing `doc/review/grobid-rust-horizon-review.schema.json` — no second JSON Schema); re-executed H14 rows.
**Approach:** Two phases (KTD3 = the wave-order rule): phase 1 before U7 — create the record and execute all H14 verification rows at the then-current revision; phase 2 at the closing revision (after U14) — re-execute all H14 rows and bind `source_revision` to that revision. Write six-key `completion_evidence`; flip `implementation_pass`/`verification_pass` together; set the checker preconditions at the same time (`tested_source.verified: true`, every cited row `pass`, completion artifacts resolving to existing paths); planning registry untouched. The review dispatch takes exactly one contract and the existing `grobid_rust_horizon_review.yaml` binds one review file with stage range H01–H13, so the sibling gets its own contract in the horizon-json-v1 shape with stage range H14 pointing `review_schema` at the existing `doc/review/grobid-rust-horizon-review.schema.json` (no second JSON Schema), and its gate is an explicit `governor review --registry scripts/governor/checks/grobid_h14_evidence_review.yaml`. The H01–H13 contract's stage range and review path are not modified; the `owner_allow` extension U9 requires is allowed.
**Verify:** `governor review` exit=0 on each bound contract: `scripts/governor/checks/grobid_h14_evidence_review.yaml` (H14 record) and the roadmap registry review; the H01–H13 review path unchanged.
**Covers:** R18.

### U14. Document reconciliation and validation sweep (S09)

**Goal:** Documents match measured fact; all roadmap validation commands green at the closing revision.
**Files:** `doc/adr/ADR-076-grobid-cascade-three-automata-graph-freeze.md` (dated correction), `doc/review/README.md`, `doc/review/grobid-absorption-audit-2026-09-21.md`, roadmap corrections.
**Approach:** KTD7 cadence; the roadmap `validation_commands` sweep; recomputation of any `scripts/governor/checks/scholarly_parse_plane.yaml` sha256 bindings invalidated by this wave's DATA edits; refresh of the registry `index_freshness` blocks to the closing revision (documented limitation in Sources); a final GitNexus reindex (`node .gitnexus/run.cjs analyze --index-only`) and `detect_changes` pass; architecture gate re-run with gsd_exec evidence (assumption on the historical FAIL).
**Verify:** all five validation commands green; schema-ok.
**Covers:** R17.

---

## Decomposition Seeds

Superseded for planning by the Implementation Units above; retained as the per-unit micro-checklists (unit Approaches cite seed numbers). Original granularity preserved unchanged below.

### S02 / H15 (P0)

| # | Size | Unit | Surface | Verify |
|---|------|------|---------|--------|
| 1 | XS | RED: empty supplied anchors + dangling body targets → typed rejection (`empty_anchors_dangling_body_targets_rejected`) | `references_xml` guard, native_fulltext_composition.rs:975–1002 | contract test |
| 2 | XS | RED: emitted bibl id-space ≠ body target set → dangling detector fires | post-splice validation, `validate_emitted` :813–820 | contract test |
| 3 | S | Impl: full ref→bibl resolution as a `validate_emitted` pass with a typed dangling-target finding (covers R1, R6) | same file | contract + properties |
| 4 | XS | Impl: harness `references()` stops passing hardcoded `NativeCitationAnchorOutcome::Empty`; wires `NativeCitationAnchorScanner` outcome (covers R5) | native_fulltext_composition_contract.rs:71–83, native_citation.rs:187–256 | contract test |
| 5 | XS | Fixture: preserved-TEI case with real topology `target="#b42"` / `xml:id="b78"` (not `bibl-{ordinal}`) | tests/fixtures/preserved/, data_contracts.rs | data contract |
| 6 | XS | GREEN sweep: `native_bibliography_same_n` parity unchanged | tests/native_bibliography_same_n.rs | command row |

### S03 / H16

| # | Size | Unit | Surface | Verify |
|---|------|------|---------|--------|
| 1 | XS | RED: silent semantic-field loss visible as typed absence on preserved TEI | native_header_fields.rs / native_body.rs / gazetteer lanes | header same-n |
| 2 | XS | RED: lane closed without Java fallback → typed failure (lane selectability matrix) | lane policy surface | contract |
| 3 | S | Impl: typed divergence reporting per lane (Rust-vs-Java field comparison) | same surfaces | native_header_same_n |
| 4 | XS | Wrapper-attribute residue (`xml:lang` on synthesized wrappers): close here or defer with owner note — H16 scope, never H14 | composition wrappers | contract |

### S04 / H17

| # | Size | Unit | Surface | Verify |
|---|------|------|---------|--------|
| 1 | XS | RED: untyped zero absence rejected (NotObserved distinct from 0) | native_visual.rs heads | visual contract |
| 2 | XS | RED: component/population denominator pooling detected | same | native_visual_family_same_n |
| 3 | XS | DATA check: `data/parse/native_visual_families.yaml` semantics unchanged | DATA | governor review |

### S05 / H18

| # | Size | Unit | Surface | Verify |
|---|------|------|---------|--------|
| 1 | S | Corpus identity freeze: fixed real-corpus denominator (offline preserved TEI; live-Java E2E stays in the holdout/tape lane) | regression_cases.yaml, cache_identity.rs (`FROZEN_CORPUS_HASH`) | identity freeze verified (numbers belong to seed 3 / U7) |
| 2 | XS | Oracle independence proof: no shared code between SUT and oracle | oracle tests | review |
| 3 | S | Metrics committed (precision/recall/F1 with fixed denominators) from the added independent oracle row; `eval_batch` executed as supporting | independent oracle output + eval_batch log | command rows |
| 4 | XS | RED: metric softening under corpus change detected | same | negative H18-N1 |
| 5 | XS | Property sweep green | composition_properties | command row |

### S06 / H19 (= H05)

| # | Size | Unit | Surface | Verify |
|---|------|------|---------|--------|
| 1 | S | Runtime measurement harness: concurrent batch, slow ordinal-0; RSS / Java-CPU / latency tails recorded | batch.rs, parse_cursor.rs | runtime row |
| 2 | S | Typed failure classification: `transport_timeout` ≠ `java_process_killed` via the harness `java_process_status` observation (no new `FailureCode` variant; no inference from timeout — H19-N2) | batch.rs (`ParseBatchRunSummary`) | batch_budget_contract |
| 3 | XS | Horizon H05 closure with `completion_evidence` + followup H19 rows re-executed `pass` (stage stays pending) + `owner_allow`/`H05.owner` extended to `M511-j7wz2f/S06` | horizon registry + followup rows + horizon contract | governor review both |

### S07 / H21

| # | Size | Unit | Surface | Verify |
|---|------|------|---------|--------|
| 1 | S | Ledger audit: all rows mapped Java source → Rust surface → independent test; gap list emitted | grobid_compatibility.yaml | artifact row |
| 2 | S | Gap fill: missing link → add independent test or honest scope reclassification | tests | native_bibliography_same_n |
| 3 | XS | Scope discipline pass: required/supporting/not_compatibility/deferred counts verified | ledger | artifact row |

### S08 / H20

| # | Size | Unit | Surface | Verify |
|---|------|------|---------|--------|
| 1 | S | Frozen probe execution prep: manifests, margins, per-document uncertainty, cost accounting | `data/parse/acceptance_manifest.yaml#external_holdout_contract` | runtime row |
| 2 | S | Execute frozen holdout run (live Java) → results artifact | artifacts/parse_compat/holdout_results.json | artifact row |
| 3 | XS | Decision package prepared (template attached, `pending_human_review` set; the reviewer is named by the human on seed 4) | HUMAN-REVIEW-TEMPLATE.md | package complete |
| 4 | — | Human records decision in acceptance_manifest.yaml — **human-only step, never automated** (GAR-015) | acceptance_manifest.yaml | human row |

### S09 + H14 closure

| # | Size | Unit | Surface | Verify |
|---|------|------|---------|--------|
| 1 | S | H14 evidence-scope record: re-execute H14 rows at closing revision; six-key `completion_evidence`; flips per R18 | evidence-scope vehicle selected under KTD2 | governor review |
| 2 | S | Document reconciliation pass (per KTD7): ADR dated corrections, README, audit, roadmap C-codes | doc/ | review diff |
| 3 | XS | Roadmap `validation_commands` sweep (five entries as of 2026-09-21) at closing revision | repo-wide commands | all green |

---

## Verification Contract

Per-unit gates:

- Each unit's **Verify** line is executable and must pass at the unit's commit; RED tests land before their implementation (S02 order is RED-first).
- ADR-051 contours on what this wave changes. Every `Serialize` type the wave adds or extends (`FailureCode` with its vocabulary unchanged, `NativeHeaderFields` and its error, the visual absence and head outcomes, `ParseDocumentOutcome`, `ParseBatchLimits`, `ParseBatchRunSummary` once it carries the measurements and `java_process_status`) gets a serde roundtrip. The invariants of S03 (typed lane divergence, no silent substitution), S04 (typed absence, unmixed denominators) and S07 (three-link parity over the ledger) each get a property test over arbitrary input, not only a negative case on a fixed fixture. S06 gets no arbitrary-input property test (H19's determinism matrix does not call for one), but its acceptance is not only H19-N2 and `cargo test -p da-parse --test batch_budget_contract`: it also covers the H19 Contour 1 row (bounded reorder and order determinism under slow ordinal-0, asserted on the existing `max_reorder_depth_observed` gauge in `batch_budget_contract`) and the H05 scope clauses on staging/scratch bounds and no deadlock, evidenced by the live concurrent run. S02 and S05 already carry this through `native_fulltext_composition_properties`. Graph-task E2E (ADR-051 §8.1) stays out because graph write is forbidden; Contour 6 (System Journey E2E) remains applicable for the H14 preserved-TEI journey, the H18 corpus run and the H20 frozen holdout, as the registry matrices state.
- Registry command rows cited by a unit are re-executed at that unit's revision — registry PASS is never claimed from prose (`claim_scope: planning`).

Slice gates:

- Slice closeout = all its units landed + all its registry rows re-executed + negative tests demonstrated + `governor review --registry doc/review/grobid-absorption-roadmap-2026-09-21.yaml` exit=0 + GSD closeout through engine tools only (`gsd_task_complete` → `gsd_slice_complete` → UAT → validate).
- S06 additionally completes the horizon H05 open item (U9) with every followup H19 evidence row re-executed `pass` (stage stays `pending`) — both governor paths green.

Wave gates:

- The roadmap `validation_commands` set green at the closing revision (`:312-331`) plus the added independent H18 oracle command row executed (KTD1); H14 evidence-scope record complete (U13 phase 2 bound at the closing revision); H05 completed in the horizon evidence registry with every followup H19 evidence row re-executed `pass`; parity ledger integral; holdout executed with margins; the human release decision recorded — until then the wave is explicitly open at the S08 human step and no wave-completion claim is made.

Failure playbook (KTD8):

- Java unreachable / transport timeout mid-slice: typed `FailureCode::Timeout` outcome with the `java_process_status` observation (`java_process_killed` only on an observed process death — H19-N2), slice stays open, no registry flip, no partial evidence.
- Feature-gated target without its feature: rc=101 = fail-visible; the unit is not "done with skip".
- One cargo on `target/` at a time; heavy `da-parse` test batches serialized in GSD ordering.

---

## Definition of Done

- All units U1–U14 landed; R1–R15 and R17–R18 satisfied with re-executed evidence at the closing revision (R16 only in terminal state (b) below).
- H14 evidence-scope record complete with all H14 verification rows re-executed at the closing revision (R18); H05 completed in the horizon evidence registry with every followup H19 evidence row re-executed `pass` (followup stage stays `pending`); parity ledger integral (three links, honest scope); holdout executed with margins and cost accounting.
- Two terminal states, explicitly distinct: (a) **execution complete, open at the human step** — all agent units U1–U14 landed and every agent-side requirement satisfied with re-executed evidence, R16 unsatisfied (`reviewer`, `decision`, `decision_record` unfilled), the wave reported open at the S08 human step, no completion claim made; (b) **wave closed** — the named human fills `reviewer`/`decision`/`decision_record` in `data/parse/acceptance_manifest.yaml` (`reviewer != null`, `decision != pending`), R16 satisfied, and the decision is recorded in the documents by one KTD7-cadence dated-correction line (not a new unit).
- Roadmap `validation_commands` green with the added independent H18 oracle row executed alongside `eval_batch` (supporting); documents reconciled (ADR dated corrections, README, audit, roadmap corrections).
- Locks and invariants unchanged: `deployed=false`, `import_eligible=false` (D127), `auto_promote=forbidden`, `graph_write=forbidden`, `this_wave_promotes=none`; R138=0; D358 model-bytes gate green; `scalar_crf.rs` purity holds; D204 fingerprints and the legacy fulltext wire request byte-identical.
- Execution remained inside GSD engine tools; no planning-registry PASS from prose; no fabricated human decisions.

---

## Decomposition Seeds — see per-unit checklists

The original fine-grained seed checklists (S02 #1–#6, S03 #1–#4, S04 #1–#3, S05 #1–#5, S06 #1–#3, S07 #1–#3, S08 #1–#4, S09 #1–#3) are retained verbatim in the Decomposition Seeds section above and cited by unit Approaches; the Implementation Units above supersede them as the planning authority. Current per-unit micro-steps are the seed rows referenced in each unit's Approach.
