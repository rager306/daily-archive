# M165 Current Baseline: Strict Architecture Assessment

## Scope

M165 is an assessment-only milestone requested after M164. It evaluates the current repository, code, tests, architecture decisions, and architecture against strict hexagonal/onion architecture expectations, including future async and multithread readiness.

No source-code changes are in scope for M165.

## Live guardrail baseline

### Onion layering guard

Command:

```bash
uv run python scripts/verify_onion_layering.py --json
```

Evidence: `.gsd/exec/3ee90692-73e7-4ea1-a359-e587109dea9f.stdout`

Result:

```text
status=clear
violation_count=0
allowed_violation_count=0
layers=domain, application, infrastructure, workflows
```

Interpretation: current strict import guard is clean across all four guarded layers. This is a material improvement over M163, where strict-boundary issues existed outside the original guard scope.

### Test architecture guard

Command:

```bash
uv run python scripts/verify_test_architecture.py --json
```

Evidence: `.gsd/exec/bb4b20fd-0df9-4c8c-b028-c821dff93cbc.stdout`

Result:

```text
status=passed
violations=0
total_test_files=269
strict_application=6
strict_infrastructure=6
strict_script_wrapper=54
allowlisted_dynamic_script_import=3
allowlisted_legacy_mixed=18
```

Interpretation: test architecture guard is green, but still reports allowlisted categories that matter for strictness review.

## Recent architecture trajectory

### M163 prior state

M163 final verdict was **PARTIAL COMPLIANCE, NOT STRICT COMPLIANCE**. Main blockers were:

1. Infrastructure imported workflow contracts and CLI DTOs.
2. Workflows imported scripts.
3. Guardrails covered only the inner layers and missed strict outer-boundary issues.
4. Future async/multithread readiness was partial.

### M164 remediation state

M164 completed the requested P1 remediation:

1. `scripts/verify_onion_layering.py` now scans `domain`, `application`, `infrastructure`, and `workflows`.
2. Workflow contracts and CLI DTOs were moved inward to canonical homes.
3. Reusable script dependencies used by workflows were converted to package modules, with scripts as thin wrappers.
4. Analysis scoring fanout is bounded.
5. Queue state JSON writes use atomic replacement.
6. Adapter lifecycle ownership is documented and tested for `Embedder` injected-client ownership.

M164 closeout reports:

```text
strict-boundary findings: 11 -> 0
blocked imports: 0
allowed bounded debt imports: 0
```

## Current known context for strict assessment

### Strengths already evidenced

- Domain and application import direction is clean.
- Infrastructure no longer imports workflow/CLI contracts in the guarded set.
- Workflows no longer import scripts in the guarded set.
- Script wrappers exist for selected reusable script logic.
- Async-first policy is documented.
- Sync wrappers are supposed to fail inside active event loops.
- Representative concurrency controls exist for analysis scoring.
- Representative atomic write path exists for CLI queue state.
- Representative adapter lifecycle ownership test exists for `Embedder`.

### Immediate assessment cautions

- `doc/onion-layers.md` contains some stale wording under “What the guard enforces”: it still says the guard AST-scans only `domain/` and `application/`, even though the live guard now scans `domain`, `application`, `infrastructure`, and `workflows`.
- Test architecture guard is green but still has allowlisted dynamic and legacy-mixed categories.
- M164 intentionally left compatibility shims in workflow/CLI/script paths.
- M164 intentionally did not migrate every artifact write path to atomic/run-scoped writes.
- Adapter sharing remains unsupported unless future code adds explicit ownership, locking, and close-order contracts.

## Baseline verdict for downstream slices

The current repository is **much closer to strict compliance than M163**. Live import guardrails are clean, but strict compliance still requires deeper review of:

- compatibility shim risk,
- unguarded or semantically outer-bound dependencies not represented as import violations,
- test coverage for architecture invariants,
- documentation drift,
- concurrency and shared-state assumptions.
