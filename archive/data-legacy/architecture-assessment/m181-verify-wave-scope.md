# M181 Verify Wave Scope

## Decision

Move exactly 12 script-only records into two source-path categories:

```text
verify-m029-output=8
verify-m027-output=4
script-only: 122 -> 110
unknown=0
shared-state=0
total_delta=+0
```

## Scanner rule boundary

Rules must match exact `source_path` values only. The selected paths are:

```text
scripts/verify_m029_post_validation_remediation.py
scripts/verify_m029_unified_conversion_quality_boundary.py
scripts/verify_m029_unified_readiness.py
scripts/verify_m029_unified_source_acquisition.py
scripts/verify_m029_validation_remediation.py
scripts/verify_m027_mixed_source_catalog.py
scripts/verify_m027_source_acquisition_boundary.py
```

## Category names

```text
verify-m029-output
verify-m027-output
```

## Test contract

Focused tests must prove:

1. Each exact selected path maps to the expected category.
2. Unlisted future paths remain `script-only`, for example:
   - `scripts/verify_m029_future_unlisted.py`
   - `scripts/verify_m027_future_unlisted.py`
3. Generic targets like `path`, `fd`, `args.write_report`, and `args.report` are not category rules.
4. No broad `verify_m029`, `verify_m027`, or `verify_` prefix rule exists.

## Deferred

`build_m028` and `replay_m031` remain candidates for a later exact wave.
