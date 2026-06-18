# M074 Scoped Commit Plan

## Safety statement

This plan performed **no git add**, **no git commit**, and **no git push**.

Use exact pathspec files only. Do not use `git add .` or `git add -A` in this dirty tree.

Excluded unrelated dirty entries: `153`

## Recommended groups

### 01-m069-research: M069 Agents-K1 research artifacts

- status: `default`
- pathspec_file: `artifacts/m074-scoped-commit-plan/pathspecs/01-m069-research.txt`
- path_count: `38`

Stage command after explicit approval:

```bash
git add --pathspec-from-file=artifacts/m074-scoped-commit-plan/pathspecs/01-m069-research.txt
```

Verification before commit:

- `test -d artifacts/m069-agents-k1-research`
- `rg -n "Agents-K1|schema|metric|benchmark|M064" artifacts/m069-agents-k1-research`

Notes:
- Research/design artifacts only; no code verification beyond artifact existence.

<details><summary>Pathspecs</summary>

```text
artifacts/m069-agents-k1-research/GAP_ANALYSIS.md
artifacts/m069-agents-k1-research/PAPER_SUMMARY.md
artifacts/m069-agents-k1-research/benchmark-contract.md
artifacts/m069-agents-k1-research/m064-assumptions.md
artifacts/m069-agents-k1-research/m064-reassessment.md
artifacts/m069-agents-k1-research/metrics-source-notes.md
artifacts/m069-agents-k1-research/schema-diff.md
artifacts/m069-agents-k1-research/schema-source-notes.md
artifacts/m069-agents-k1-research/source-extracts/7-2-occ1.txt
artifacts/m069-agents-k1-research/source-extracts/7-2-occ2.txt
artifacts/m069-agents-k1-research/source-extracts/7-2-occ3.txt
artifacts/m069-agents-k1-research/source-extracts/7-2.txt
artifacts/m069-agents-k1-research/source-extracts/7-6-information-extraction-backbone-evaluation-occ1.txt
artifacts/m069-agents-k1-research/source-extracts/7-6-information-extraction-backbone-evaluation-occ2.txt
artifacts/m069-agents-k1-research/source-extracts/appendix-d-schema-compact.txt
artifacts/m069-agents-k1-research/source-extracts/citation-context-classification-schema-occ1.txt
artifacts/m069-agents-k1-research/source-extracts/citation-context-classification-schema-occ2.txt
artifacts/m069-agents-k1-research/source-extracts/citation-context-classification-schema-occ3.txt
artifacts/m069-agents-k1-research/source-extracts/citation-context-classification.txt
artifacts/m069-agents-k1-research/source-extracts/contain-acc-occ1.txt
artifacts/m069-agents-k1-research/source-extracts/contain-acc-occ2.txt
artifacts/m069-agents-k1-research/source-extracts/contain-acc-occ3.txt
artifacts/m069-agents-k1-research/source-extracts/contain-acc.txt
artifacts/m069-agents-k1-research/source-extracts/d-disaggregated-knowledge-graph-schema-occ1.txt
artifacts/m069-agents-k1-research/source-extracts/d-disaggregated-knowledge-graph-schema-occ2.txt
artifacts/m069-agents-k1-research/source-extracts/disaggregated-knowledge-graph-schema-occ1.txt
artifacts/m069-agents-k1-research/source-extracts/disaggregated-knowledge-graph-schema-occ2.txt
artifacts/m069-agents-k1-research/source-extracts/disaggregated-knowledge-graph-schema.txt
artifacts/m069-agents-k1-research/source-extracts/gpt-acc-occ1.txt
artifacts/m069-agents-k1-research/source-extracts/gpt-acc-occ2.txt
artifacts/m069-agents-k1-research/source-extracts/gpt-acc-occ3.txt
artifacts/m069-agents-k1-research/source-extracts/gpt-acc.txt
artifacts/m069-agents-k1-research/source-extracts/heading-index.txt
artifacts/m069-agents-k1-research/source-extracts/knowledge-graph-schema.txt
artifacts/m069-agents-k1-research/source-extracts/metric.txt
artifacts/m069-agents-k1-research/source-extracts/proofs-and-constructive-details-occ1.txt
artifacts/m069-agents-k1-research/source-extracts/proofs-and-constructive-details-occ2.txt
artifacts/m069-agents-k1-research/source-extracts/proofs-and-constructive-details-occ3.txt
```

