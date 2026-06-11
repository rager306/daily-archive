# M057 S01 Marker vs OpenDataLoader

This comparison is diagnostic only; graph import is disabled and production import is disabled.

## Sample (n=1, real extraction)

After the env fix (transformers 4.57.6), Marker now runs end-to-end on a real PDF.

- Sample size: 1 PDF (2605.28617v1, "LACUNA: Safe Agents as Recursive Program Holes", EPFL, 19 pages, 0.4 MB)
- Marker status: marker_extracted
- OpenDataLoader status: low_quality_source (header-only path)
- Marker markdown: 94715 chars (13253 body words)
- OpenDataLoader markdown: 82491 bytes
- Marker slowdown: 162.7x slower than OpenDataLoader (341 sec vs 2.1 sec)
- Markdown size ratio Marker/ODL: 1.148x (14.8% larger)

## Quality

- Marker table structure quality: 0.85 (baseline assumption, not measured)
- OpenDataLoader table structure quality: 0.0 for this PDF (status=low_quality_source)
- Marker detected 0 tables via simple heuristic; the actual tables exist in the markdown output but require a proper parser to enumerate

## Interpretation

- Marker successfully extracted the 19-page LACUNA paper with full body text, sections, math, and citations.
- OpenDataLoader marked the same PDF as low_quality_source because it relied on header-only GROBID path; the markdown body was still produced (82491 bytes) but lacks structural fidelity.
- The 1-PDF sample is too small for a robust comparison. Per M059 plan, expand to 5-10 PDFs across categories.
- Cost analysis: at 341s per 0.4MB PDF, full 166-PDF re-extraction is estimated at 8-15 hours single-threaded, or 2-4 hours with 4-way parallelism. Cost vs benefit is borderline; M059 should re-evaluate.

## Safety

All five safety defaults remain false. This is diagnostic-only comparison data; graph import is disabled and production import is disabled.

## Historical (pre-fix)

The previous version of this artifact (M057 S01 first pass) showed:

- Marker PDFs: 166
- Marker extracted: 0 (all 166 marked marker_unavailable due to env)
- OpenDataLoader matched PDFs: 165
- All 166 had Marker quality 0.0 vs OpenDataLoader quality 1.0 (degenerate)

That data has been superseded by this 1-PDF real sample.
