# M164 Strict Boundary Baseline

## Purpose

Baseline strict onion violations before changing guardrails or moving contracts. This extends M163 from assessment into reproducible remediation evidence.

## Current guard result

- Command: `uv run python scripts/verify_onion_layering.py --json`
- Exit code: `0`
- Current guard violation count: `0`
- Interpretation: current guard result is inner-layer proof only; the strict boundary findings below are outside the current guard scope unless stated otherwise.

## Strict-boundary inventory

- Total strict findings: `11`
- `infra-imports-cli-entrypoint`: `1`
- `infra-imports-workflow-entrypoint`: `6`
- `workflow-imports-script-wrapper`: `4`

| Rule | Severity | File | Line | Importer | Imported | Covered by current guard |
|---|---|---|---:|---|---|---:|
| `infra-imports-workflow-entrypoint` | P1 | `src/research_graph/infrastructure/corpus/ingestion/loader.py` | 25 | `research_graph.infrastructure.corpus.ingestion.loader` | `research_graph.workflows.validation.logging` | no |
| `infra-imports-workflow-entrypoint` | P1 | `src/research_graph/infrastructure/corpus/ingestion/logging.py` | 12 | `research_graph.infrastructure.corpus.ingestion.logging` | `research_graph.workflows.validation.logging` | no |
| `infra-imports-cli-entrypoint` | P1 | `src/research_graph/infrastructure/graph/ladybug_client.py` | 20 | `research_graph.infrastructure.graph.ladybug_client` | `research_graph.cli` | no |
| `infra-imports-workflow-entrypoint` | P1 | `src/research_graph/infrastructure/papers/artifacts/batch_validation.py` | 27 | `research_graph.infrastructure.papers.artifacts.batch_validation` | `research_graph.workflows.validation.batch_provenance` | no |
| `infra-imports-workflow-entrypoint` | P1 | `src/research_graph/infrastructure/papers/artifacts/batch_validation.py` | 33 | `research_graph.infrastructure.papers.artifacts.batch_validation` | `research_graph.workflows.validation.batch_state` | no |
| `infra-imports-workflow-entrypoint` | P1 | `src/research_graph/infrastructure/papers/artifacts/models.py` | 18 | `research_graph.infrastructure.papers.artifacts.models` | `research_graph.workflows.universal_kb.contracts` | no |
| `infra-imports-workflow-entrypoint` | P1 | `src/research_graph/infrastructure/repair/chunk_import_contract.py` | 15 | `research_graph.infrastructure.repair.chunk_import_contract` | `research_graph.workflows.universal_kb.contracts` | no |
| `workflow-imports-script-wrapper` | P1 | `src/research_graph/workflows/universal_kb/smoke.py` | 16 | `research_graph.workflows.universal_kb.smoke` | `scripts.audit_m036_real_corpus_smoke` | no |
| `workflow-imports-script-wrapper` | P1 | `src/research_graph/workflows/universal_kb/smoke.py` | 19 | `research_graph.workflows.universal_kb.smoke` | `scripts.run_m036_real_corpus_no_write_smoke` | no |
| `workflow-imports-script-wrapper` | P1 | `src/research_graph/workflows/universal_kb/smoke.py` | 22 | `research_graph.workflows.universal_kb.smoke` | `scripts.select_m036_real_corpus_smoke_batch` | no |
| `workflow-imports-script-wrapper` | P1 | `src/research_graph/workflows/validation/batch_workflow.py` | 35 | `research_graph.workflows.validation.batch_workflow` | `scripts.run_quality_gate` | no |

## Target policy for S02 and S03

- Infrastructure must not import CLI entrypoint modules.
- Infrastructure must not import workflow orchestration modules.
- Infrastructure must not import scripts as reusable package dependencies.
- Workflows must not import `scripts.*`; reusable logic belongs in package modules, and scripts stay thin process-boundary wrappers.
- Existing debt may be temporarily allowlisted only with explicit rule IDs and counts, never hidden as a green strict pass.

## Current guard coverage gap

- `domain` and `application` direction checks are covered today.
- `infrastructure -> cli/workflows/scripts` is not covered today.
- `workflows -> scripts` is not covered today.
- S02 should add infrastructure import enforcement.
- S03 should add workflow/script boundary enforcement.

## Enforcement staging

1. Add JSON-visible rule IDs and debt counts first.
2. Add tests for synthetic pass/fail cases.
3. Either fail immediately for zero-debt categories or require explicit bounded allowlist entries for existing debt.
4. Remove allowlist entries as S05-S07 move contracts and script dependencies inward.
