# Plan — daily-archive M001 "orientation"

<!-- Written by the planner role. Executed wave-by-wave by /skill:gsd-path-build. -->

Project verify: `python3 - <<'PYEOF'
import re, subprocess, sys
diff = subprocess.run(['git', 'diff', '--name-only', 'origin/master...HEAD'], capture_output=True, text=True).stdout.split()
product = [p for p in diff if not (p.endswith('.md') or p.startswith('.project/') or p == '.gitignore')]
if product:
    print('NON-MD PRODUCT DIFF (N1/N3 per INTENT C3):', product); sys.exit(1)
reports = ['doc/review/architecture-map.md', 'doc/review/functionality-ledger.md', 'doc/review/grobid-state.md']
ok = True
for r in reports:
    try:
        text = open(r, encoding='utf-8').read()
    except OSError:
        print('MISSING REPORT:', r); ok = False; continue
    if len(text) < 2000:
        print('STUB REPORT:', r); ok = False
    paths = set(re.findall(r'\`((?:crates|artifacts|data|scripts|doc|\.github)/[^\ \`]+)\`', text))
    missing = [p for p in paths if subprocess.run(['git', 'cat-file', '-e', 'HEAD:' + p], capture_output=True).returncode != 0]
    if missing:
        print('DEAD CITATIONS in', r, ':', missing); ok = False
stale = [('README.md', '037–042'), ('README.md', '14 stages'), ('AGENTS.md', 'enforces 7 rules'), ('PRODUCT.md', '14-stage'), ('doc/OKF.md', 'ADR-001…067')]
for f, s in stale:
    if s in open(f, encoding='utf-8').read():
        print('STALE CLAIM REMAINS:', f, s); ok = False
sys.exit(0 if ok else 1)
PYEOF`
<!-- Runs once at ship, in a fresh sidecar worktree checked out at the reviewed
     commit on gsd-path/M001: no build output or untracked env files exist
     there. Uses only git + python3 stdlib (no cargo, no installs) per the
     evidence budget: static + recorded evidence, .md-only diff. -->

## Config

- max_review_cycles: 3
- wave_budget: none
- review_panel: off      <!-- copied from INTENT.md Review panel: off -->
- finding_skeptics: off  <!-- copied from INTENT.md Finding skeptics: off -->

## Wave 1 — walking skeleton: grobid-state first, then the other two reports

Goal: all three doc/review/ assessment reports exist, every architecture/dependency claim cites a code path that exists at HEAD 0f06b764c (E1 machinery proven on the first report), ledger verdicts use exactly the closed vocabulary with proof-weight tags, and the GROBID gate inventory is enumerated from the acceptance-package JSON — without flipping any gate. The wave-1 blockers from SYNTHESIS burn down inside T001 before the sibling reports are drafted: (1) the citation git-enumeration pipeline is proven against the first drafted report, (2) the preflight scorecard numbers (measurability 5/21 (23.8%), feature_gated_coverage 19/33 (57.6%)) are confirmed verbatim against /tmp/preflight-20261122.log, (3) the da-parse feature-gate absence claim is re-checked at grep level. If the wave fails, the citation pipeline or gate-inventory method is wrong and the report set cannot be trusted for milestone decisions.

Review depth: full

## Wave 2 — mechanical fix-doc remediation

Goal: the DOCS-AUDIT.md remediation rows already ruled fix-doc (rows 1–9, 11–14, 16–22 plus row 10 wording alignment) are corrected across README.md, AGENTS.md, PRODUCT.md, OKF.md, crate READMEs, ADR-INDEX.md, doc/OKF.md, scripts/OKF.md, and .docker/README.md — .md-only, no code or config change. Rows 15, 23, 24 are accept-drift (SYNTHESIS settled; U2): no action. If it fails, stale claims persist into downstream milestones' ground truth.

Review depth: verify-only   <!-- mechanical .md corrections; each row's stale string is directly covered by the task Verify. -->

## Surface contract

### doc/review/grobid-state.md — T001

Criteria: SC3
Entry: open `doc/review/grobid-state.md` on branch gsd-path/M001.
States: empty — file absent (task not done); success — four sections (a) production route + identity (GROBID 0.9.1, D576, rollback 0.9.0-crf) with citations, (b) R123 ladder family table with per-family shadow/candidate/promoted status and evidence paths, typed-absent rows for families with no code (E6), out-of-set finds get their own candidate rows (E5), (c) retirement-package status, (d) exhaustive gate list sourced from `artifacts/parse_compat/grobid_retirement_acceptance_package.json` (`mandatory_gates` + `blocking_gates`) cross-checked against `artifacts/parse_compat/grobid_retirement_readiness.json`, each gate with a feasibility note labeled a judgment not a measurement, plus the recorded preflight axes as a dated stale snapshot; loading/error do not apply to a committed report (a reviewer sees commit + diff instead). No gate is declared flippable; no "ready" wording without a gate-receipt citation (N2).
Walkthrough:
1. `git log --oneline gsd-path/M001 -- doc/review/grobid-state.md` — the report is committed on the bound branch.
2. Read the four sections; for three cited evidence paths at random, `git cat-file -e HEAD:<path>` succeeds.
3. Check §(d) against `artifacts/parse_compat/grobid_retirement_acceptance_package.json` blocking_gates — all four blocking ids appear.
4. Confirm no verdict declares a native family production-ready without a gate-receipt citation.

