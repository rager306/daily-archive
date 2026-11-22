# Review — wave 1, cycle 1

Wave verdict: pass
Cycle: 1
Depth: full
Tasks reviewed: 3

<!-- Inspection mode: reconstructed product in the supplied verify sidecar
     (branch gsd-path-verify/wave-1-cycle-1, HEAD 5a32c9ddc). Identity proven
     before judging: the sidecar tree carries the exact landing blobs for all
     three product files — doc/review/grobid-state.md = ff4e82c9 (landing
     89227c76c), doc/review/architecture-map.md = bf6c36d8 (landing
     3bd9ba34c), doc/review/functionality-ledger.md = 45b1afc5 (landing
     9b8baa97f). T001's landing commit 89227c76c has parent 5da21f4765 = the
     recorded base; its commit^..commit patch touches only the task file
     (allowed orchestrator-owned fields status/agent/base + Log append). The
     product report and the .gitignore /build/ anchor + INTENT C3 line landed
     via the sanctioned workspace-state commit 2266246fc (ancestor of the
     base, recorded in T001's Log re-base entry); its grobid-state.md blob is
     byte-identical (ff4e82c9) at base 5da21f476, at the landing, and at
     5a32c9ddc. Pipeline-owned state (.gitignore /build/ anchor, INTENT C3,
     .project/** task files) in the lineage is sanctioned by owner correction
     C3. Verify evidence: the recorded orchestrator Verify outputs in the
     task Logs (sidecars gsd-path-verify/task-t001-verify, task-t002-verify,
     task-t003-verify — pass, exit 0 each); none re-run here. Spot checks
     re-read artifacts in the reviewed product only. -->

## T001 — Draft and commit doc/review/grobid-state.md (walking skeleton: gate inventory from acceptance-package JSON): pass

- ✅ `doc/review/grobid-state.md` exists, contains sections (a)–(d), and every path citation in it resolves at HEAD (`git cat-file -e`); the Verify command below exits 0 — recorded orchestrator Verify (gsd-path-verify/task-t001-verify): pass, exit 0; file present at ff4e82c9 with sections (a) §(a) line 15, (b) line 33, (c) line 71, (d) line 88; dead-citation enumeration covered by the recorded Verify
- ✅ §(b) covers exactly the R123 families; any beyond-set finding has its own candidate row (E5) and any no-code family has a typed-absent row (E6) — no family silently missing — §(b) table rows: lattice/segmentation, header, bibliography, body accounting, visual (grobid-state.md:47-56); typed-absent statement "none — every R123 family has code and suites in tree" (line 63); beyond-set table with own кандидат rows: callout matching, fulltext composition, java-lift oracle harness (lines 65-70)
- ✅ §(b) statuses use only shadow/candidate/promoted; §(d) lists every blocking gate from the acceptance-package JSON — cross-checking §(d) against `artifacts/parse_compat/grobid_retirement_acceptance_package.json` `blocking_gates` finds all four blocking ids present — no "promoted" anywhere in §(b) (line 45: "no family row below claims promoted"); artifact re-read: blocking_gates = body_axis.body_not_assessable, named_gap_learning_ruvllm_ablation, h20_frozen_external_holdout_decision, learning.s07_measured_no_gain — all four in §(d) as BLOCKING items 1-4 (lines 94-108); §(d) also lists the 21 mandatory gates incl. threshold.extraction_weakest_family (line 117)
- ✅ The preflight numbers in §(d) are byte-equal to the scorecard lines in `/tmp/preflight-20261122.log` and explicitly marked as a dated, stale snapshot; feasibility notes state they are judgments, not measurements — §(d) "Preflight axes (dated, stale snapshot)" quotes verbatim `axis measurability: 5/21 (23.8%)…` and `axis feature_gated_coverage: 19/33 (57.6%)…` (lines 129-141), marked "predates the M001 doc edits"; "judgments, not measurements" at lines 92 and 142; verbatim confirmation recorded done in the task Log (coder entry, 2026-11-22)
- ✅ No sentence declares a native family production-ready without a gate-receipt citation (N2), and no gate is described as flippable within M001 — every family stays кандидат (line 45: "no gate receipt for a promotion exists in the recorded evidence"); §(a) carries the lock block `production_route=forbidden` … `thresholds=not_accepted` (lines 29-32); §(c) "Nothing in this milestone flips any of these [N1/N2]" (line 87)
- ✅ The wave-1 blockers are recorded done in the Log: preflight numbers confirmed verbatim; da-parse feature-gate absence re-checked (grep `cfg(feature` over `crates/da-parse/tests/*.rs`); E1 enumeration run clean on this report — T001 Log coder entry 2026-11-22 records all three; §(b) restates the grep result citing the only match, the deliberate-ungatedness comment at `crates/da-parse/tests/native_retirement_authorization_contract.rs` line 13 (grobid-state.md:40-43)

Warnings (non-blocking):
- §(b) feature-gate sentence garbled by an edit leftover: "no `#![cfg(feature)]` gate exists on any every file under `crates/da-parse/tests` (glob *.rs) file" (grobid-state.md:39-40) — claim content correct per the Log grep; prose only.
- AC3's "statuses use only shadow/candidate/promoted" is satisfied in the report's Russian verdict vocabulary (кандидат = candidate; prose at lines 26-27 and 79-80 uses shadow/candidate); needs tightening if a later cycle wants literal English status words.

Contract violations (blocking):
- none

## T002 — Draft and commit doc/review/architecture-map.md (every claim cites a code path at HEAD): pass

- ✅ Every claim line in doc/review/architecture-map.md carries a path citation into crates/ or Cargo.toml (or another repo path), and every cited path resolves at HEAD — the Verify command below exits 0 (E1, Verify-side per Correction C2) — recorded orchestrator Verify (gsd-path-verify/task-t002-verify): pass, exit 0; every section of the map (architecture-map.md:3-55) carries backticked path citations; T002 Log records three dead-citation drafts caught and fixed during drafting
- ✅ Every absence claim cites the checked location (E2) — §5 closing block "Absence claims in this map, each citing the checked location (E2)": da-graph absent per `Cargo.toml` members list + `git ls-tree 3031bf3ec crates/`; legacy/ absent per `git ls-tree 3031bf3ec --name-only` at repo root (architecture-map.md:44-46)
- ✅ Occurrences of `da-graph` and `legacy/` appear only in sentences explicitly marking removed/proposed/documentary status (N4) — da-graph marked "[removed, not on the runtime path]" (§1); legacy/ marked "[removed; more no longer exists in the tree]" (§1, §5); ADR-042–049 marked "proposed-documentary status, not implemented fact" (§1) and "Proposed … documentary state … no manifest under `crates/` implements them at HEAD [proposed]" (§7)
- ✅ The service-dependent-suite boundary sentence names the gate script provisioning list, and every suite-execution mention states the CI `--lib`-only gap (U1) — §5 places the boundary at `scripts/governor/__main__.py` provisioning list lines 784-800 with the seven da-cli example names (architecture-map.md:48-50); execution-plane bullets state CI runs `cargo test -p da-parse --lib` (line 101 cited) and "Nothing in the 146-suite da-parse integration plane is CI-protected"; §7 repeats the CI `--lib`-only gap (U1)

Warnings (non-blocking):
- Cosmetic phrase "[removed; more no longer exists in the tree]" (twice) reads as an editing leftover for "no longer exists"; the N4 marker still matches (removed).

Contract violations (blocking):
- none

## T003 — Draft and commit doc/review/functionality-ledger.md (closed enumeration, verdict vocabulary + proof-weight tags): pass

- ✅ The ledger enumerates every dispatch arm from `crates/da-cli/src/main.rs` (help-only/hidden included) plus the da-application use-cases — no dispatch entry omitted (E4); a reviewer diffing the ledger's command list against the match arms finds no gap — reviewer re-enumeration at the landing tree: 38 top-level `Commands::*` arms (main.rs match, lines 534-981), all 38 kebab-cased names present in §1a/§1b of the ledger; the 23 source-action match strings in the dispatch region (main.rs:557-645) all appear in §1b's 24-action table; all 103 `pub mod` declarations in `crates/da-application/src/lib.rs` cited as `<module>.rs` rows in §2 (103/103, none missing)
- ✅ Every verdict is exactly one of {работает, кандидат, заглушка, unverified}, one verdict per command — the Verify command below exits 0 (E3, Verify-side per Correction C2) — recorded orchestrator Verify (gsd-path-verify/task-t003-verify): pass, exit 0; reviewer cell scan over the ledger tables: кандидат 165, unverified 3, работает 2, zero out-of-vocabulary verdict cells
- ✅ Every «работает» row cites a recorded-run/gate-log evidence path (N5); every row carries a proof-weight tag and a path citation that resolves at HEAD — both «работает» rows cite `/tmp/arch-20261123-pass2.log` ran-in-gate (§1a parse-batch row: governor gate `da-cli tests... OK` + `da-parse tests... OK`; §3 native/route example-backed flows: named example runners from the gate log); §2 default-downgrade rationale explicitly refuses «работает» from crate-level test execution alone (ledger §2 preamble); count summary §4 restates the N5 rule
- ✅ Suite-execution mentions state the CI `--lib`-only gap and the local gate as the execution plane of record (U1) — ledger preamble "Execution plane of record (U1)" cites `.github/workflows/architecture-guardrail.yml` line 101, `cargo test -p da-parse --lib`, 146 suites not CI-protected, local governor gate 23/23 (`/tmp/arch-20261123-pass2.log`); §3 closing note repeats it

Warnings (non-blocking):
- §2 row ordering places rows 101–103 (acap_contract_audit, acquisition_batch, batch_ingest) after row 100 instead of alphabetical position; completeness unaffected (103/103 verified).

Contract violations (blocking):
- none

## Intent coverage

### SC1 — doc/review/architecture-map.md: every architecture/dependency claim cites a concrete path in crates/ or Cargo.toml; no claim line without a path citation (checked over the whole file).: pass
- ✅ Recorded orchestrator Verify (gsd-path-verify/task-t002-verify) pass, exit 0: cited-path git-enumeration clean at HEAD and claimless-line scan clean; whole-file read confirms every claim line carries a backticked repo-path citation, including absence claims with checked locations (E2) and crate manifests per row of the §1 table. Held-out edge E1 (criterion SC1 per C2) proven by that Verify output; E2 judged directly (architecture-map.md:44-46). Prohibition N4 held: da-graph/legacy/ appear only removed-marked, ADR-042–049 only proposed-marked (architecture-map.md §1, §5, §7).
- Excluded by C2: E1's committed check-script form — held-out enforcement lives in the task Verify command, not in a committed test file.

### SC2 — doc/review/functionality-ledger.md: closed enumeration — every subcommand from the dispatch match in crates/da-cli/src/main.rs plus every use-case in crates/da-application — each with one verdict from exactly {работает, кандидат, заглушка, unverified} and an evidence path; no dispatch entry omitted.: pass
- ✅ Reviewer re-enumeration over the landing tree: 38/38 dispatch arms and 103/103 `pub mod` use-case modules present in the ledger; verdict cells scanned — only {работает: 2, кандидат: 165, unverified: 3}, one verdict per command, every row with an evidence path resolving at HEAD (recorded T003 Verify pass, exit 0). Held-out edge E3 (criterion SC2 per C2) proven by the recorded Verify vocabulary grep; E4 judged directly: closure over the main.rs match, help-only/internal arms included, `#[command(hide)]` absence grepped (ledger §Enumeration method). Prohibition N5 held: both «работает» rows cite `/tmp/arch-20261123-pass2.log` ran-in-gate; §2 preamble downgrades crate-level execution to кандидат.
- Excluded by C2: E3's committed check-script form — held-out enforcement lives in the task Verify command, not in a committed test file.

### SC3 — doc/review/grobid-state.md contains four sections: (a) production route + identity (GROBID 0.9.1, D576) with citations; (b) table of all R123 ladder families (lattice/segmentation, header, bibliography, body accounting, visual) with per-family status shadow/candidate/promoted and evidence paths; (c) retirement-package status; (d) exhaustive list of gates separating current state from any production flip, each with a feasibility note.: pass
- ✅ All four sections present (grobid-state.md:15, 33, 71, 88); (a) cites GROBID 0.9.1 digest pin in `.env.example`, D576/D572–D575 [documentary], rollback 0.9.0-crf; (b) five R123 family rows all кандидат with suite evidence paths, typed-absent statement for no-code families (E6: none needed), beyond-set findings in own candidate rows (E5); (c) acceptance package + readiness receipt + source binding + authorization contract, all cited; (d) all four blocking gates each with a feasibility note classified judgment-not-measurement (E5/E6 edges judged directly). Edge E5: beyond-set rows at lines 65-70. Edge E6: typed-absent statement at line 63. Prohibition N2 held: no promoted claim, no ready wording without gate receipt; feasibility notes explicitly "judgments…

## Fixed since last cycle

<!-- C == 1: nothing to re-check. -->

## Summary for orchestrator

- blocked findings: none — wave verdict pass
- repeat offenders: none (first cycle)
- warnings worth a human eye: grobid-state.md §(b) garbled feature-gate sentence (prose leftover, claim correct); architecture-map.md "[removed; more no longer exists in the tree]" phrasing (twice); functionality-ledger.md §2 rows 101–103 out of alphabetical order; SC3 status-vocabulary reading (кандидат=candidate) may deserve literal tightening in a later plan.
