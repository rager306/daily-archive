# M185 Architecture Horizon Two State

## Verdict

**Architecture horizon two is validation-ready.**

## Extracted seams

- `scripts/audit_test_architecture.py` is now a thin CLI wrapper over `src/research_graph/application/test_architecture_inventory.py`.
- `scripts/audit_pipeline_scripts.py` is now a thin CLI wrapper over `src/research_graph/application/pipeline_script_audit_inventory.py`; CLI printing remains script-local.

## No-move verifier probes

- `scripts/verify_m025_article_catalog.py`: no-move; safety-sensitive catalog verifier helpers need cohesive verifier package design.
- `scripts/verify_m031_validation_remediation.py`: no-move; validation evidence/path helpers need cohesive validation evidence boundary.

## Manifest/cache residual state

```text
total_records=341
script-only=4
unknown=0
shared-state=0
```

The four `script-only` residuals remain intentional no-move records:

- `scripts/benchmark_m055_corpus_manifest.py`
- `scripts/build_m055deep_corpus_manifest_20.py`
- `scripts/m058_build_graph_manifest.py`
- `scripts/m059_build_manifest.py`

## Guardrail state

- Focused integrated tests: 102 passed.
- Strict write-path drift: pass.
- Test architecture guard: violations=0.
- Onion guard: violation_count=0.
- Ruff: pass.
- Pyrefly: 0 errors.
- Pre-commit: pass.
- GitNexus detect_changes: low risk, affected_processes=[].

## Next horizon

1. Design a cohesive verifier package before moving M025/M031 helpers.
2. Design a manifest lifecycle boundary before reducing `script-only` below 4.
3. Keep GSD files local and ignored; do not commit `.gsd/*`.