### doc/review/architecture-map.md — T002

Criteria: SC1
Entry: open `doc/review/architecture-map.md` on branch gsd-path/M001.
States: empty — file absent (task not done); success — every architecture/dependency claim line carries a concrete path citation into crates/ or Cargo.toml that exists at HEAD (E1), absence claims cite the checked location (E2), Proposed ADRs 042–049 and removed crates (da-graph, legacy/) appear only explicitly marked proposed/removed (N4); the live-test boundary is drawn at the gate script's provisioning list (`scripts/governor/__main__.py:784-800`), not at crates/da-parse; CI-vs-gate coverage is stated explicitly wherever suite execution is cited (RQ2 ruling U1: CI untouched). Loading/error do not apply; a reviewer sees commit + diff.
Walkthrough:
1. `git log --oneline gsd-path/M001 -- doc/review/architecture-map.md` — committed on the bound branch.
2. Run the task's citation-enumeration Verify — zero dead paths.
3. Grep the report for `da-graph` and `legacy/` — each occurrence sits in a sentence explicitly marking removed/proposed status.
4. Find the service-dependent-suite boundary sentence — it names the gate script provisioning list.

### doc/review/functionality-ledger.md — T003

Criteria: SC2
Entry: open `doc/review/functionality-ledger.md` on branch gsd-path/M001.
States: empty — file absent (task not done); success — closed enumeration: every subcommand from the dispatch match in `crates/da-cli/src/main.rs` (help-only/hidden included, E4) plus every use-case in crates/da-application, each with exactly one verdict from {работает, кандидат, заглушка, unverified} (E3) and an evidence path; every «работает» cites ran-in-gate or recorded-run evidence, otherwise downgraded (N5); each entry carries a proof-weight tag (ran-in-gate / static-only / unverified). Loading/error do not apply; a reviewer sees commit + diff.
Walkthrough:
1. `git log --oneline gsd-path/M001 -- doc/review/functionality-ledger.md` — committed on the bound branch.
2. Run the task's vocabulary Verify — zero out-of-vocabulary verdicts.
3. Pick two subcommand rows; their evidence paths resolve at HEAD.
4. Grep «работает» rows — each cites /tmp/arch-20261123-pass2.log or another recorded run.

## Intent coverage

| Criterion | Task | Acceptance |
|-----------|------|------------|
| SC1 | T002 | AC1 |
| SC1 | T002 | AC2 |
| SC1 | T002 | AC3 |
| SC1 | T002 | AC4 |
| SC2 | T003 | AC1 |
| SC2 | T003 | AC2 |
| SC2 | T003 | AC3 |
| SC2 | T003 | AC4 |
| SC3 | T001 | AC1 |
| SC3 | T001 | AC2 |
| SC3 | T001 | AC3 |
| SC3 | T001 | AC4 |
| SC3 | T001 | AC5 |
| SC4 | T004 | AC1 |
| SC4 | T004 | AC2 |

## Dependency notes

- T002, T003 depend on T001: the E1 citation git-enumeration Verify pattern and the three wave-1 blocker confirmations (preflight numbers verbatim, da-parse feature-gate absence recheck, dead-path enumeration) are proven on the first drafted report and recorded in T001's Log; the sibling reports reuse that proven method rather than re-deriving it. Reviewer evidence: T001's recorded Verify output plus its Log entry. No file overlap (each report is its own file).
- T004 has no dependency on wave-1 tasks: the DOCS-AUDIT stale strings it corrects exist independently of the new reports; placing it in wave 2 is plan order (mechanical work last), not a data dependency.
- No held-out rows: INTENT Correction C2 re-dispositioned E1/E3 to criterion SC1/SC2 with Verify-side commands; the edges are enforced inside T002/T003 Verify and Acceptance, not in committed test files.

## Edge / prohibition coverage placement

- E1 (SC1) → T002 Verify + AC1; E2 (SC1) → T002 AC2; N4 (SC1) → T002 Approach.
- E3 (SC2) → T003 Verify + AC2; E4 (SC2) → T003 AC1; N5 (SC2) → T003 Approach + AC3.
- E5/E6 (SC3) → T001 AC2/AC3; N2 (SC3) → T001 Approach + AC5.
- N1/N3 (SC4) → all tasks Approach; enforced by T004 AC2 (.md-only diff check) and Project verify.
