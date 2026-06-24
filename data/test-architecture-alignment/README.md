# Test Architecture Alignment

M128 inventories and gradually aligns tests with the project hexagonal/onion architecture.

## Test layer taxonomy

| Bucket | Purpose | Expected dependencies |
|---|---|---|
| `domain` | Pure domain invariants, value objects, and contracts | `research_graph.domain` plus stdlib/test helpers |
| `application` | Use-case behavior through ports and fake adapters | `research_graph.application`, `research_graph.domain` |
| `infrastructure` | Adapter behavior against real filesystem/parser/graph/telemetry boundaries | `research_graph.infrastructure` plus inward contracts |
| `script-wrapper` | Compatibility scripts, CLI args, subprocess behavior, artifact shape | `scripts/*` or subprocess wrappers |
| `acceptance` | Bounded end-to-end acceptance commands and generated artifacts | Multiple layers via public entrypoints |
| `legacy-mixed` | Historical tests with dynamic script imports or mixed surfaces | Explicitly inventoried before migration |
| `unknown` | No clear project-layer signal yet | Needs manual classification |

## Inventory command

```bash
uv run python scripts/audit_test_architecture.py --output-dir data/test-architecture-alignment
```

Outputs:

- `test-architecture-inventory.json`
- `test-architecture-inventory.md`
- `pilot-candidates.json`

The inventory is informational. It measures the suite and feeds the ratchetable guardrail.

## Guardrail command

```bash
uv run python scripts/verify_test_architecture.py --output-dir data/test-architecture-alignment
```

Outputs:

- `test-architecture-guardrail.json`
- `test-architecture-guardrail.md`

The guardrail is intentionally ratchetable:

- `legacy-mixed` and dynamic script-import tests are allowed only when listed in `test-architecture-allowlist.json`.
- Strict `application` tests must not import infrastructure, workflows, CLI, pipeline legacy modules, scripts, or dynamic script loaders.
- Strict `domain` tests must not import application, infrastructure, workflows, CLI, pipeline legacy modules, scripts, or dynamic script loaders.
- Strict `infrastructure` tests may import inward contracts but must not dynamic-import scripts.
- `script-wrapper` and `acceptance` tests are separated from use-case tests and should only validate wrapper or bounded end-to-end contracts.

## Pilot verification

The current pilot set is listed in `pilot-candidates.json` and summarized in `pilot-classification.json` / `.md`.

Run the pilot tests with:

```bash
uv run pytest $(python3 - <<'PY'
import json
from pathlib import Path
print(' '.join(c['path'] for c in json.loads(Path('data/test-architecture-alignment/pilot-candidates.json').read_text())))
PY
) tests/test_test_architecture_guardrail.py
```

## Migration guidance

When adding or moving tests:

1. Put pure domain behavior in domain tests with no application/infrastructure imports.
2. Put use-case behavior in application tests with fake ports/adapters, not real infrastructure.
3. Put filesystem/parser/graph/telemetry adapter behavior in infrastructure tests.
4. Put script argument compatibility and artifact-shape checks in script-wrapper tests.
5. Put bounded multi-step command proof in acceptance tests.
6. Do not add new dynamic script imports unless the file is intentionally classified and allowlisted as legacy-mixed.

Future milestones should shrink `legacy_mixed`, `dynamic_script_import`, and `unknown` counts rather than expanding the allowlist.

## Ratchet workflow

For a small allowlist-reduction pass:

1. Write a candidate artifact, such as `ratchet-candidates.json` / `.md`, with before counts, selected files, target bucket, and exclusions.
2. Run focused pytest on selected files before editing.
3. Replace dynamic script loading with normal imports or otherwise clarify the test boundary without changing assertions.
4. Regenerate inventory and update `test-architecture-allowlist.json` so migrated files leave `legacy_mixed` / `dynamic_script_import` and enter the appropriate strict set.
5. Run the guardrail and assert exact before/after count deltas.

M129 performed the first ratchet pass:

| Bucket | Before | After | Delta |
|---|---:|---:|---:|
| `dynamic_script_import` | 56 | 53 | -3 |
| `legacy_mixed` | 70 | 67 | -3 |
| `strict_script_wrapper` | 2 | 5 | +3 |

M130 performed the second ratchet pass:

| Bucket | Before | After | Delta |
|---|---:|---:|---:|
| `dynamic_script_import` | 53 | 52 | -1 |
| `legacy_mixed` | 67 | 66 | -1 |
| `strict_script_wrapper` | 5 | 6 | +1 |

