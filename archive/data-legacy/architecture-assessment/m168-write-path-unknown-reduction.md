# M168 Write Path Unknown Reduction

## Verdict

**Backlog item 4 status: CLOSED for safe scanner reduction scope.**

M168 reduced write-path inventory `unknown` records from 26 to 3 by adding conservative scanner categories instead of forcing unknown records into unsafe existing buckets.

## Change

Updated `scripts/inventory_write_paths.py` with two conservative categories:

- `caller-owned`: caller-provided or adapter-owned output paths such as `path`, `filepath`, `destination`, report paths, cache paths, summary paths, and validation artifact paths.
- `temporary`: same-directory temporary writes before final replacement.

Existing `shared-state` classification remains earlier than these new categories, so queue/state/index/catalog-like paths still stay conservative.

## Before

Baseline from S01/M167-style inventory:

```text
total_records=344
script-only=263
run-scoped=41
unknown=26
append-log=7
shared-state=6
database=1
```

## After

Current inventory after M168 catalog atomic write changes and scanner heuristic update:

```text
total_records=342
script-only=263
caller-owned=38
run-scoped=25
append-log=7
shared-state=4
temporary=1
database=1
unknown=3
```

Evidence: `gsd_exec[b1202e7b-ec15-4b74-a056-edfe95137cae]`.

## Remaining unknown records

```text
src/research_graph/cli/__init__.py
  L355 write_text target=paper_dir / 'paper.json'
  L358 write_text target=paper_dir / 'scored.json'

src/research_graph/infrastructure/corpus/ingestion/fetchers.py
  L44 write_bytes target=pdf_path
```

These remain unknown because they need ownership review:

- `paper_dir / paper.json` and `paper_dir / scored.json` are per-paper CLI artifacts but use a stable global directory; classifying them as run-scoped would require confirming directory ownership.
- `pdf_path` may be a fetch cache or canonical source path depending on caller; classifying it without caller context could hide shared-state risk.

## Verification

```text
uv run python scripts/inventory_write_paths.py --json /tmp/m168-s10-inventory-after.json --markdown /tmp/m168-s10-inventory-after.md
unknown_count=3

uv run ruff check scripts/inventory_write_paths.py
All checks passed
```

## Closeout impact

This improves future write-path reviews by replacing generic unknowns with an explicit `caller-owned` bucket while leaving truly ambiguous stable-path writes visible as `unknown`.
