# M084 Article Assets Package Move Summary

## Canonical path

Metadata-only article assets now live at:

```text
arxiv_archive.artifacts.assets
src/arxiv_archive/artifacts/assets.py
```

## Compatibility shim

The old import path remains available:

```text
arxiv_archive.article_assets
src/arxiv_archive/article_assets.py
```

The old module explicitly re-exports the article assets public contract surface: schema constants, allowed sets, diagnostic/record dataclasses, safety defaults, manifest builder/validator, summary helper, attach helper, and JSON helper.

## Path distinction

`arxiv_archive.artifacts.assets` is the article-artifact asset preservation boundary.

The existing `arxiv_archive.assets` package remains separate and was not moved or renamed in M084.

## Repo import updates

Updated imports to prefer the canonical path in:

- `src/arxiv_archive/artifacts/evidence_bridge.py`
- `tests/test_article_assets.py`
- `tests/test_property_article_assets.py`
- `tests/test_article_evidence_bridge.py`

Added a compatibility test proving the legacy module re-exports representative canonical objects.

## GitNexus blast radius

Before moving article assets code, GitNexus impact checks were run:

- `build_article_asset_manifest`: LOW risk; no upstream callers reported.
- `validate_article_asset_manifest`: LOW risk; direct impact flows through evidence bridge asset diagnostics.
- `attach_article_assets_summary`: LOW risk; no upstream callers reported.

## Verification

Fresh targeted tests:

```bash
uv run pytest tests/test_article_assets.py tests/test_property_article_assets.py tests/test_article_evidence_bridge.py tests/test_property_article_evidence_bridge.py -q
```

Result: **PASS** — 72 passed.

Fresh compile check:

```bash
python3 -m py_compile src/arxiv_archive/artifacts/assets.py src/arxiv_archive/article_assets.py src/arxiv_archive/artifacts/evidence_bridge.py
```

Result: **PASS**.

## Boundaries

- no live API calls
- no secrets collected or printed
- no graph writes
- no fact promotion
- no `arxiv_archive.assets` package move
- no article loader/link/retrieval moves
- no broad package restructure
- no shim removal

## Next candidate

Future artifact moves can target another scoped module after repeating the same guardrails: GitNexus impact, explicit shim, canonical repo imports, compatibility test, targeted tests, and GitNexus detect_changes.
