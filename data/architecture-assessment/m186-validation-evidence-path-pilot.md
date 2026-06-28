# M186 Validation Evidence Path Primitive Pilot

## Verdict

**PASS: M031 validation evidence path primitives moved behind an application boundary with script compatibility wrappers.**

## Changed boundary

New module:

- `src/research_graph/application/validation/evidence_paths.py`

It owns pure primitives:

- `json_path`
- `repo_relative_path`
- `safe_output_path`
- `ValidationEvidencePathError`

Compatibility wrappers remain in `scripts/verify_m031_validation_remediation.py`:

- `_json_path` delegates to `json_path`.
- `_repo_relative_path` delegates to `repo_relative_path` and maps `ValidationEvidencePathError` back to `M031ValidationRemediationError`.
- `_safe_output_path` delegates to `safe_output_path` with `OUTPUT_DIR`.

## GitNexus impact

Before editing, exact symbol impact was run for M031 helpers:

- `_json_path`: LOW, direct caller `_walk`, affected process `Run -> _json_path`.
- `_safe_output_path`: LOW, direct caller `run`.
- `_repo_relative_path`: LOW after UID disambiguation, direct callers `_safe_output_path`, `load_text`, `load_json`, `load_jsonl`, affected process `Run -> _repo_relative_path`.

After editing, `gitnexus_detect_changes(scope=all, repo=daily-archive)` reported MEDIUM risk with affected processes limited to:

- `Run -> _json_path`
- `Run -> _repo_relative_path`

## Verification

| Check | Result | Evidence |
|---|---|---|
| Application evidence path tests before wiring | PASS: 3 passed | `gsd_exec[9bd906b4-d383-4435-87a7-6d8a5b96f439]` |
| Application evidence path tests after wiring | PASS: 3 passed | `gsd_exec[07b59ed5-4e35-46bf-93f1-34045a71fc14]` |
| Focused M031 tests after wiring | PASS: 14 passed | `gsd_exec[a99a424e-7b5d-4baf-a0af-001bf5cca34c]` |
| Ruff | PASS | `gsd_exec[f3936720-9296-4b4c-bf13-a3ea0b914ef2]` |
| Pyrefly | PASS: 0 errors | `gsd_exec[42ab468a-8a15-4ffa-85e7-a11f69d220d5]` |
| Strict write-path drift | PASS: script-only=4, unknown=0, shared-state=0 | `gsd_exec[a6e30b48-fc5a-4406-8576-993dcc9449e1]` |

## Result

S03 successfully converts a no-move M185 probe into a small application boundary while preserving the script-level private helper API and M031 error type. This is safe input for S06 evidence builder boundary work.
