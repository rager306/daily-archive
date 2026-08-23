# M163 Code Layering and Guardrail Audit

## Verdict

**Strict onion verdict: CONCERN with live violations outside current guard scope.**

The existing onion guard passes for the currently enforced inner-layer invariant (`domain` and `application` do not import outward). However, a stricter hexagonal/onion reading finds live `infrastructure -> entry/workflows` and `workflows -> scripts` imports that are not currently checked by `scripts/verify_onion_layering.py`.

## Evidence

### Existing guardrail

Command:

```bash
uv run python scripts/verify_onion_layering.py --json
```

Result: exit `0`, `status=clear`, `violation_count=0`.

Coverage limit: the guard scans `src/research_graph/domain/` and `src/research_graph/application/`. It does **not** currently fail on infrastructure importing workflows/CLI or workflows importing scripts.

### Supplemental strict inventory

Read-only AST inventory over live `src/research_graph` excluding `archive/` found 11 stricter-boundary violations:

| Severity | File | Line | Import | Why it matters |
|---|---|---:|---|---|
| VIOLATION | `src/research_graph/infrastructure/repair/chunk_import_contract.py` | 15 | `research_graph.workflows.universal_kb.contracts` | Infrastructure depends on workflow contracts even though workflows are documented as entry/wiring. |
| VIOLATION | `src/research_graph/infrastructure/graph/ladybug_client.py` | 20 | `research_graph.cli` | Infrastructure type boundary depends on CLI entry-layer `DailyAnalysis`. |
| VIOLATION | `src/research_graph/infrastructure/corpus/ingestion/loader.py` | 25 | `research_graph.workflows.validation.logging` | Infrastructure imports workflow validation logging. |
| VIOLATION | `src/research_graph/infrastructure/corpus/ingestion/logging.py` | 12 | `research_graph.workflows.validation.logging` | Infrastructure logging shim depends on workflow layer. |
| VIOLATION | `src/research_graph/infrastructure/papers/artifacts/models.py` | 18 | `research_graph.workflows.universal_kb.contracts` | Infrastructure models depend on workflow contracts. |
| VIOLATION | `src/research_graph/infrastructure/papers/artifacts/batch_validation.py` | 27 | `research_graph.workflows.validation.batch_provenance` | Infrastructure artifact validation depends on workflow state. |
| VIOLATION | `src/research_graph/infrastructure/papers/artifacts/batch_validation.py` | 33 | `research_graph.workflows.validation.batch_state` | Infrastructure artifact validation depends on workflow state. |
| CONCERN | `src/research_graph/workflows/validation/batch_workflow.py` | 35 | `scripts.run_quality_gate` | Package workflow depends on loose script module. |
| CONCERN | `src/research_graph/workflows/universal_kb/smoke.py` | 16 | `scripts.audit_m036_real_corpus_smoke` | Package workflow depends on loose script module. |
| CONCERN | `src/research_graph/workflows/universal_kb/smoke.py` | 19 | `scripts.run_m036_real_corpus_no_write_smoke` | Package workflow depends on loose script module. |
| CONCERN | `src/research_graph/workflows/universal_kb/smoke.py` | 22 | `scripts.select_m036_real_corpus_smoke_batch` | Package workflow depends on loose script module. |

## What is compliant

- `domain/` is guarded and currently pure under the enforced rule.
- `application/` is guarded and currently does not import infrastructure, workflows, CLI, or scripts under the enforced rule.
- Infrastructure adapters generally point inward to application/domain use-case contracts.
- CLI acts as an entry/composition root and may import outward to infrastructure and workflows.
- Compatibility shim status is not itself a violation when the canonical implementation lives inward and shims only re-export.

## Strict interpretation

`doc/onion-layers.md` classifies `cli/`, `workflows/`, and `scripts/` as entry/wiring. Under a strict onion rule, dependencies should point inward:

```text
domain <- application <- infrastructure <- entry/wiring
```

Therefore:

- `domain` and `application` must not import outward. Current guard enforces this.
- `infrastructure` should not import `cli`, `workflows`, or `scripts`. Current guard does not enforce this.
- `workflows` should not import `scripts` if workflows are reusable package entrypoints. Current guard does not enforce this.

## Recommendations

1. **P1: Extend `verify_onion_layering.py` to cover infrastructure and workflows.** Add explicit rules: infrastructure must not import `research_graph.cli`, `research_graph.workflows`, or `scripts`; workflows must not import `scripts` except an allowlisted migration window.
2. **P1: Move workflow contracts used by infrastructure inward.** Candidates: `research_graph.workflows.universal_kb.contracts`, `validation.batch_state`, `validation.batch_provenance`, and `validation.logging` should be split so pure contracts live in `domain/` or application-local modules.
3. **P1: Move `DailyAnalysis` out of CLI.** `src/research_graph/infrastructure/graph/ladybug_client.py` should not type against `research_graph.cli.DailyAnalysis`; use a domain/application DTO or Protocol.
4. **P2: Convert `scripts` dependencies used by package workflows into package modules.** Leave scripts as thin wrappers around package code.

## Evidence command id

- `gsd_exec`: `b3ae0fe0-49d7-480f-a865-513b8f0c51d5`
