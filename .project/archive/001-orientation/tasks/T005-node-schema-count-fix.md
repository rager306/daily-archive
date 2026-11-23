---
id: T005
title: Fix stale "31 node schemas" live claims to 43 (finding P001, .md-only)
wave: 3
deps: []
status: done
agent: build_T005
base: 5eb7649787e61b97dde0e24b5d20ce462b5016b0
worktree: null
task_branch: null
files:
  - AGENTS.md
---

# T005 — Fix stale "31 node schemas" live claims to 43 (finding P001)

## Context

Ship final review (`.project/review/final-gap-2.md`, PATCH-FINDINGS.md P001, reviewed HEAD 7e891591a5a0da7131f61f4050bb8fd0c6dd09a3) found the doc set internally inconsistent on the node-schema count. Finding evidence, verbatim:

> 1 contradiction. All figures the wave-2 edits introduced are correct against the sources and consistent with the wave-1 reports, but AGENTS.md:348 ("Export all 31 node schemas to YAML") contradicts `data/ontology/node_schemas.yaml:12` (43 schemas) and the corrected counts in README.md:25 and PRODUCT.md:15 — the doc set is internally inconsistent on exactly the figure class wave 2 was remediated to fix.

Fix direction (verbatim): extend the row-7-class correction to AGENTS.md:348 — change "31 node schemas" to "43 node schemas" (rendered from `data/ontology/node_schemas.yaml`). The primary site is the bash comment at AGENTS.md:348. The patch instruction additionally requires sweeping the whole tracked .md tree for other live assertions of the same stale pattern: the sweep found none outside archival/historical context — the remaining matches (AGENTS.pre-path.md backup snapshot; ADR-048 "30 → 31 schemas after ADR-047 Conflict"; ADR-050 migration-churn estimate; the dated ONTOLOGY-DESIGN-V2 / ONTOLOGY-COMPLETENESS-AUDIT snapshots; .project findings docs quoting the string verbatim as evidence) are historical records of counts true at their writing and must not be rewritten. N1/N3 dominate: .md-only diff. This task repairs a gap finding and owns no INTENT success criterion. The Verify must prove the pattern absent codebase-wide (grep over `git ls-files '*.md'` plus a count check against `data/ontology/node_schemas.yaml`), not only at AGENTS.md:348.

## Approach

- Render the corrected count from the current source, not from memory: `data/ontology/node_schemas.yaml` currently defines 43 node schemas (`^- label:` entries). Change the AGENTS.md:348 comment "Export all 31 node schemas to YAML" to "Export all 43 node schemas to YAML".
- Do not touch README.md, PRODUCT.md, or doc/OKF.md — their 43-count figures are already correct per the finding evidence.
- Do not touch any of the archival/historical matches listed in Context; rewriting them would falsify dated records.
- If the YAML count is no longer 43 when this task runs, re-render the replacement number from the YAML at that moment and record the delta in the Log; never hardcode from this file.

## Interface contract

- None

## Intent coverage

- None

## Acceptance criteria

1. AGENTS.md contains "Export all 43 node schemas to YAML" and no longer contains "31 node schemas".
2. The codebase-wide sweep in the Verify command exits 0: no tracked .md outside the enumerated archival/historical set contains "31 node schemas" or "31 schemas", and the `^- label:` count in `data/ontology/node_schemas.yaml` equals 43.
3. `git diff` for this task touches only AGENTS.md (N1/N3: .md-only).

## Verify

```bash
test -f AGENTS.md || { echo "missing AGENTS.md"; exit 1; }
python3 - <<'PYEOF'
import subprocess, sys
files = subprocess.run(['git', 'ls-files', '*.md'], capture_output=True, text=True).stdout.split()
# Archival/historical matches: counts true at their writing, quoted evidence,
# or backup snapshots — never rewritten by this patch.
allowed = {
    'AGENTS.pre-path.md',
    'doc/adr/ADR-048-decision-intelligence.md',
    'doc/adr/ADR-050-universal-graph-subsystem-kg-crate-family.md',
    'doc/ONTOLOGY-DESIGN-V2.md',
    'doc/ONTOLOGY-COMPLETENESS-AUDIT.md',
    '.project/review/PATCH-FINDINGS.md',
    '.project/review/final-gap-2.md',
    '.project/research/DOCS-AUDIT.md',
    '.project/plan/PLAN.md',
    '.project/tasks/T005-node-schema-count-fix.md',
}
ok = True
for f in files:
    text = open(f, encoding='utf-8').read()
    if ('31 node schemas' in text or '31 schemas' in text) and f not in allowed:
        print('STALE LIVE CLAIM:', f); ok = False
n = sum(1 for l in open('data/ontology/node_schemas.yaml', encoding='utf-8') if l.startswith('- label:'))
if n != 43:
    print('YAML COUNT CHANGED, re-render the figure:', n); ok = False
agents = open('AGENTS.md', encoding='utf-8').read()
if 'Export all 43 node schemas to YAML' not in agents:
    print('AGENTS.md NOT FIXED'); ok = False
if '31 node schemas' in agents:
    print('STALE REMAINS in AGENTS.md'); ok = False
sys.exit(0 if ok else 1)
PYEOF
```
<!-- Fails when skipped: the stale string check, the codebase-wide grep over
     `git ls-files '*.md'`, and the 43-count check against
     data/ontology/node_schemas.yaml all gate exit 0. Names AGENTS.md and
     data/ontology/node_schemas.yaml from this task's inputs. -->

Heavy: no

## Log

- 2026-11-22 — created by planner (patch wave 3, finding P001)
- 2026-11-23 — Orchestrator answer: reading (a) — `.project/plan/PLAN.md` and this task file are quoted-pattern/evidence files of the same class the Verify's allowed set already admits (PATCH-FINDINGS.md, final-gap-2.md); neither asserts a live count. Verify `allowed` extended with both; contract text unchanged otherwise. Documented plan-defect repair of the Verify field named by the blocker; base advances to the workspace commit carrying this repair; task reset to pending for re-activation.

- 2026-11-23 — build_T005: applied AGENTS.md:348 fix ("31 node schemas" → "43 node schemas"; YAML renders 43 `- label:` entries). Verify exits 1: two additional tracked .md matches outside the Verify's `allowed` set — `.project/plan/PLAN.md:56` (goal text quoting the swept patterns verbatim) and this task file itself (evidence quotes + Verify script source). Neither asserts a live count. NEEDS-ORCHESTRATOR: should the Verify `allowed` set include `.project/plan/PLAN.md` and `.project/tasks/T005-node-schema-count-fix.md` (pattern quoted verbatim, not live count claims), or must these files be edited? — readings: (a) add both to `allowed` as quoted-evidence files of the archival class; (b) keep Verify as written and edit the two files, contradicting the touch-only-AGENTS.md rule.
- 2026-11-23 — orchestrator Verify (sidecar gsd-path-verify/task-t005-verify): pass, exit 0
- 2026-11-23 — orchestrator regression Verify of T004 (sidecar gsd-path-verify/task-t005-verify): pass, exit 0