</details>

### 02-m070-queue-foundation: M070 queue payload metadata foundation

- status: `default`
- pathspec_file: `artifacts/m074-scoped-commit-plan/pathspecs/02-m070-queue-foundation.txt`
- path_count: `5`

Stage command after explicit approval:

```bash
git add --pathspec-from-file=artifacts/m074-scoped-commit-plan/pathspecs/02-m070-queue-foundation.txt
```

Verification before commit:

- `uv run pytest tests/test_universal_kb_queue.py -q`
- `python3 -m py_compile src/arxiv_archive/universal_kb_queue.py`

Notes:
- Queue code/test/artifacts. Requires exact pathspecs because unrelated queue-era files exist in dirty tree.

<details><summary>Pathspecs</summary>

```text
artifacts/m070-queue-foundation/compatibility-report.md
artifacts/m070-queue-foundation/metadata-contract.md
artifacts/m070-queue-foundation/verification-summary.md
src/arxiv_archive/universal_kb_queue.py
tests/test_universal_kb_queue.py
```

</details>

### 03-m071-m073-benchmark-gates: M071-M073 executable benchmark gates

- status: `default`
- pathspec_file: `artifacts/m074-scoped-commit-plan/pathspecs/03-m071-m073-benchmark-gates.txt`
- path_count: `33`

Stage command after explicit approval:

```bash
git add --pathspec-from-file=artifacts/m074-scoped-commit-plan/pathspecs/03-m071-m073-benchmark-gates.txt
```

Verification before commit:

- `uv run pytest tests/test_m073_parser_evidence_benchmark.py tests/test_extraction_benchmark.py tests/test_universal_kb_queue.py -q`
- `python3 -m py_compile src/arxiv_archive/extraction_benchmark.py scripts/verify_m072_queue_benchmark_gate.py scripts/augment_m073_evidence_paths.py scripts/verify_m073_queue_evidence_gate.py`

Notes:
- Combined group avoids unsafe whole-file partial staging in tests/test_extraction_benchmark.py, which spans M071 and M072 changes.

<details><summary>Pathspecs</summary>

```text
artifacts/m071-extraction-benchmark/benchmark-gate-report.md
artifacts/m071-extraction-benchmark/fixture-schema.md
artifacts/m071-extraction-benchmark/fixtures/smoke-expected-metrics.json
artifacts/m071-extraction-benchmark/fixtures/smoke-gold.jsonl
artifacts/m071-extraction-benchmark/fixtures/smoke-predictions.jsonl
artifacts/m071-extraction-benchmark/verification-summary.md
artifacts/m072-reviewed-extraction-benchmark/closeout-report.md
artifacts/m072-reviewed-extraction-benchmark/evaluation-report.md
artifacts/m072-reviewed-extraction-benchmark/evaluation-results.json
artifacts/m072-reviewed-extraction-benchmark/fixtures/expected-metrics.json
artifacts/m072-reviewed-extraction-benchmark/fixtures/train-baseline-predictions.jsonl
artifacts/m072-reviewed-extraction-benchmark/fixtures/train-gold.jsonl
artifacts/m072-reviewed-extraction-benchmark/fixtures/validation-baseline-predictions.jsonl
artifacts/m072-reviewed-extraction-benchmark/fixtures/validation-gold.jsonl
artifacts/m072-reviewed-extraction-benchmark/label-plan.md
artifacts/m072-reviewed-extraction-benchmark/queue-metadata-verification.json
artifacts/m072-reviewed-extraction-benchmark/source-selection.md
artifacts/m072-reviewed-extraction-benchmark/verification-summary.md
artifacts/m073-parser-evidence-benchmark/closeout-report.md
artifacts/m073-parser-evidence-benchmark/evidence-path-contract.md
artifacts/m073-parser-evidence-benchmark/fixtures/evidence-coverage.json
artifacts/m073-parser-evidence-benchmark/fixtures/train-gold-evidence.jsonl
artifacts/m073-parser-evidence-benchmark/fixtures/validation-gold-evidence.jsonl
artifacts/m073-parser-evidence-benchmark/queue-evidence-verification.json
artifacts/m073-parser-evidence-benchmark/source-evidence-audit.json
artifacts/m073-parser-evidence-benchmark/source-evidence-audit.md
artifacts/m073-parser-evidence-benchmark/verification-summary.md
scripts/augment_m073_evidence_paths.py
scripts/verify_m072_queue_benchmark_gate.py
scripts/verify_m073_queue_evidence_gate.py
src/arxiv_archive/extraction_benchmark.py
tests/test_extraction_benchmark.py
tests/test_m073_parser_evidence_benchmark.py
```

