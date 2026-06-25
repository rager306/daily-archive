# M169 CLI Write Resolution

## Verdict

**CLI per-paper unknown write paths are resolved.**

S06 routes `write_paper_artifacts(...)` per-paper JSON files through an atomic sibling temporary file and replacement. The write-path inventory no longer reports `paper_dir / 'paper.json'` or `paper_dir / 'scored.json'` as unknown.

## Impact analysis

GitNexus impact for current target:

```text
Function:src/research_graph/cli/__init__.py:write_paper_artifacts
risk=LOW
impactedCount=4
direct=1
affected_processes=0
```

The ambiguous first impact result included an archived candidate, but the current source target was disambiguated by UID and remained LOW risk.

## Changes

### `src/research_graph/cli/__init__.py`

- Added a CLI-local `_atomic_write_text(path, content)` helper using stdlib `tempfile.NamedTemporaryFile(...)` in the target directory and `Path.replace(...)`.
- Routed `write_paper_artifacts(...)` `paper.json` and `scored.json` writes through the helper.
- Left daily/session artifact writes unchanged because S06 scope is only the two unknown per-paper records.

### `tests/test_analysis.py`

- Added `test_s04_write_paper_artifacts_uses_atomic_replacement(...)`.
- The test monkeypatches `Path.replace` to verify both `paper.json` and `scored.json` are replaced atomically while preserving payload shape.

## Verification

| Check | Result | Evidence |
|---|---|---|
| Focused analysis tests | PASS: 36 passed | `gsd_exec[c1118969-015b-47d7-9dd0-b6ac9e040caa]` |
| Write-path inventory | PASS: unknown=1 | `gsd_exec[f765b761-030e-45c4-adec-cb5693760ad6]` |
| Scoped ruff | PASS | `gsd_exec[dbde4a2f-c62a-4304-b4c9-09f1616409e0]` |

## Inventory before and after

Before S06:

```text
unknown=3
unknown CLI records:
  src/research_graph/cli/__init__.py L355 paper_dir / 'paper.json'
  src/research_graph/cli/__init__.py L358 paper_dir / 'scored.json'
  src/research_graph/infrastructure/corpus/ingestion/fetchers.py L44 pdf_path
```

After S06:

```text
unknown=1
remaining unknown:
  src/research_graph/infrastructure/corpus/ingestion/fetchers.py L44 pdf_path
```

## Residual risk

The helper is intentionally local and minimal. It does not change higher-level concurrency semantics, but it prevents partial final JSON files for the per-paper cache artifacts under interrupted writes.
