# M186 Validation Evidence Builder Impact

## Verdict

**`build_evidence` has LOW upstream impact but high local coupling to M031-specific helpers and constants.**

## GitNexus impact

Exact target: `Function:scripts/verify_m031_validation_remediation.py:build_evidence`

```text
risk=LOW
direct callers=run, tests/test_m031_validation_remediation.py::_evidence
affected process=run
```

GitNexus context outgoing calls:

- `_contains_65_pass_signal`
- `_assessment_is_stale_failure`
- `_full_repo_collection_debt`
- `_requirement_rows`
- `_class_rows`
- `_flag_from_sources`

## Local dependency scan

`gsd_exec[bd70b3c3-c910-4596-b733-64674565e85f]` found:

```text
function_lines=337..468
local_calls=_assessment_is_stale_failure, _class_rows, _contains_65_pass_signal, _flag_from_sources, _full_repo_collection_debt, _requirement_rows
constants=CANONICAL_CLASSES, DEFAULT_AUDIT, DEFAULT_MATRIX, DEFAULT_REPLAY_CLOSEOUT, DEFAULT_REVIEW_EVENTS, DEFAULT_S02_ASSESSMENT, DEFAULT_S02_SUMMARY, DEFAULT_S02_UAT, DEFAULT_S05_CLOSEOUT, MILESTONE_ID, REQUIRED_FALSE_FLAGS, REQUIRED_REQUIREMENT_IDS, SCHEMA_VERSION, SELECTION_ID, SLICE_ID, TASK_ID
```

## Interpretation

The function is not unsafe by blast radius, but it is milestone-specific orchestration. Extracting it cleanly would require moving a cluster of M031 constants and helper row builders or inventing a configuration object for one caller. That would increase abstraction before a second implementation exists.
