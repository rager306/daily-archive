# M185 Wrapper Contract Baseline

## Existing coverage

- `tests/test_pipeline_script_wrapper_contracts.py` checks canonical ingest, M056 ingest, parser replay, coverage report, and networkx probe script-wrapper boundaries.
- `tests/test_pipeline_script_audit.py` checks pipeline script audit inventory generation, schema, CLI write output, and category contracts.
- `tests/test_article_catalog_selection.py` checks the M184 article catalog wrapper pilot.
- `tests/test_test_architecture_guardrail.py` checks the enforcement guard around legacy mixed, dynamic imports, strict application/domain/workflow rules, and markdown rendering.

## Gap found

`scripts/audit_test_architecture.py` had no direct baseline test for its own output schema and `write_outputs` contract, even though S03 plans to inspect that helper.

## Baseline decision

Add one minimal test that calls `audit_test_architecture.build_inventory` and `write_outputs` against a temporary one-file test suite, without dynamic script import or subprocess execution.
