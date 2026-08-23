# M169 Write Path Ownership Recon

## Verdict

**All three remaining unknown write paths are stable cache or artifact writes, not ephemeral run-scoped outputs.**

S05 recommends resolving them by adding bounded atomic writes in S06/S07 rather than merely teaching the inventory scanner to hide them under a broader category.

## Current unknown records

```text
src/research_graph/cli/__init__.py
  L355 write_text target=paper_dir / 'paper.json'
  L358 write_text target=paper_dir / 'scored.json'

src/research_graph/infrastructure/corpus/ingestion/fetchers.py
  L44 write_bytes target=pdf_path
```

## CLI per-paper JSON writes

Function:

```text
src/research_graph/cli/__init__.py::write_paper_artifacts(scored)
```

Current behavior:

- Builds `paper_dir = PAPERS_DIR / scored.paper.id`.
- `PAPERS_DIR = Path.home() / ".research" / "papers"` by default.
- Writes two stable per-paper artifacts:
  - `paper.json`
  - `scored.json`
- Called from `write_daily_artifacts(analysis)` for each scored paper.
- Tests monkeypatch `PAPERS_DIR` to `tmp_path / "papers"`, proving the directory is configurable in tests but stable by default.

Ownership classification:

- Not run-scoped: paths are keyed by paper id under a stable user cache directory.
- Not caller-owned in production default: caller does not pass the target path to `write_paper_artifacts`.
- Best category after hardening: `shared-state` or cache-style stable artifact write.

Risk:

- A concurrent or interrupted write could leave partial JSON in `~/.research/papers/<paper-id>/paper.json` or `scored.json`.
- The existing tests assert presence and payload shape, not atomic replacement behavior.

S06 recommendation:

- Add a CLI-local atomic JSON write helper or tiny atomic text helper.
- Route only `write_paper_artifacts` through it unless a focused test justifies broader CLI artifact changes.
- Add or extend a focused test to prove per-paper artifacts still persist.
- Run inventory and ensure the two CLI records are no longer unknown.

## Fetcher PDF write

Function:

```text
src/research_graph/infrastructure/corpus/ingestion/fetchers.py::PDFDownloader.download(arxiv_id, pdf_url=None)
```

Current behavior:

- Builds `pdf_path = self.cache_dir / f"{arxiv_id}.pdf"`.
- `DEFAULT_CACHE_DIR = Path.home() / ".arxiv_cache"` by default.
- Returns existing cached files without network access.
- Downloads with `httpx.Client`, validates content-type or PDF signature, then writes `pdf_path.write_bytes(response.content)`.
- Used by markdown conversion fallback through `MDConverter(pdf_downloader=...)` and tests instantiate `PDFDownloader(cache_dir=tmp_path)`.

Ownership classification:

- Stable cache write keyed by arXiv id.
- Not run-scoped: default path survives runs.
- Not purely caller-owned in production default: callers can pass `cache_dir`, but the downloader owns the file layout once constructed.
- Best category after hardening: stable cache or shared-state write, with atomic replacement to avoid partial PDFs.

Risk:

- Interrupted or concurrent download can leave a partial PDF in cache.
- Existing `pdf_path.exists()` short-circuits later reads, so a partial file could poison future calls.

S07 recommendation:

- Add a fetcher-local atomic bytes write helper.
- Write to a sibling temporary path and replace only after validation succeeds.
- Prefer a deterministic temp suffix including PID or a simple suffix if tests prove cleanup behavior.
- Add a focused test that failed writes do not leave the final PDF path, or that replacement succeeds for valid PDF content.
- Run inventory and ensure the fetcher record is no longer unknown.

## Scanner plan

Do not solve S06/S07 by scanner-only classification first. After atomic write helpers remove the direct `paper_dir / ...` and `pdf_path.write_bytes(...)` records, run `scripts/inventory_write_paths.py` and classify any helper temp-path writes as `temporary` if needed.

## Stop conditions

Stop and document a blocker if:

- GitNexus impact for `write_paper_artifacts` or `PDFDownloader.download` reports high or critical risk;
- focused tests show existing consumers require non-atomic direct writes;
- inventory still reports unknown records after bounded atomic edits and scanner cannot classify them without hiding shared-state risk.
