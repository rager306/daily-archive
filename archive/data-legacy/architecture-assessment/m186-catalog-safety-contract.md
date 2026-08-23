# M186 Catalog Safety Contract

## Verdict

**M025 catalog safety movement is contract-gated and must remain fail-closed.**

## Helper contracts

| Helper | Contract | Direct callers from GitNexus | Movement rule |
|---|---|---|---|
| `normalize_posix_path` | Backslash paths normalize to POSIX without permitting traversal. | `safe_catalog_path`, `article_ref_from_path`, `_local_article_artifact_path` | May move only with exact behavior test. |
| `safe_catalog_path` | Reject absolute paths and any parent traversal, resolve under catalog root only. | `article_manifest_path` | Must preserve `ValueError` failure semantics. |
| `article_ref_from_path` | Accept only `article_catalog/.../article.json`; reject non-canonical source or manifest paths. | `validate_index`, `article_entry_from_record` | Must preserve canonical ID derivation. |
| `check_safety_flags` | Recursively reject forbidden true flags with stable dotted/list locations. | `validate_catalog`, `validate_index`, `validate_selection`, `article_entry_from_record` | Must stay fail-closed; no warning-only mode. |

## GitNexus exposure

All exact M025 helper impacts are LOW. However, the helpers participate in `proc_6_main` and `proc_83_main` catalog flows and reach `scripts/verify_m027_mixed_source_catalog.py:main` by depth 3. S05 movement therefore must verify both M025 and M027 focused catalog suites.

## Existing S02 tests that pin behavior

- `tests/test_m025_article_catalog_verifier.py::test_m186_catalog_path_helpers_are_fail_closed`
- `tests/test_m025_article_catalog_verifier.py::test_m186_safety_flags_reject_forbidden_true_values`

## S05 gate

Before moving any helper, S05 must:

1. re-run exact `gitnexus_impact` for edited symbols;
2. keep script-level wrapper functions compatible;
3. pass M025 and M027 focused catalog tests;
4. pass strict write-path drift;
5. preserve `script-only=4`, `unknown=0`, and `shared-state=0`.
