# M186 Catalog Verifier Boundary Pilot

## Verdict

**PASS: M025 catalog safety primitives moved behind an application boundary with script compatibility wrappers.**

## Changed boundary

New module:

- `src/research_graph/application/corpus/catalog_safety.py`

It owns pure primitives:

- `normalize_posix_path`
- `catalog_root`
- `safe_catalog_path`
- `article_ref_from_path`
- `safety_flag_errors`

Compatibility wrappers remain in `scripts/verify_m025_article_catalog.py`:

- `normalize_posix_path` delegates to the application primitive.
- `safe_catalog_path` delegates to the application primitive.
- `article_ref_from_path` delegates to the application primitive with `CATALOG_RECORD_DIR`.
- `check_safety_flags` extends the caller-provided error list with application `safety_flag_errors`.

## GitNexus impact

Before editing, exact impact was run for edited M025 symbols:

- `normalize_posix_path`: LOW, direct callers `safe_catalog_path`, `article_ref_from_path`, `_local_article_artifact_path`.
- `safe_catalog_path`: LOW, direct caller `article_manifest_path`.
- `article_ref_from_path`: LOW, direct callers `validate_index`, `article_entry_from_record`.
- `check_safety_flags`: LOW, direct callers `validate_catalog`, `validate_index`, `validate_selection`, `article_entry_from_record`.

M027 mixed-source catalog exposure was expected by depth 3 and verified with the scoped M027 subset.

Final `gitnexus_detect_changes(scope=all, repo=daily-archive)` reported MEDIUM risk because M025 and prior M031 wrappers are touched; this is expected and bounded by focused tests.

## Verification

| Check | Result | Evidence |
|---|---|---|
| Application catalog safety tests before wiring | PASS: 3 passed | `gsd_exec[8ce84667-5971-41ec-b91b-3a3f366b254e]` |
| Application plus M025 focused tests after final edit | PASS: 12 passed | `gsd_exec[68da559c-737f-4e9d-afcc-f57e03ef42e5]` |
| Scoped M027 catalog tests after final edit | PASS: 11 passed, 2 deselected | `gsd_exec[dd00be98-a947-4319-bc22-67e73e25dd7d]` |
| Ruff | PASS | `gsd_exec[0c217a67-7fff-4695-ac37-2a50eb1c34be]` |
| Pyrefly | PASS: 0 errors | `gsd_exec[b4285913-42ee-4642-8b9c-c90e52e593ef]` |
| Strict write-path drift | PASS: script-only=4, unknown=0, shared-state=0 | `gsd_exec[aedf1b59-4e85-4419-b0d8-fea9f63d538a]` |

## Result

S05 safely converts the M025 no-move probe into an application boundary while preserving fail-closed script behavior and catalog verification semantics. The known full-file M027 baseline failures remain outside this movement and are still assigned to S16.