</details>

### 04-optional-shared-gsd-registry: Optional shared GSD registry files

- status: `optional`
- pathspec_file: `artifacts/m074-scoped-commit-plan/pathspecs/04-optional-shared-gsd-registry.txt`
- path_count: `1`

Stage command after explicit approval:

```bash
git add --pathspec-from-file=artifacts/m074-scoped-commit-plan/pathspecs/04-optional-shared-gsd-registry.txt
```

Verification before commit:

- `test -f .gsd/ROADMAP.md`

Notes:
- Optional review group. Stage only if reviewer accepts generated GSD registry changes; do not include .gsd/gsd.db unless gsd_checkpoint_db is run immediately before staging.

<details><summary>Pathspecs</summary>

```text
.gsd/ROADMAP.md
```

</details>

### 99-optional-m074-plan: Optional M074 commit-plan artifacts

- status: `optional`
- pathspec_file: `artifacts/m074-scoped-commit-plan/pathspecs/99-optional-m074-plan.txt`
- path_count: `11`

Stage command after explicit approval:

```bash
git add --pathspec-from-file=artifacts/m074-scoped-commit-plan/pathspecs/99-optional-m074-plan.txt
```

Verification before commit:

- `test -f artifacts/m074-scoped-commit-plan/commit-plan.md`
- `test -f artifacts/m074-scoped-commit-plan/plan-verification.md`

Notes:
- Optional meta-planning artifacts; not part of M069-M073 product scope.

<details><summary>Pathspecs</summary>

```text
artifacts/m074-scoped-commit-plan/commit-plan.json
artifacts/m074-scoped-commit-plan/commit-plan.md
artifacts/m074-scoped-commit-plan/dirty-tree-inventory.json
artifacts/m074-scoped-commit-plan/dirty-tree-inventory.md
artifacts/m074-scoped-commit-plan/gitnexus-scope-assessment.md
artifacts/m074-scoped-commit-plan/pathspecs/01-m069-research.txt
artifacts/m074-scoped-commit-plan/pathspecs/02-m070-queue-foundation.txt
artifacts/m074-scoped-commit-plan/pathspecs/03-m071-m073-benchmark-gates.txt
artifacts/m074-scoped-commit-plan/pathspecs/04-optional-shared-gsd-registry.txt
artifacts/m074-scoped-commit-plan/pathspecs/99-optional-m074-plan.txt
artifacts/m074-scoped-commit-plan/plan-verification.md
```

</details>

## Excluded entries

All `unrelated_dirty` entries from `dirty-tree-inventory.json` are excluded from this plan unless the user explicitly broadens scope.

## Push policy

Pushing remote branches requires a separate explicit user confirmation after local commits exist.
