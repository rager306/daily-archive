---
id: T004
title: Apply DOCS-AUDIT fix-doc remediation rows (mechanical, .md-only)
wave: 2
deps: []
status: pending
agent: null
base: null
worktree: null
task_branch: null
files:
  - README.md
  - AGENTS.md
  - PRODUCT.md
  - OKF.md
  - crates/kg-ontology/README.md
  - crates/kg-pipeline/README.md
  - crates/kg-algorithms/README.md
  - doc/adr/ADR-INDEX.md
  - doc/OKF.md
  - scripts/OKF.md
  - .docker/README.md
---

# T004 — Apply DOCS-AUDIT fix-doc remediation rows

## Context

The docs audit (.project/research/DOCS-AUDIT.md, audited HEAD 0f06b764c) found 24 non-verified rows; the owner's rulings bundle (2026-11-22, recorded in INTENT Constraints) ruled rows 1–9, 11–14, 16–22 fix-doc (mechanical .md corrections), row 10 fix-doc (align the ".project/ is committed" wording to pipeline-committed reality), and rows 15, 23, 24 accept-drift (resolved by router initialization / MCP-managed mirror / frozen engine) — no action on 15/23/24. This wave is mechanical: correct stale counts, paths, and dead `legacy/` references in the listed docs. N1/N3 dominate: git diff shows only .md files — no code or config change, even where a row's class was fix-code (row 14: the .md-only resolution is dropping or correcting the stale `logs/` line, not creating the directory). Verify re-runs the audit's claim checks per the patch-mode convention.

## Approach

