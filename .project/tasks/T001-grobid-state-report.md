---
id: T001
title: Draft and commit doc/review/grobid-state.md (walking skeleton: gate inventory from acceptance-package JSON)
wave: 1
deps: []
status: pending
agent: null
base: null
worktree: null
task_branch: null
files:
  - doc/review/grobid-state.md
---

# T001 — Draft and commit doc/review/grobid-state.md

## Context

M001 is the owner's orientation/assessment milestone: verified, evidence-cited reports before any further milestone decision. This task is the walking skeleton — the riskiest report first. It burns the three wave-1 blockers from SYNTHESIS "For the planner": (1) prove the E1 citation pipeline (every cited path git-enumerated as existing at HEAD 0f06b764c) on this first drafted report; (2) confirm the preflight scorecard numbers verbatim from `/tmp/preflight-20261122.log` (`measurability 5/21 (23.8%)`, `feature_gated_coverage 19/33 (57.6%)`) before they enter §(d); (3) re-check at grep level the claim that no `#![cfg(feature)]` gate exists on any `crates/da-parse/tests/*.rs` file before any report claims the 146 suites run as one ungated unit. Evidence budget: static reading + codebase-memory-mcp toolset + recorded logs only; no new cargo/test runs. SC3d gate inventory source of truth (settled decision): `artifacts/parse_compat/grobid_retirement_acceptance_package.json` (`mandatory_gates` + `blocking_gates`, incl. `threshold.extraction_weakest_family`, `body_axis.body_not_assessable`, `named_gap_learning_ruvllm_ablation`, `h20_frozen_external_holdout_decision`, `h21_revision_label_residual`, `learning.s07_measured_no_gain`), cross-checked against `artifacts/parse_compat/grobid_retirement_readiness.json` (`claim_scope: blocked_pending_human_authorization`, four refusal codes).

## Approach

- Four sections exactly: (a) production route + identity — GROBID 0.9.1 current (D576), 0.9.0-crf rollback target, D572–D575 carry; cite code paths (e.g. `crates/da-parse` GrobidParser surface, `.env.example` GROBID_* keys). (b) R123 ladder family table (lattice/segmentation, header, bibliography, body accounting, visual) with per-family status shadow/candidate/promoted and evidence paths; the family set is exactly the R123 ladder — anything found beyond it gets its own candidate row (E5); a family with no code gets a typed-absent row, never a silent skip (E6). Same-n suite pointers: `crates/da-parse/tests/native_{header,bibliography,fulltext,visual_family}_same_n.rs`, `java_lift_same_n_oracle.rs`. (c) retirement-package status — acceptance package + readiness receipt + source binding `crates/da-cli/src/native_retirement_readiness.rs` (`READINESS_EVIDENCE_CARRIERS`, sha256 path+pointer binding). (d) exhaustive gate list from the acceptance-package JSON, each gate with a feasibility note per the settled RQ1 classification: all gates except two are execution of existing packages (conservation receipts `crates/da-application/src/native_family_promotion_gate.rs`, `parse_learning_cycle.rs`, `context_conservation.rs`; same-n suites; dual-metric/val_gap machinery `crates/da-cli/examples/policy_corpus/evaluation.rs:935`, `crates/da-cli/src/native_acceptance_package.rs:138`; R135/R130 classifier `crates/da-application/src/derived_evidence_maturity.rs`; authorization contract `authorize_native_retirement`; governor DATA checks `scripts/governor/adapters/checks/external_holdout_package.py`, `grobid_absorption_progress.yaml`). `body_axis.body_not_assessable` needs new measurement infrastructure (independent body-localization oracle; none in tree). `named_gap_learning_ruvllm_ablation` needs a Java-donor ablation build or an owner scope decision — record both as roadmap-input, not resolved (D471 stays fail-closed typed_refusal).
- Report the recorded preflight axes verbatim as a dated stale snapshot (log predates M001 doc edits); feasibility notes are judgments, not measurements — say so explicitly.
- Every suite-execution citation must state explicitly that GitHub CI does not execute the da-parse integration suites (CI runs `cargo test -p da-parse --lib` per `.github/workflows/architecture-guardrail.yml`); the local governor gate recorded 23/23 PASS at `/tmp/arch-20261123-pass2.log` is the only execution plane of record (owner ruling U1 — CI untouched). Never claim a suite is CI-protected.
- Prohibition N2: any "ready" wording for a native family carries a gate-receipt citation; without it the row stays candidate. No gate flip, no pin/threshold/status change — this report is assessment only (N1: git diff shows only .md files).
- Every cited path is re-verified by the Verify command below before landing; every «работает»-grade claim traces to a recorded run.
- The E1 citation-enumeration Verify pattern (extract backticked repo paths → `git cat-file -e HEAD:<path>`) proven here is recorded in this task's Log for T002 to reuse verbatim; it is not a committed script (C2: no committed check scripts in M001).