M132 performed the third ratchet pass after M131 repaired the M061 stale fixture:

| Bucket | Before | After | Delta |
|---|---:|---:|---:|
| `dynamic_script_import` | 52 | 51 | -1 |
| `legacy_mixed` | 66 | 65 | -1 |
| `strict_script_wrapper` | 6 | 7 | +1 |

M132 also configured pyrefly with `search-path = ["."]` so normal repo-root `from scripts import ...` imports type-check without local `pyrefly: ignore [missing-import]` comments. Use local missing-import suppressions only for genuine optional dependencies or legacy import hacks that are not solved by the repo-root search path.

M133 performed the fourth ratchet pass and classifier cleanup:

| Bucket | Before | After | Delta |
|---|---:|---:|---:|
| `dynamic_script_import` | 51 | 50 | -1 |
| `legacy_mixed` | 65 | 64 | -1 |
| `strict_script_wrapper` | 7 | 8 | +1 |
| `unknown` | 81 | 77 | -4 |

M133 also categorized remaining `pyrefly: ignore [missing-import]` suppressions, removed three obsolete normal `scripts.*` import suppressions from baseline-green tests, and documented the remaining suppression debt by category. The unknown bucket reduction came from recognizing AST imports whose module is `scripts` or starts with `scripts.` as script-wrapper tests. `tests/test_m052_s02_e2e.py` was reclassified by import shape but remains baseline-red and must be repaired separately before runtime ratcheting.

M135 performed a strict script-wrapper promotion after M134 repaired M052:

| Bucket | Before | After | Delta |
|---|---:|---:|---:|
| `dynamic_script_import` | 50 | 50 | 0 |
| `legacy_mixed` | 64 | 64 | 0 |
| `strict_script_wrapper` | 8 | 9 | +1 |
| `unknown` | 77 | 77 | 0 |

M135 added `tests/test_m052_s02_e2e.py` to strict script-wrapper coverage after fresh pytest, ruff, pyrefly, and inventory checks passed. This was an allowlist-only promotion; no source code or dynamic/legacy debt counts changed.

M136 performed the fifth dynamic ratchet pass:

| Bucket | Before | After | Delta |
|---|---:|---:|---:|
| `dynamic_script_import` | 50 | 49 | -1 |
| `legacy_mixed` | 64 | 63 | -1 |
| `strict_script_wrapper` | 9 | 10 | +1 |
| `unknown` | 77 | 77 | 0 |

M136 moved `tests/test_article_baseline_recovery_replay.py` from importlib script loading to normal `scripts` imports after focused baseline pytest, ruff, pyrefly, and LOW GitNexus blast-radius checks passed.

M137 performed the sixth dynamic ratchet pass:

| Bucket | Before | After | Delta |
|---|---:|---:|---:|
| `dynamic_script_import` | 49 | 48 | -1 |
| `legacy_mixed` | 63 | 62 | -1 |
| `strict_script_wrapper` | 10 | 11 | +1 |
| `unknown` | 77 | 77 | 0 |

M137 moved `tests/test_article_preprocessing_replay_contract.py` from importlib script loading to normal `scripts` imports after focused baseline pytest, ruff, pyrefly, and LOW GitNexus blast-radius checks passed.

M138 performed the seventh dynamic ratchet pass:

| Bucket | Before | After | Delta |
|---|---:|---:|---:|
| `dynamic_script_import` | 48 | 47 | -1 |
| `legacy_mixed` | 62 | 61 | -1 |
| `strict_script_wrapper` | 11 | 12 | +1 |
| `unknown` | 77 | 77 | 0 |

M138 moved `tests/test_bounded_chunk_repair.py` from importlib script loading to normal `scripts` imports after focused baseline pytest, ruff, pyrefly, and LOW target-file GitNexus blast-radius checks passed.

M139 performed the eighth dynamic ratchet pass:

| Bucket | Before | After | Delta |
|---|---:|---:|---:|
| `dynamic_script_import` | 47 | 46 | -1 |
| `legacy_mixed` | 61 | 60 | -1 |
| `strict_script_wrapper` | 12 | 13 | +1 |
| `unknown` | 77 | 77 | 0 |

M139 moved `tests/test_codebase_memory_governance.py` from importlib script loading to a normal `scripts` import after focused baseline pytest, ruff, pyrefly, and LOW GitNexus blast-radius checks passed.

M140 performed the ninth dynamic ratchet pass after skipping a baseline-red candidate:

