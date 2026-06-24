# M134 M052 Diagnosis

Schema: `daily-archive-m134-m052-diagnosis.v1`

## Observed failure

- Command: `uv run pytest tests/test_m052_s02_e2e.py`
- Result: `7 failed`
- Exception: `TypeError: Claim.__init__() got an unexpected keyword argument id`
- Location: `scripts/m052_rlm_e2e.py:91 in build_fixture_patch`

## Root cause

scripts/m052_rlm_e2e.py constructs typed extraction fixture objects with legacy flat constructor fields (`id`, `paper_id`, `label`, relation `source_id`/`target_id`, patch `paper_id`). Current `research_graph.domain.schema` dataclasses use typed fields (`claim_id`, `source_id`, `entity_id`, `canonical_name`, `relation_id`, `from_entity_id`, `to_entity_id`) and UPPERCASE typed relation values.

## Minimal repair

Update only the M052 fixture construction in scripts/m052_rlm_e2e.py to use the current typed field names and canonical UPPERCASE relation type, mirroring tests/test_ladybug_scientific_kg.py. Remove stale pyrefly/ty ignores that existed only for the legacy mismatched constructor fields.

## Out of scope

Do not change domain schema or test architecture allowlists in M134; M052 ratchet remains future work after baseline repair.
