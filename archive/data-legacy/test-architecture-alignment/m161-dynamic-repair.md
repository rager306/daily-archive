# M161 Dynamic Repair

Selected candidate: `tests/test_m045_project_trajectory.py`

## Why this candidate

S03 bounded recheck showed the smallest deterministic baseline failure: clean-project reverse ADR audit expected `rule_count == 8`, while current `scripts/check_project_trajectory.py` reports 10 reverse ADR audit rules. The test was quick, local, and did not involve protected artifact hashes, legacy package shims, or known timeout behavior.

## Change

- Replaced dynamic `importlib.util.spec_from_file_location(...)` loading with normal `from scripts import check_project_trajectory as traj` import.
- Updated stale expected `reverse_adr_audit_details.rule_count` from `8` to `10`.
- Moved `tests/test_m045_project_trajectory.py` from `dynamic_script_import` and `legacy_mixed` allowlists into `strict_script_wrapper`.

## Verification

- `uv run pytest tests/test_m045_project_trajectory.py` → `14 passed`.
- `uv run python scripts/audit_test_architecture.py --output-dir <tmp> --json` → `dynamic_script_import: 7`, `legacy-mixed: 22`, `script-wrapper: 67`.
- `uv run python scripts/verify_test_architecture.py --output-dir <tmp> --json` → `violations: 0`, `allowlisted_dynamic_script_import: 7`, `allowlisted_legacy_mixed: 22`, `strict_script_wrapper: 50`.

## Outcome

Promoted one remaining dynamic candidate out of dynamic and legacy debt. Remaining dynamic candidates: 7.
