# Bounded source acquisition plan

## Purpose

M006/S01 selected 30 papers but found only 10 Markdown-ready papers. S02 will attempt bounded source acquisition/conversion for the 20 missing-Markdown expansion papers so S03 can analyze real 30-paper deviations rather than mostly source blockers.

## Existing project mechanisms

- `src/arxiv_archive/md_converter.py` provides `MDConverter.convert()`.
  - Uses cached Markdown when present.
  - Attempts `arxiv2md.org` first.
  - If needed, downloads PDF through `PDFDownloader` and converts with Marker when available.
  - If Marker is not available, falls back to Docling.
  - Rejects low-quality Markdown through `assess_full_text_quality()`.
- `src/arxiv_archive/pdf_downloader.py` downloads PDFs into `~/.arxiv_cache` with content-type/PDF-header validation.
- `src/arxiv_archive/full_text.py` ingests local Markdown/text only and classifies missing, empty, low-quality, structured, or plain text sources.

## Bounded method order

For each of the 20 papers missing Markdown:

1. Check `~/.arxiv_cache/{paper_id}.md` and `/root/.research/papers/{paper_id}/full_text.md` again.
2. If Markdown is still missing, call `MDConverter.convert(paper_id)`.
3. If conversion returns Markdown, write/cache is handled by `MDConverter`; copy or mirror the result to `/root/.research/papers/{paper_id}/full_text.md` only when quality is OK.
4. Record a redacted diagnostic with method, outcome, output path, PDF path if present, error class/message prefix, and quality counts.
5. Do not run any KG import, embedding generation, vector generation, extraction, or LadybugDB write.

## Operational limits

- Concurrency: sequential or very small bounded concurrency only. Start sequentially to avoid arxiv2md/PDF pressure.
- Timeout: use existing converter/downloader timeouts; do not increase Marker timeout beyond existing project default.
- Marker: optional only. Do not depend on Marker completing; Docling is the accepted default PDF fallback when Marker is unavailable.
- Retries: no unbounded retries. One conversion attempt per missing-Markdown paper in this slice.
- Output: machine artifacts store paths, hashes/sizes, method/outcome, quality metrics, and redacted errors only.

## Forbidden during S02

- Production LadybugDB KG writes.
- Trusted KG import.
- Embedding/vector generation.
- DSPy optimizers or live unbounded RLM behavior.
- Raw paper text or chunk text in JSON/JSONL logs.
- Raw PDF/image/base64 payloads in JSON/JSONL logs.
- Secrets, tokens, credentials, or optimizer traces in diagnostics.

## Expected S02 outputs

- `source-acquisition-summary.json` with before/after readiness counts.
- `source-acquisition-diagnostics.jsonl` with one redacted diagnostic per selected paper.
- `source-acquisition-report.md` explaining readiness deltas and whether S03 can run full 30-paper deviation analysis.

## Go/partial-go rule for S03

- Full-go: all or nearly all 30 papers become Markdown-ready; S03 can analyze chunking/import-model deviations across 30.
- Partial-go: some papers remain blocked; S03 must separate source blockers from chunking/import-model results and avoid claiming full 30-paper chunk behavior.
