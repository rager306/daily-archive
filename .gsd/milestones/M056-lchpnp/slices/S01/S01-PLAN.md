# S01: Wave 1: first 30 mostmentioned refs of 2605.18747

**Goal:** Acquire first 30 mostmentioned refs of 2605.18747, run GROBID fulltext + OpenDataLoader, per-wave analysis with conectividad gain + parser quality + self-citation cluster + category/length distribution.
**Demo:** 30 new PDFs acquired in corpus + GROBID fulltext + OpenDataLoader metrics + per-wave connectivity analysis.

## Must-Haves

- 30 new PDFs acquired (90%+ success rate)
- 30 GROBID fulltext + 30 OpenDataLoader packets
- Wave 1 analysis with connectivity gain, parser quality, self-citation cluster
- Cumulative corpus: 50 PDFs
- 5+ tests pass
- 5 safety defaults stay false
- M045 trajectory on_track, M044 guardrail exit 0
- 1 commit in git history

## Proof Level

- This slice proves: operational

## Integration Closure

Provides Wave 1 evidence. Updates cumulative corpus manifest.

## Verification

- Wave 1 acquisition log, GROBID + OpenDataLoader packets, analysis report, cumulative corpus.

## Tasks

- [x] **T01: Acquired 30 Wave 1 arXiv PDFs for M056 S01 and wrote the acquisition log plus corpus manifest.** `est:20m`
  scripts/acquire_m056_wave.py that downloads 30 PDFs from arxiv.org/pdf/{id}. Uses bounded retry (3 attempts, 30s timeout). Input: arxiv ID list (skip self 2605.18747). Reads from /tmp/wave-order.json or hardcoded for Wave 1 (first 30 after self-skip). Output: artifacts/m056-bfs-graph/wave-1/acquisition-log.json with per-PDF status, sha256, attempts, http_status, error.
  Accept 25/30 minimum (90% threshold). Document 404s and rate limits.
  Per-PDF success/status field: acquired (HTTP 200) / blocked (404) / network_error (timeout).
  - Files: `artifacts/m056-bfs-graph/wave-1/acquisition-log.json`, `scripts/acquire_m056_wave.py`
  - Verify: test -f artifacts/m056-bfs-graph/wave-1/acquisition-log.json

- [x] **T02: Ran GROBID fulltext and OpenDataLoader probes over the 30 acquired Wave 1 PDFs.** `est:15m`
  Run GROBID /api/processFulltextDocument and OpenDataLoader on each of 30 acquired PDFs. Per-PDF:
  - GROBID: TEI with body, refs, bibl, equations, figures, sections
  - OpenDataLoader: markdown with tables, images, sections, pages, bboxes
  - Both: 5-flag safety defaults explicit
  - Fail-closed on errors
  Output: artifacts/m056-bfs-graph/wave-1/grobid-fulltext/per-pdf/*.json + summary.json, and opendataloader/per-pdf/*.json + summary.json.
  Use scripts/benchmark_m055deep_grobid_fulltext.py and scripts/benchmark_m055_opendataloader_only.py.
  - Files: `artifacts/m056-bfs-graph/wave-1/grobid-fulltext/summary.json`, `artifacts/m056-bfs-graph/wave-1/opendataloader/summary.json`
  - Verify: test -f artifacts/m056-bfs-graph/wave-1/grobid-fulltext/summary.json

- [x] **T03: Built Wave 1 analysis, cumulative 50-PDF corpus, tests, and verified Wave 1 plus M050-M055deep regression gates.** `est:15m`
  scripts/analyze_m056_wave_1.py that reads:
  - Wave 1 acquisition log
  - 30 GROBID fulltext packets
  - 30 OpenDataLoader packets
  - Existing 20-PDF corpus (M055-kyxuqm corpus-manifest-20.json)
  - 2605.18747 GROBID TEI (anchor)
  Emits:
  - artifacts/m056-bfs-graph/wave-1/analysis.md: connectivity gain (new edges between Wave 1 PDFs and existing corpus), parser quality distribution (success/low_quality_source), self-citation cluster detection (first-author match), category distribution, length buckets (short/medium/long)
  - artifacts/m056-bfs-graph/wave-1/cumulative-corpus.json: 50 PDFs (20 existing + 30 new) with per-PDF source_milestone label (M027/M041, M051, M055deep, M056-wave-1) and sha256
  - Wave 1 tests: test_acquisition_min_25_pdfs, test_grobid_fulltext_30_packets, test_opendataloader_30_packets, test_connectivity_gain_nonzero, test_self_citation_cluster_detection, test_cumulative_corpus_50_pdfs, test_5_safety_defaults
  + M050+M051+M052+M053+M054+M055deep regression
  - Files: `artifacts/m056-bfs-graph/wave-1/analysis.md`, `artifacts/m056-bfs-graph/wave-1/cumulative-corpus.json`, `tests/test_m056_wave_1.py`, `scripts/analyze_m056_wave_1.py`
  - Verify: uv run pytest tests/test_m056_wave_1.py -q

## Files Likely Touched

- artifacts/m056-bfs-graph/wave-1/acquisition-log.json
- scripts/acquire_m056_wave.py
- artifacts/m056-bfs-graph/wave-1/grobid-fulltext/summary.json
- artifacts/m056-bfs-graph/wave-1/opendataloader/summary.json
- artifacts/m056-bfs-graph/wave-1/analysis.md
- artifacts/m056-bfs-graph/wave-1/cumulative-corpus.json
- tests/test_m056_wave_1.py
- scripts/analyze_m056_wave_1.py
