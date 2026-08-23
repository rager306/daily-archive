# M186 Verifier Contract Baseline

## Verdict

**PASS: verifier movement is now contract-gated before extraction.**

## Contracts added

### M031 validation remediation

- `_json_path` formatting is pinned for object keys and array indexes.
- `_repo_relative_path` rejects empty, whitespace-padded, parent traversal, absolute, URL, and missing paths.
- `_safe_output_path` rejects outputs outside `data/validation-remediation`.
- `build_evidence` remains metadata-only and fail-closed for graph/import/model/write/secrets flags.

### M025 article catalog verifier

- `normalize_posix_path` preserves canonical POSIX article paths from backslash input.
- `safe_catalog_path` rejects absolute and parent traversal paths.
- `article_ref_from_path` rejects non-canonical article manifest paths.
- `check_safety_flags` accepts forbidden flags only when false and reports true values with stable locations.

## Evidence

| Check | Result | Evidence |
|---|---|---|
| Focused verifier contract tests | PASS: 23 passed | `gsd_exec[e0a66fb4-e1e3-4270-9758-5f787555c1db]` |
| Ruff | PASS | `gsd_exec[6348fdc9-01ea-4b2d-98a3-f823ebdd405f]` |
| Pyrefly | PASS: 0 errors | `gsd_exec[d5e84b76-76f6-4f72-9c61-b7febcdb43fd]` |
| Strict write-path drift | PASS: script-only=4, unknown=0, shared-state=0 | `gsd_exec[f4d1f603-baee-4ec3-bc50-687f18ab6438]` |

## Impact context

See `data/architecture-assessment/m186-verifier-impact-map.md`. All target helpers were LOW risk, but M025 helpers touch M027 mixed-source catalog flows and remain fail-closed.

## Source movement

No source movement occurred in S02. S03 to S06 may now use these contracts as the extraction gate.