| Bucket | Before | After | Delta |
|---|---:|---:|---:|
| `dynamic_script_import` | 46 | 45 | -1 |
| `legacy_mixed` | 60 | 59 | -1 |
| `strict_script_wrapper` | 13 | 14 | +1 |
| `unknown` | 77 | 77 | 0 |

M140 skipped `tests/test_dspy_extraction_boundary.py` because focused baseline pytest failed before migration on stale static-scope path `src/research_graph.infrastructure.evaluation.dspy_extraction.py`. It then moved `tests/test_m024_validation_evidence_closure.py` from importlib script loading to a normal `scripts` import after focused baseline pytest, ruff, pyrefly, and LOW GitNexus blast-radius checks passed.

M141 repaired and ratcheted the DSPy boundary test into strict infrastructure coverage:

| Bucket | Before | After | Delta |
|---|---:|---:|---:|
| `dynamic_script_import` | 45 | 44 | -1 |
| `legacy_mixed` | 59 | 58 | -1 |
| `strict_infrastructure` | 5 | 6 | +1 |
| `strict_script_wrapper` | 14 | 14 | 0 |
| `unknown` | 77 | 77 | 0 |

M141 first repaired `tests/test_dspy_extraction_boundary.py` by correcting its stale static-scope source path, then replaced its dynamic fixture loader with a normal `tests.test_ladybug_scientific_kg` fixture import. Because the test imports infrastructure boundary code, it was promoted to `strict_infrastructure`, not `strict_script_wrapper`.

M142 promoted `tests/test_m025_boundary_replay_completion.py` into strict script-wrapper coverage:

| Bucket | Before | After | Delta |
|---|---:|---:|---:|
| `dynamic_script_import` | 44 | 43 | -1 |
| `legacy_mixed` | 58 | 57 | -1 |
| `strict_script_wrapper` | 14 | 15 | +1 |
| `strict_infrastructure` | 6 | 6 | 0 |
| `unknown` | 77 | 77 | 0 |

The test was baseline-green before migration and now imports `scripts.verify_m025_boundary_replay_completion` normally while preserving the existing helper aliases.

M143 promoted `tests/test_m025_evidence_replay.py` into strict script-wrapper coverage:

| Bucket | Before | After | Delta |
|---|---:|---:|---:|
| `dynamic_script_import` | 43 | 42 | -1 |
| `legacy_mixed` | 57 | 56 | -1 |
| `strict_script_wrapper` | 15 | 16 | +1 |
| `strict_infrastructure` | 6 | 6 | 0 |
| `unknown` | 77 | 77 | 0 |

The test was baseline-green before migration and now imports `scripts.verify_m025_evidence_boundaries` normally while preserving the existing module alias and helper aliases.

M144 promoted `tests/test_m025_requirement_scope_reconciliation.py` into strict script-wrapper coverage:

| Bucket | Before | After | Delta |
|---|---:|---:|---:|
| `dynamic_script_import` | 42 | 41 | -1 |
| `legacy_mixed` | 56 | 55 | -1 |
| `strict_script_wrapper` | 16 | 17 | +1 |
| `strict_infrastructure` | 6 | 6 | 0 |
| `unknown` | 77 | 77 | 0 |

The test was baseline-green before migration and now imports `scripts.verify_m025_requirement_scope_reconciliation` normally while preserving helper aliases.

M145 promoted `tests/test_m026_requirement_scope_reconciliation.py` into strict script-wrapper coverage:

| Bucket | Before | After | Delta |
|---|---:|---:|---:|
| `dynamic_script_import` | 41 | 40 | -1 |
| `legacy_mixed` | 55 | 54 | -1 |
| `strict_script_wrapper` | 17 | 18 | +1 |
| `strict_infrastructure` | 6 | 6 | 0 |
| `unknown` | 77 | 77 | 0 |

The test was baseline-green before migration and now imports `scripts.verify_m026_requirement_scope_reconciliation` normally while preserving helper aliases.

M146 promoted `tests/test_m027_current_pipeline_baseline.py` into strict script-wrapper coverage:

| Bucket | Before | After | Delta |
|---|---:|---:|---:|
| `dynamic_script_import` | 40 | 39 | -1 |
| `legacy_mixed` | 54 | 53 | -1 |
| `strict_script_wrapper` | 18 | 19 | +1 |
| `strict_infrastructure` | 6 | 6 | 0 |
| `unknown` | 77 | 77 | 0 |

The test was baseline-green before migration and now imports both M027 replay and verifier scripts normally while preserving `replay` and `verifier` aliases.

M147 promoted four M027 cohort tests into strict script-wrapper coverage:

