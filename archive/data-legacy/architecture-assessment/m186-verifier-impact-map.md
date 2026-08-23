# M186 Verifier Impact Map

## Verdict

**All S02 verifier contract targets are LOW risk, but M025 helpers participate in M027 mixed-source catalog flow and must remain fail-closed.**

## M031 validation evidence helpers

| Symbol | File | Risk | Direct caller or process | Notes |
|---|---|---|---|---|
| `_json_path` | `scripts/verify_m031_validation_remediation.py` | LOW | `_walk`, then `_flag_from_sources`, `_validate_metadata_safety`, `build_evidence`, `validate_evidence`, `run` | Ambiguous by name; disambiguated to M031 UID. |
| `_safe_output_path` | `scripts/verify_m031_validation_remediation.py` | LOW | `run` | Impacts focused M031 CLI output tests through `main`. |
| `build_evidence` | `scripts/verify_m031_validation_remediation.py` | LOW | `run`, test helper `_evidence` | Directly used by focused tests; contract should pin metadata-only flags and diagnostics. |

## M025 catalog verifier helpers

| Symbol | File | Risk | Direct caller or process | Notes |
|---|---|---|---|---|
| `normalize_posix_path` | `scripts/verify_m025_article_catalog.py` | LOW | `safe_catalog_path`, `article_ref_from_path`, `_local_article_artifact_path` | Participates in `proc_6_main`; path normalization must stay traversal-safe. |
| `safe_catalog_path` | `scripts/verify_m025_article_catalog.py` | LOW | `article_manifest_path` | Ambiguous by name; disambiguated to M025 UID. |
| `article_ref_from_path` | `scripts/verify_m025_article_catalog.py` | LOW | `validate_index`, `article_entry_from_record` | Also reaches `update_index_if_exists` indirectly; canonical path format must remain strict. |
| `check_safety_flags` | `scripts/verify_m025_article_catalog.py` | LOW | `validate_catalog`, `validate_index`, `validate_selection`, `article_entry_from_record` | Directly fail-closed; contract tests must preserve forbidden true flag rejection. |

## Risk constraints

- No HIGH or CRITICAL blast radius was found.
- S02 adds tests only; source movement waits for S03 to S06.
- M025 helper changes in later slices must re-run focused M025 tests and consider M027 mixed-source catalog process exposure.
