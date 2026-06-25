# M165 Code Layering Assessment

## Verdict

**Code layering verdict: STRICT IMPORT COMPLIANCE, HIGH HEXAGONAL COMPLIANCE WITH OBSERVATIONS.**

The current codebase satisfies the strict import-direction matrix enforced by the live guard:

```text
domain -> no outward imports
application -> no infrastructure/workflow/cli/scripts imports
infrastructure -> no cli/workflows imports
workflows -> no scripts imports
src -> no scripts imports
blocked violations=0
allowed debt=0
```

Under a broader semantic hexagonal review, the code is close to strict but not perfectly frictionless because compatibility shims and historical script surfaces remain. They are acceptable as transitional boundaries, but they should not grow.

## Evidence

### Live guard evidence

- Onion guard evidence: `.gsd/exec/3ee90692-73e7-4ea1-a359-e587109dea9f.stdout`
- Scan evidence: `.gsd/exec/f6f146b3-383c-478f-95ea-7adc37d3e20a.stdout`

Targeted scan result:

```text
src_imports_scripts: 0
infra_imports_cli_or_workflows: 0
workflow_imports_scripts: 0
application_imports_infra_workflows_cli_scripts: 0
domain_imports_outward: 0
compat_shims: 11
script_wrappers: 4
protocol_files: 12
```

### Layer assessment

| Layer / boundary | Verdict | Evidence | Notes |
|---|---|---|---|
| Domain | Pass | Guard scans 11 domain files, no outward imports | `domain/universal_kb/contracts.py` is now an inward canonical home. |
| Application | Pass | Guard scans 18 application files, no infrastructure/workflow/CLI/scripts imports | `application/analysis.py` and `application/validation/*` are correct homes for shared use-case DTOs/contracts. |
| Infrastructure | Pass | Targeted scan found zero `infrastructure -> cli/workflows` imports | Infrastructure now imports inward contracts instead of entry/workflow contracts. |
| Workflows | Pass | Targeted scan found zero `workflows -> scripts` imports | Workflows use package modules; selected scripts are wrappers. |
| Scripts as process boundaries | Partial pass | 4 M164 scripts are thin wrappers | Many historical scripts remain prototype/process-boundary code; not all are package wrappers. This is acceptable only if workflows/src do not import them. |
| Ports/adapters | Mostly pass | 12 Protocol-bearing files across domain/application/infrastructure | Placement matches ADR taxonomy: cross-cutting domain ports, use-case application ports, infra-local protocols. |

## Compatibility shims

The scan found 11 files with shim/re-export language:

- `domain/semantic_chunks.py`
- `domain/statistical_context.py`
- `infrastructure/evaluation/scientific_extraction.py`
- `infrastructure/papers/indexing/navigation.py`
- `infrastructure/papers/semantic_chunks.py`
- `infrastructure/repair/candidate_locators.py`
- `workflows/import_boundary_rehearsal.py`
- `workflows/universal_kb/contracts.py`
- `workflows/validation/batch_provenance.py`
- `workflows/validation/batch_state.py`
- `workflows/validation/logging.py`

Assessment: these are not current import-direction violations. The workflow validation and Universal KB shims are deliberate compatibility surfaces after M164 moved canonical homes inward. They are acceptable if treated as deprecated facades and not as new dependency targets.

## Script boundary

M164 converted four workflow-consumed script implementations into package modules and left scripts as thin wrappers:

- `scripts/audit_m036_real_corpus_smoke.py`
- `scripts/run_m036_real_corpus_no_write_smoke.py`
- `scripts/run_quality_gate.py`
- `scripts/select_m036_real_corpus_smoke_batch.py`

Current code scan found no `src/research_graph` imports from `scripts` and no workflow imports from `scripts`. This closes the strict violation from M163.

## Gaps

### G1 — Compatibility shim lifecycle is not encoded as a removal/expiry contract

Shims are currently safe, but strict architecture benefits from an explicit policy: no new imports should target old workflow shim modules, and future cleanup should remove them once downstream imports migrate.

Risk: **medium**. Shims can silently become canonical again if tests import them indefinitely.

### G2 — Not all scripts are thin wrappers

The strict architecture rule only requires reusable logic used by packages/workflows to live in packages. Many historical scripts still contain standalone process-boundary logic. That is acceptable, but it means “scripts are wrappers” is not globally true.

Risk: **low to medium**. This becomes a violation only if package/workflow code imports those scripts.

### G3 — Import guard cannot prove runtime purity

AST import direction is necessary but not sufficient. It does not prove no global mutation, hidden I/O, shared mutable state, or adapter lifecycle leak exists inside an allowed layer.

Risk: **medium**, handled by S05 async/thread readiness.

## Backlog

| Priority | Item | Rationale |
|---|---|---|
| P1 | Add a shim lifecycle policy/test that prevents new imports to deprecated workflow shim modules | Keeps compatibility surfaces from regrowing into canonical APIs. |
| P2 | Continue converting reusable historical scripts only when a package/workflow dependency appears | Avoids speculative cleanup while preserving strict dependency direction. |
| P2 | Add semantic architecture probes for global mutation/shared state in allowed layers | Import guards alone do not prove concurrency safety. |

## Final code-layer conclusion

For code layering alone, the repository now meets strict onion import rules. For full hexagonal architecture, it is highly aligned but should be described as **strict on dependency direction, transitional on compatibility surfaces**.
