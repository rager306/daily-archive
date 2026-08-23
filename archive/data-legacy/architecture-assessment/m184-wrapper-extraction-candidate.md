# M184 Wrapper Extraction Candidate

## Selected candidate

**Pilot:** extract `scripts/verify_article_catalog.py::build_default_selection` into `src/research_graph/application/corpus/article_catalog_selection.py`.

## Why this candidate

- Small function with one direct caller: `scripts/verify_article_catalog.py::run`.
- GitNexus impact for `build_default_selection`: LOW, exact, 1 direct caller, no affected processes.
- No graph writes, no LLM calls, no cache/index lifecycle ownership change.
- The script already behaves like a wrapper around `verify_m025_article_catalog.main`; this extraction makes that boundary explicit.
- Tests can cover the new application helper and script no-arg behavior without network.

## Candidates skipped

- Replay flows: GitNexus showed active replay processes; defer runtime extraction until smaller pilot succeeds.
- Cache/manifest scripts: blocked until S11 lifecycle proof.
- Governance sync scripts: more file-format and docs coupling than needed for the first pilot.

## Expected change

- Add `build_current_catalog_index_selection(index_path: Path) -> dict[str, Any]` in application corpus layer.
- Update `scripts/verify_article_catalog.py` to call the helper.
- Add focused tests for the helper and no-arg script behavior.

## Verification plan

- `uv run pytest tests/test_article_catalog_selection.py tests/test_inventory_write_paths.py -q`
- `uv run ruff check src/research_graph/application/corpus/article_catalog_selection.py scripts/verify_article_catalog.py tests/test_article_catalog_selection.py`
- strict canonical inventory drift pass
- `gitnexus_detect_changes` scoped to daily-archive
