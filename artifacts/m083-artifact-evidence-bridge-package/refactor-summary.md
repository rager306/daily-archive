# M083 Artifact Evidence Bridge Package Move Summary

## Canonical path

Article evidence bridge now lives at:

```text
arxiv_archive.artifacts.evidence_bridge
src/arxiv_archive/artifacts/evidence_bridge.py
```

## Compatibility shim

The old import path remains available:

```text
arxiv_archive.article_evidence_bridge
src/arxiv_archive/article_evidence_bridge.py
```

The old module explicitly re-exports the evidence bridge public contract surface: schema constants, allowed status/use sets, dataclasses, bundle builders, validators, redaction helpers, summary attachers, and run-summary helpers.

## Repo import updates

Updated targeted tests to prefer the canonical path in:

- `tests/test_article_evidence_bridge.py`
- `tests/test_property_article_evidence_bridge.py`

Added a compatibility test proving the legacy module re-exports representative canonical objects.

## GitNexus blast radius

Before moving bridge code, GitNexus impact checks were run:

- `build_article_evidence_bundle`: LOW risk; no upstream callers reported.
- `build_article_evidence_run_summary`: LOW risk; direct caller is internal `build_article_evidence_run_summary_from_load_events`.
- `validate_article_evidence_bundle`: LOW risk; no upstream callers reported.

Direct import search found old-path imports only in bridge tests before the move.

## Verification

Fresh targeted tests:

```bash
uv run pytest tests/test_article_evidence_bridge.py tests/test_property_article_evidence_bridge.py -q
```

Result: **PASS** — 47 passed.

Fresh compile check:

```bash
python3 -m py_compile src/arxiv_archive/artifacts/evidence_bridge.py src/arxiv_archive/article_evidence_bridge.py
```

Result: **PASS**.

## Boundaries

- no live API calls
- no secrets collected or printed
- no graph writes
- no fact promotion
- no loader/assets/link/retrieval module moves
- no broad package restructure
- no shim removal

## Next candidate

Future `arxiv_archive.artifacts` moves can target another leaf module after repeating the same guardrails: GitNexus impact, explicit shim, canonical repo imports, compatibility test, targeted tests, and GitNexus detect_changes.