| Bucket | Before | After | Delta |
|---|---:|---:|---:|
| `dynamic_script_import` | 39 | 35 | -4 |
| `legacy_mixed` | 53 | 49 | -4 |
| `strict_script_wrapper` | 19 | 23 | +4 |
| `strict_infrastructure` | 6 | 6 | 0 |
| `unknown` | 77 | 77 | 0 |

The batch was baseline-green before migration and now imports M027 replay, verification, synthesis, gate, and requirement-scope scripts normally while preserving existing aliases.

M148 promoted five M028 cohort tests into strict script-wrapper coverage:

| Bucket | Before | After | Delta |
|---|---:|---:|---:|
| `dynamic_script_import` | 35 | 30 | -5 |
| `legacy_mixed` | 49 | 44 | -5 |
| `strict_script_wrapper` | 23 | 28 | +5 |
| `strict_infrastructure` | 6 | 6 | 0 |
| `unknown` | 77 | 77 | 0 |

The batch was baseline-green before migration and now imports M028 build/verify scripts normally. Two verifier compatibility aliases remain in `sys.modules` for bare build-script imports; no dynamic module loading remains.

M149 promoted three validation remediation tests into strict script-wrapper coverage:

| Bucket | Before | After | Delta |
|---|---:|---:|---:|
| `dynamic_script_import` | 30 | 27 | -3 |
| `legacy_mixed` | 44 | 41 | -3 |
| `strict_script_wrapper` | 28 | 31 | +3 |
| `strict_infrastructure` | 6 | 6 | 0 |
| `unknown` | 77 | 77 | 0 |

The batch was baseline-green before migration and now imports validation remediation verifier scripts normally while preserving existing test-level aliases.

M150 promoted three M056 wave analyzer tests into strict script-wrapper coverage:

| Bucket | Before | After | Delta |
|---|---:|---:|---:|
| `dynamic_script_import` | 27 | 24 | -3 |
| `legacy_mixed` | 41 | 38 | -3 |
| `strict_script_wrapper` | 31 | 34 | +3 |
| `strict_infrastructure` | 6 | 6 | 0 |
| `unknown` | 77 | 77 | 0 |

The batch was baseline-green before migration and now imports M056 wave analyzer scripts normally while preserving the `load_analyzer()` helper API.

M151 promoted three selected M060 tests into strict script-wrapper coverage:

| Bucket | Before | After | Delta |
|---|---:|---:|---:|
| `dynamic_script_import` | 24 | 21 | -3 |
| `legacy_mixed` | 38 | 35 | -3 |
| `strict_script_wrapper` | 34 | 37 | +3 |
| `strict_infrastructure` | 6 | 6 | 0 |
| `unknown` | 77 | 77 | 0 |

`tests/test_m060g_s02.py` was excluded because baseline focused pytest timed out at 300 seconds. The promoted files now import M060 scripts normally; M060C source-text assertions retain explicit source path constants.

M152 promoted five M041-M043 connectivity-sidecar tests into strict script-wrapper coverage:

| Bucket | Before | After | Delta |
|---|---:|---:|---:|
| `dynamic_script_import` | 21 | 16 | -5 |
| `legacy_mixed` | 35 | 30 | -5 |
| `strict_script_wrapper` | 37 | 42 | +5 |
| `strict_infrastructure` | 6 | 6 | 0 |
| `unknown` | 77 | 77 | 0 |

The batch was baseline-green before migration and now imports connectivity-sidecar scripts normally. M041 keeps script-directory path setup for a bare sibling import in the script under test; no dynamic module loading remains.

M153 promoted two pipeline audit tests into strict script-wrapper coverage:

| Bucket | Before | After | Delta |
|---|---:|---:|---:|
| `dynamic_script_import` | 16 | 14 | -2 |
| `legacy_mixed` | 30 | 28 | -2 |
| `strict_script_wrapper` | 42 | 44 | +2 |
| `strict_infrastructure` | 6 | 6 | 0 |
| `unknown` | 77 | 77 | 0 |

The batch was baseline-green before migration and now imports `scripts.audit_pipeline_scripts` normally. The helper `_load_audit_module()` remains as a compatibility shim returning the imported module; `AUDIT_SCRIPT_PATH` remains only as a subprocess CLI path.

If a candidate fails focused baseline pytest before migration, exclude it from the ratchet batch and record the reason in the candidate artifact. M130 rejected `tests/test_m061_s02.py` this way because it had a stale fixture SHA before any import cleanup; M131 repaired that stale fixture before M132 ratcheted the file.
