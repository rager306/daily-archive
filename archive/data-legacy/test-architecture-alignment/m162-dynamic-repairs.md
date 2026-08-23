# M162 Dynamic Repairs

## S01 M066

Baseline:

- Command: `uv run pytest tests/test_m066_s01.py -q`
- Result: failed quickly, `1 failed, 7 passed`.
- Failure: `test_scoring_matrix_has_top_3` expected `## Top-3 candidates` in `artifacts/m066-graphdb-reselection/scoring-matrix.md`.

Inspection:

- `scripts/m066_graphdb_full_benchmark.py` still renders a generated Top-3 section for fresh generated matrices.
- The persisted matrix artifact has newer M067-era semantics: `Total-score winner: Neo4j` and `M067 self-hosted winner: FalkorDB`, without the old Top-3 section.
- The dynamic debt in the test comes from `importlib.util.spec_from_file_location`; normal namespace-package import works with `from scripts import m066_graphdb_full_benchmark as benchmark`.

Decision:

- Do not rewrite the persisted matrix back to old M066 Top-3 wording.
- Repair the test to import normally and assert the current persisted matrix semantics.

Outcome:

- `tests/test_m066_s01.py` now imports `m066_graphdb_full_benchmark` normally from `scripts`.
- The stale Top-3 assertion now checks current persisted winner semantics: total-score Neo4j and M067 self-hosted FalkorDB.
- `uv run pytest tests/test_m066_s01.py` passed: `8 passed`.
- `uv run python scripts/verify_test_architecture.py --json` passed with `violations=0`.
- Counts after S01: `allowlisted_dynamic_script_import=6`, `allowlisted_legacy_mixed=21`, `strict_script_wrapper=51`.
- `uv run ruff check tests/test_m066_s01.py scripts/m066_graphdb_full_benchmark.py` passed.
- `uv run pyrefly check tests/test_m066_s01.py scripts/m066_graphdb_full_benchmark.py` passed with 0 errors.

## S02 M067

Baseline:

- Command: `uv run pytest tests/test_m067_s03.py -q`
- Result: failed quickly, `1 failed, 8 passed`.
- Failure: `test_m067_closeout_artifacts` expected `M045 on_track` in `M067-oqsavh-VALIDATION.md`.

Inspection:

- `M067-oqsavh-VALIDATION.md` is a compact validation artifact and does not include the older literal `M045 on_track` text.
- `M067-oqsavh-SUMMARY.md` records the intended closeout condition: `M045 stays on_track and M044 stays ok`.
- The dynamic import debt again came from `importlib.util.spec_from_file_location`; normal import works via `from scripts import m066_graphdb_full_benchmark as benchmark`.

Decision:

- Do not rewrite historical GSD validation artifacts.
- Repair the test to import normally and assert the M045/M044 closeout evidence from the summary, where it currently lives.

Outcome:

- `tests/test_m067_s03.py` now imports the benchmark script normally from `scripts`.
- The closeout artifact assertion now checks `M045 stays on_track` and `M044 stays ok` in the summary.
- `uv run pytest tests/test_m067_s03.py` passed: `9 passed`.
- `uv run python scripts/verify_test_architecture.py --json` passed with `violations=0`.
- Counts after S02: `allowlisted_dynamic_script_import=5`, `allowlisted_legacy_mixed=20`, `strict_script_wrapper=52`.
- `uv run ruff check tests/test_m066_s01.py tests/test_m067_s03.py scripts/m066_graphdb_full_benchmark.py` passed.
- `uv run pyrefly check tests/test_m066_s01.py tests/test_m067_s03.py scripts/m066_graphdb_full_benchmark.py` passed with 0 errors.
