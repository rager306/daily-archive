# M033 S01 T01: Current Pipeline Entrypoints

This inventory maps the current daily-archive article-processing baseline before comparing GROBID, OpenDataLoader, or quant-mind patterns.

| Stage | Role | Entrypoints | Current boundary |
|---|---|---|---|
| catalog_intake | Bound requested refs to article catalog rows or typed catalog blockers before any source/parser work. | `scripts/build_m031_catalog_backed_replay_selection.py`<br>`scripts/register_m029_missing_metadata_refs.py`<br>`scripts/verify_m030_requested_ref_intake.py` | S01/M031 shows catalog gaps become typed blockers, not silent success. |
| source_acquisition | Copy or reference safe local source artifacts and convert unavailable rows to typed blockers. | `scripts/replay_m031_catalog_backed_acquisition.py`<br>`scripts/replay_m027_current_pipeline_baseline.py` (`replay_baseline`) | No network fetch is assumed for M031/M033 baseline comparison. |
| loader_evidence | Load only captured local artifacts and align all non-captured acquisition rows to loader blockers. | `scripts/replay_m031_catalog_backed_loader_evidence.py` (`loader_result_for_capture`)<br>`scripts/verify_m029_unified_loader_runtime_smoke.py` (`verify`) | Loader evidence is not parser/chunk/graph readiness. |
| parser_conversion | Convert only usable local artifacts and emit refusal diagnostics for unsafe, missing, metadata-only, external-only, or low-quality variants. | `scripts/replay_m031_parser_conversion.py`<br>`scripts/verify_m031_parser_conversion_replay.py`<br>`scripts/convert_m029_unified_source_quality_boundary.py` (`run_conversion`)<br>`scripts/verify_m029_unified_conversion_quality_boundary.py` (`verify`) | Non-empty extracted text is not sufficient; low_quality_source remains a first-class refusal state. |
| chunk_evidence | Chunk only parser-ready converted text and emit zero-chunk refusals for non-ready rows. | `scripts/replay_m031_chunk_evidence.py`<br>`scripts/verify_m031_chunk_evidence_replay.py` | Chunks are candidate evidence and do not imply import eligibility. |
| graph_readiness_handoff | Create reviewer packets only from chunk evidence and require independent completed review before import eligibility. | `python module arxiv_archive.graph_readiness_review` | Pending review packets are not graph-ready facts. |
| no_write_import_boundary | Rehearse import eligibility in no-write fail-closed mode and preserve graph/import/LadybugDB flags false. | `scripts/verify_m031_s05_closeout.py`<br>`scripts/verify_m031_process_continuity_audit.py` (`build_progression_matrix`) | Accepted/import-eligible counts remain zero unless prior gates explicitly pass in a future milestone. |

## Safety summary

- This inventory does not modify code.
- Parser/conversion success remains candidate evidence only.
- Graph import, production import, and LadybugDB writes remain out of scope.
