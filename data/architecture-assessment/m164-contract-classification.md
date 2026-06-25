# M164 Contract and Boundary Classification

## Purpose

Classify the 11 strict-boundary debt findings before moving code. This prevents speculative abstractions and gives S05-S07 exact source and target paths.

## Classification summary

| Debt group | Count | Classification | Target layer | Target path | Planned slice |
|---|---:|---|---|---|---|
| Infrastructure imports validation logging | 2 | reusable validation event sink | infrastructure | `research_graph.infrastructure.validation.logging` | S05 |
| Infrastructure imports validation batch state | 1 | pure validation DTO/contract | application | `research_graph.application.validation.batch_state` | S05 |
| Infrastructure imports validation provenance | 1 | reusable provenance utility | application | `research_graph.application.validation.batch_provenance` | S05 |
| Infrastructure imports Universal KB contracts | 2 | fail-closed pure DTO/contract | domain | `research_graph.domain.universal_kb.contracts` | S05 |
| Infrastructure imports CLI `DailyAnalysis` | 1 | analysis result DTO | application | `research_graph.application.analysis` | S06 |
| Workflows import M036 smoke scripts | 3 | reusable smoke selection/runner/audit functions | application or infrastructure package modules | `research_graph.application.universal_kb.smoke` plus infra helpers if needed | S07 |
| Workflow imports quality gate script | 1 | reusable diagnostic quality runner | infrastructure quality module | `research_graph.infrastructure.quality.gate` | S07 |

## Finding-by-finding movement map

| Rule | Current importer | Current imported | Classification | Target import after remediation | Rationale |
|---|---|---|---|---|---|
| infra-imports-workflow-entrypoint | `research_graph.infrastructure.corpus.ingestion.loader` | `research_graph.workflows.validation.logging` | reusable validation event sink | `research_graph.infrastructure.validation.logging` | This is concrete logging/file I/O support used by infrastructure; it should not live in workflow entry/wiring. |
| infra-imports-workflow-entrypoint | `research_graph.infrastructure.corpus.ingestion.logging` | `research_graph.workflows.validation.logging` | reusable validation event sink | `research_graph.infrastructure.validation.logging` | Same logging sink dependency; moving one module removes both imports. |
| infra-imports-cli-entrypoint | `research_graph.infrastructure.graph.ladybug_client` | `research_graph.cli` | analysis DTO type-only dependency | `research_graph.application.analysis` | Infrastructure must not import CLI; `DailyAnalysis` describes application output, not entrypoint behavior. |
| infra-imports-workflow-entrypoint | `research_graph.infrastructure.papers.artifacts.batch_validation` | `research_graph.workflows.validation.batch_provenance` | reusable provenance utility | `research_graph.application.validation.batch_provenance` | Provenance is deterministic application-support logic, not workflow orchestration. |
| infra-imports-workflow-entrypoint | `research_graph.infrastructure.papers.artifacts.batch_validation` | `research_graph.workflows.validation.batch_state` | validation DTO/contract | `research_graph.application.validation.batch_state` | Batch state models validation use-case state; no I/O driver required. |
| infra-imports-workflow-entrypoint | `research_graph.infrastructure.papers.artifacts.models` | `research_graph.workflows.universal_kb.contracts` | pure fail-closed contract | `research_graph.domain.universal_kb.contracts` | These dataclasses carry core graph/import safety invariants; they are cross-cutting domain contracts. |
| infra-imports-workflow-entrypoint | `research_graph.infrastructure.repair.chunk_import_contract` | `research_graph.workflows.universal_kb.contracts` | pure fail-closed contract | `research_graph.domain.universal_kb.contracts` | Same Universal KB contract dependency. |
| workflow-imports-script-wrapper | `research_graph.workflows.universal_kb.smoke` | `scripts.audit_m036_real_corpus_smoke` | reusable smoke audit implementation | `research_graph.application.universal_kb.smoke_audit` or package-local equivalent | Workflows may orchestrate reusable package code, not import script wrappers. |
| workflow-imports-script-wrapper | `research_graph.workflows.universal_kb.smoke` | `scripts.run_m036_real_corpus_no_write_smoke` | reusable smoke runner implementation | `research_graph.application.universal_kb.smoke_runner` or package-local equivalent | Runner logic is reusable package behavior; script should delegate. |
| workflow-imports-script-wrapper | `research_graph.workflows.universal_kb.smoke` | `scripts.select_m036_real_corpus_smoke_batch` | reusable deterministic selection logic | `research_graph.application.universal_kb.smoke_selection` or package-local equivalent | Deterministic selection can be package code consumed by workflow and script. |
| workflow-imports-script-wrapper | `research_graph.workflows.validation.batch_workflow` | `scripts.run_quality_gate` | reusable diagnostic quality runner | `research_graph.infrastructure.quality.gate` | Quality diagnostic writes artifacts and calls concrete quality infrastructure; reusable implementation belongs in infrastructure, script delegates. |

## Policy decisions for S05-S07

1. **Do not add new Ports for these moves.** The debts are import-ownership problems, not polymorphism problems.
2. **Move pure contracts inward, not orchestration.** Dataclasses and safety contracts move to domain/application; workflows remain runtime orchestration.
3. **Keep scripts thin.** Scripts may import package modules and parse args; package modules must not import scripts.
4. **Prefer compatibility shims only when needed.** If many imports still reference old workflow paths, keep a temporary re-export shim with no behavior. Ratchet guard allowlists down as direct imports are updated.
5. **Preserve fail-closed safety invariants.** Universal KB safety flags and validation safety defaults must remain false-by-default.

## Exact next moves

### S05

- Create `src/research_graph/infrastructure/validation/logging.py` from `workflows.validation.logging` or move the canonical implementation there with a workflow shim.
- Move or shim `workflows.validation.batch_state` to `application.validation.batch_state`.
- Move or shim `workflows.validation.batch_provenance` to `application.validation.batch_provenance`.
- Move or shim `workflows.universal_kb.contracts` to `domain.universal_kb.contracts`.
- Update direct infrastructure imports and reduce `STRICT_DEBT_ALLOWLIST`.

### S06

- Create `src/research_graph/application/analysis.py` for `DailyAnalysis` and related status type if needed.
- Update CLI and `ladybug_client` type-checking import.
- Remove infra-to-CLI allowlist entry.

### S07

- Extract M036 smoke script reusable functions into package modules consumed by workflows and scripts.
- Extract quality gate reusable implementation from script into infrastructure quality module consumed by workflow and script.
- Remove workflow-to-script allowlist entries.

## Non-goals

- No broad package rename.
- No new abstraction layer.
- No behavior rewrite of validation, Universal KB smoke, or quality diagnostics.
- No async/concurrency changes in this classification slice.
