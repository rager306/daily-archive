---
estimated_steps: 15
estimated_files: 4
skills_used: []
---

# T03: Built Wave 1 analysis, cumulative 50-PDF corpus, tests, and verified Wave 1 plus M050-M055deep regression gates.

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

Final verification:
- uv run pytest tests/test_m056_wave_1.py -q (5+ pass)
- M045 trajectory on_track, M044 guardrail exit 0
- gsd_checkpoint_db + commit with feat(m056-bfs): S01 Wave 1 message

## Inputs

- `scripts/benchmark_m055deep_grobid_fulltext.py`
- `scripts/benchmark_m055_opendataloader_only.py`
- `artifacts/m056-bfs-graph/wave-1/`
- `artifacts/m055deep-parser-benchmark/corpus-manifest-20.json`

## Expected Output

- `artifacts/m056-bfs-graph/wave-1/analysis.md`
- `artifacts/m056-bfs-graph/wave-1/cumulative-corpus.json`
- `tests/test_m056_wave_1.py`
- `.gsd/gsd.db`

## Verification

uv run pytest tests/test_m056_wave_1.py -q
