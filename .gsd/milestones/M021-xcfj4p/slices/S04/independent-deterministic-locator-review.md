## Review Summary

**Verdict: FLAG**

M021 S04 evidence is safe and directionally better than M020: deterministic route filtering reduced noise, tests pass, guards validate, and import/LadybugDB promotion remains blocked. However, I would not treat the implementation as fully reproducible or diagnostically complete yet. The main gaps are path-dependent span hashes and missing overlap diagnostics despite observed overlapping locator windows in the generated batch.

## Verification Performed

- `pytest tests/test_candidate_locators.py -q` -> **10 passed**
- `PYTHONPATH=src python3 ... validate_candidate_locator_artifact(...)` on `deterministic-locator-batch.json` -> **diagnostics `[]`**
- `find_forbidden_payload_keys(...)` on batch artifact -> **`[]`**
- Safety flags in batch artifact -> **all false**

## Findings

### Medium: Span hashes are path-dependent, weakening reproducibility

**File:** `src/arxiv_archive/candidate_locators.py`  
**Related artifact:** `.gsd/milestones/M021-xcfj4p/slices/S03/deterministic-locator-batch.json`

`span_hash` is derived from `source_path`, and the batch ledger stores local absolute source paths. The same paper content, source hash, and coordinates can produce different span hashes if regenerated under a different checkout path or machine-local paper cache path.

Suggested fix: hash stable provenance fields such as `source_id`, `source_hash`, coordinate space, offsets, and route name. Keep `source_path` as local provenance only, not stable span identity input.

### Medium: Overlapping locator windows are not diagnosed

**File:** `src/arxiv_archive/candidate_locators.py`  
**Design reference:** `.gsd/milestones/M021-xcfj4p/slices/S01/deterministic-locator-design.md`

The design calls out `overlapping_signal_window`, but the implementation builds and classifies each route independently. Coordinate-only inspection found overlapping span pairs in the generated batch, yet none were marked with `overlapping_signal_window`.

Suggested fix: after locators are generated for a source, run a redacted coordinate-only pass that compares `(source_id, char_start, char_end)` windows and appends `overlapping_signal_window` to affected locators/spans. Add a regression test.

### Low: Route filtering improves M020 but still does not show semantic readiness

M021 improves M020's noisy batch shape: locator count drops from 35 to 26 and ambiguous spans from 27 to 19. That is a reproducibility/noise-control improvement, not semantic validation. The batch still has 19 ambiguous spans, 7 retrieval-only locators, and 0 review-required locators.

## Risks

1. Reproducibility risk: path-derived hashes may drift across machines or cache locations.
2. Ambiguity diagnostics risk: overlap ambiguity exists but is not surfaced.
3. Reviewer burden risk: broad route signals still dominate.
4. Semantic overclaim risk: lower ambiguity than M020 is useful, but not KG correctness evidence.
5. Import safety risk: positive KG import and LadybugDB writes remain unsupported.

## Recommendation

Do not proceed to positive KG import or LadybugDB writes. Keep both blocked.

Recommended next work after fixing the two concrete implementation gaps:

1. Chunk/structure repair plus overlap diagnostics.
2. Reviewer packets after overlap and stable span identity are fixed.
3. Route-specific heuristics after or alongside structure repair.

Final decision before remediation: **FLAG** for reproducibility and diagnostic completeness gaps; **PASS** on safety/redaction/import-blocking boundaries.
