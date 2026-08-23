# M185 Wrapper Extraction Outcomes

## Summary

M185 S03-S06 produced two successful script-to-application extractions and two explicit no-move verifier decisions.

| Slice | Target | Outcome | Evidence |
|---|---|---|---|
| S03 | `scripts/audit_test_architecture.py` | Extracted to `src/research_graph/application/test_architecture_inventory.py` | `m185-test-architecture-extraction-result.md` |
| S04 | `scripts/audit_pipeline_scripts.py` | Extracted builder to `src/research_graph/application/pipeline_script_audit_inventory.py` | `m185-pipeline-audit-extraction-result.md` |
| S05 | `scripts/verify_m025_article_catalog.py` | No-move | `m185-m025-verifier-boundary-result.md` |
| S06 | `scripts/verify_m031_validation_remediation.py` | No-move | `m185-validation-evidence-helper-result.md` |

## Reusable extraction pattern

1. Run disambiguated GitNexus impact before editing.
2. Add or confirm wrapper baseline tests before movement.
3. Move concrete implementation into `src/research_graph/application/**` only when it does not require infrastructure or script-only dependencies.
4. Keep CLI printing and argument parsing in the script wrapper.
5. Preserve re-exported names when sibling scripts/tests import the old script surface.
6. Run focused tests, CLI smoke, architecture guard, ruff, pyrefly, and strict write-path drift.

## No-move rule refined

Verifier/security/path-safety helpers do not move one at a time. They require a cohesive package boundary that covers neighboring verifier flows and their safety contracts.