## Interface contract

- None

## Intent coverage

- SC3

## Acceptance criteria

1. `doc/review/grobid-state.md` exists, contains sections (a)–(d), and every path citation in it resolves at HEAD (`git cat-file -e`); the Verify command below exits 0.
2. §(b) covers exactly the R123 families; any beyond-set finding has its own candidate row (E5) and any no-code family has a typed-absent row (E6) — no family silently missing.
3. §(b) statuses use only shadow/candidate/promoted; §(d) lists every blocking gate from the acceptance-package JSON — cross-checking §(d) against `artifacts/parse_compat/grobid_retirement_acceptance_package.json` `blocking_gates` finds all four blocking ids present.
4. The preflight numbers in §(d) are byte-equal to the scorecard lines in `/tmp/preflight-20261122.log` and explicitly marked as a dated, stale snapshot; feasibility notes state they are judgments, not measurements.
5. No sentence declares a native family production-ready without a gate-receipt citation (N2), and no gate is described as flippable within M001.
6. The wave-1 blockers are recorded done in the Log: preflight numbers confirmed verbatim; da-parse feature-gate absence re-checked (grep `cfg(feature` over `crates/da-parse/tests/*.rs`); E1 enumeration run clean on this report.

## Verify

```bash
test -f doc/review/grobid-state.md || { echo "missing doc/review/grobid-state.md"; exit 1; }
python3 - <<'EOF'
import re, subprocess, sys
text = open('doc/review/grobid-state.md', encoding='utf-8').read()
ok = True
for sec in ['(a)', '(b)', '(c)', '(d)']:
    if sec not in text:
        print('MISSING SECTION', sec); ok = False
if len(text) < 2000:
    print('STUB REPORT'); ok = False
paths = set(re.findall(r'`((?:crates|artifacts|data|scripts|doc|\.github|\.env\.example)/[^`]+)`', text))
missing = [p for p in paths if subprocess.run(['git','cat-file','-e','HEAD:'+p], capture_output=True).returncode != 0]
if missing:
    print('DEAD CITATIONS:', missing); ok = False
pkg = open('artifacts/parse_compat/grobid_retirement_acceptance_package.json', encoding='utf-8').read()
for gid in ['threshold.extraction_weakest_family','body_axis.body_not_assessable','named_gap_learning_ruvllm_ablation','h20_frozen_external_holdout_decision','h21_revision_label_residual','learning.s07_measured_no_gain']:
    if gid not in text:
        print('GATE MISSING FROM REPORT:', gid); ok = False
    if gid not in pkg:
        print('GATE NOT IN PACKAGE (check id):', gid); ok = False
if '23.8' not in text or '57.6' not in text:
    print('PREFLIGHT AXES MISSING'); ok = False
for fam in ['lattice', 'segmentation', 'header', 'bibliography', 'body', 'visual']:
    if fam not in text.lower():
        print('FAMILY ROW MISSING:', fam); ok = False
sys.exit(0 if ok else 1)
EOF
```
<!-- Fails when the task is skipped (file absent → open raises), when any cited path is dead at HEAD (E1), when a gate or family row or preflight axis is missing. Names doc/review/grobid-state.md from files. -->

Heavy: no

## Log

- 2026-11-22 — created by planner