- Row-by-row (source of truth for wording: DOCS-AUDIT.md `## Remediation queue`; render replacement values from the current code/YAML, not from the audit's suggested numbers alone where a fresher count exists):
  1. README.md: ADR range "037–042" → current range or "see ADR-INDEX".
  2. README.md: "14 stages" → current stage count from `data/ontology/pipeline_stage_contracts.yaml` (19 per audit; verify against the YAML).
  3. README.md: summary generator binding → current models.yaml summary bindings (summary-roles-v0.2 tier).
  4. README.md: drop or repoint the prompt-optimizer readiness-evidence claim (DOCS-AUDIT row 4; the cited runtime artifact does not exist at HEAD) to surviving evidence.
  5. README.md: remove the source-recovery corpus-wave section or repoint to archived evidence.
  6. README.md: extraction metrics claim → cite the surviving archive/artifacts-legacy copy; keep numbers only if the cited copy carries them.
  7. README.md: schema registry counts → current node-schema/index counts from the YAML registry (audit: 56 indexes, 43 schemas).
  8. README.md: remove stale layout entries (dead `legacy/` paths, `data/article_catalog/article_catalog/arxiv/` doubling).
  9. AGENTS.md: "Reverse-ADR audit enforces 7 rules" → current rule count from `scripts/governor/checks/adr_rules.yaml` (audit: 65).
  10. AGENTS.md: align the ".project/ is committed; ... user work in stash@{0}" line to pipeline-committed reality — .project/ is committed by pipeline checkpoint commits; drop or mark the stash note as historical, not current state.
  11–13. PRODUCT.md: stage ledger count (→ 19), schema registry indexes (→ 56), ontology schemas (→ 43) — render from the YAML sources.
  14. OKF.md: drop or correct the `logs/` tree-layout line (.md-only; do not create logs/).
  16. crates/kg-ontology/README.md: rewrite the Status section to the current module surface.
  17. crates/kg-pipeline/README.md: refresh stale test counts.
  18. crates/kg-algorithms/README.md: rewrite Status to include csr/ppr/view algorithms and current counts.
  19. doc/adr/ADR-INDEX.md: drop the dead `legacy/doc/adr/` pointer for Python-era ADRs (legacy/ is a restore stub).
  20. doc/OKF.md: ADR range "ADR-001…067" → current range (audit: through ADR-080).
  21. scripts/OKF.md: remove the retired `check_project_trajectory.py` entry-point line.
  22. .docker/README.md: drop the `legacy/` frozen-paths sentence.
- Rows 15, 23, 24: no action (accept-drift; U2 keeps the MCP-managed adr.md mirror).
- N1/N3 hard limit: every change lands in the .md files listed above; nothing else.

## Interface contract

- None

## Intent coverage

- SC4

## Acceptance criteria

1. Every fix-doc row above is corrected: the stale strings listed in the Verify command are absent from their files, and the replacement counts rendered from code/YAML are in place.
2. The cumulative milestone diff `git diff --name-only origin/master...HEAD` contains only .md paths (N1/N3, SC4/E7), and the wave-1 reports remain committed on gsd-path/M001.
3. Rows 15, 23, 24 are untouched (accept-drift).

## Verify

```bash
for f in README.md AGENTS.md PRODUCT.md OKF.md crates/kg-ontology/README.md crates/kg-pipeline/README.md crates/kg-algorithms/README.md doc/adr/ADR-INDEX.md doc/OKF.md scripts/OKF.md .docker/README.md; do test -f "$f" || { echo "missing $f"; exit 1; }; done
python3 - <<'EOF'
import subprocess, sys
stale = [
    ('README.md', '037–042'),
    ('README.md', '14 stages'),
    ('README.md', 'agnes-ai/agnes-2.0-flash'),
    ('README.md', 'prompt_optimizer_readiness_m387.json'),
    ('README.md', 'awesome_agentic_reasoning'),
    ('README.md', 'eval_aggregate_all_sources.json'),
    ('AGENTS.md', 'enforces 7 rules'),
    ('AGENTS.md', 'stash@{0}'),
    ('PRODUCT.md', '14-stage'),
    ('OKF.md', 'logs/'),
    ('crates/kg-pipeline/README.md', 'Phase E: 7 tests'),
    ('crates/kg-algorithms/README.md', '9 tests'),
    ('doc/adr/ADR-INDEX.md', 'legacy/doc/adr/'),
    ('doc/OKF.md', 'ADR-001…067'),
    ('scripts/OKF.md', 'check_project_trajectory.py'),
    ('.docker/README.md', 'legacy/'),
]
ok = True
for f, s in stale:
    if s in open(f, encoding='utf-8').read():
        print('STALE CLAIM REMAINS:', f, repr(s)); ok = False
diff = subprocess.run(['git','diff','--name-only','origin/master...HEAD','--','*.md',':(exclude).project/**',':(exclude).gitignore'], capture_output=True, text=True).stdout.split()
nonmd = [p for p in diff if not p.endswith('.md')]
if nonmd:
    print('NON-MD DIFF PATHS (N1/N3, product scope per INTENT C3):', nonmd); ok = False
pipeline_paths = subprocess.run(['git','diff','--name-only','origin/master...HEAD'], capture_output=True, text=True).stdout.split()
outside = [p for p in pipeline_paths if not (p.endswith('.md') or p.startswith('.project/') or p == '.gitignore')]
if outside:
    print('SCOPE VIOLATION (C3 allows only product .md + pipeline-owned state):', outside); ok = False
if not diff:
    print('EMPTY MILESTONE DIFF'); ok = False
for r in ['doc/review/grobid-state.md','doc/review/architecture-map.md','doc/review/functionality-ledger.md']:
    if subprocess.run(['git','ls-files','--error-unmatch',r], capture_output=True).returncode != 0:
        print('REPORT NOT COMMITTED:', r); ok = False
sys.exit(0 if ok else 1)
EOF
```
<!-- Fails when any fix-doc row's stale claim remains, when the milestone diff carries a non-.md path (SC4/N1/N3), or when a wave-1 report is missing from the branch. Names the listed files from this task's files. -->

Heavy: no

## Log

- 2026-11-22 — created by planner
- 2026-11-22 — build_T004: all fix-doc rows applied (1–14, 16–22; counts re-rendered from source: 19 stages, 43 node schemas, 46+10=56 indexes, 65 ADR rules, ADR range through 080, models.yaml binding paper-summary-generate-default → agnes-ai/agnes-2.5-flash + summary-roles-v0.2 quality tier; rows 15/23/24 untouched). Stale-claim portion of Verify passes (exit 0 on claim checks).
- 2026-11-22 — NEEDS-ORCHESTRATOR: task Verify fails on `git diff --name-only origin/master...HEAD` carrying two non-.md paths — `.gitignore` and `.project/build/verify-ledger.jsonl` — committed by earlier branch commits 2266246fc / 3031bf3ec / 5a32c9ddc ("build: T001/T003 verify ledger"), which are outside T004's .md-only file scope and cannot be touched by this task — readings: (a) waive the N1/N3 check for those committed build/ledger paths so Verify can pass on T004's diff; (b) have the orchestrator squash/remove those build commits from the milestone diff; (c) reword AC2/Verify to check only the task's own changed paths.
- 2026-11-22 — Orchestrator answer: reading (a) per INTENT.md ## Corrections C3 — "N1/E7 «только .md» действует на product-дифф; pipeline-owned состояние (.project/**, однострочный /build/ анкор в .gitignore) разрешено". The Verify range check is amended to scope the .md-only assertion to product files and to flag only paths outside {product .md, .project/**, .gitignore}; readings (b)/(c) rejected — (b) rewrites sanctioned history, (c) weakens SC4. Documented plan-defect repair of the Verify field named by the blocker; base advances to the workspace commit carrying this repair.
