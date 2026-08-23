# M169 Write Path Inventory Closeout

## Verdict

**Write-path unknown count is zero.**

M169 resolved all three remaining unknown write paths from the M168 inventory by adding bounded atomic writes for stable cache/artifact outputs rather than hiding them with scanner-only classification.

## Final inventory

Artifacts:

```text
data/architecture-assessment/m169-write-path-inventory.json
data/architecture-assessment/m169-write-path-inventory.md
```

Final counts:

```text
total_records=339
script-only=263
caller-owned=38
run-scoped=25
append-log=7
shared-state=4
temporary=1
database=1
unknown=0
```

Evidence: `gsd_exec[2861b3f4-ce3d-44b2-b111-e4fee3e2c4aa]`.

## Before and after

| Stage | Unknown count | Notes |
|---|---:|---|
| M169 S01 baseline | 3 | Two CLI per-paper JSON writes plus one fetcher PDF cache write. |
| After S06 | 1 | CLI per-paper writes routed through atomic replacement. |
| After S07 and S08 | 0 | Fetcher PDF cache write routed through atomic replacement. |

## Resolved records

### CLI per-paper JSON artifacts

Resolved records:

```text
src/research_graph/cli/__init__.py
  paper_dir / 'paper.json'
  paper_dir / 'scored.json'
```

Resolution:

- Added `_atomic_write_text(...)` in `src/research_graph/cli/__init__.py`.
- Uses stdlib `tempfile.NamedTemporaryFile(...)` in the destination directory and `Path.replace(...)`.
- `write_paper_artifacts(...)` now writes `paper.json` and `scored.json` through atomic replacement.
- Focused tests pass: `tests/test_analysis.py`, 36 passed.

### Fetcher PDF cache artifact

Resolved record:

```text
src/research_graph/infrastructure/corpus/ingestion/fetchers.py
  pdf_path
```

Resolution:

- Added `_atomic_write_bytes(...)` in `src/research_graph/infrastructure/corpus/ingestion/fetchers.py`.
- Uses stdlib `tempfile.NamedTemporaryFile(...)` in the destination directory and `Path.replace(...)`.
- `PDFDownloader.download(...)` now writes validated PDF bytes through atomic replacement.
- Focused tests pass: `tests/test_pdf_downloader.py`, 5 passed.

## Residual risks

- Atomic replacement prevents partial final files but does not add lock-based multi-writer coordination for same-key cache writes.
- The inventory now has zero unknowns, but `shared-state=4` remains intentionally visible for future write-safety reviews.

## Closeout

The write-path track is closed for M169: unknown records are eliminated, stable cache writes are safer, and the final inventory artifacts are durable under `data/architecture-assessment/`.
