# M184 GitNexus Planning Signals

## Queries used

GitNexus was queried for four planning axes:

1. `pipeline script inventory wrapper contracts architecture boundaries scripts application infrastructure`
2. `corpus coverage report replay graph probe use case CLI wrappers`
3. `write path inventory canonical baseline category classifier tests`
4. `cache manifest lifecycle invalidation concurrency owner consumer contract`

## Signals

- GitNexus surfaced script wrapper and inventory validation symbols such as `validate_inventory`, `validate_report`, and `main` in `scripts/verify_m030_pipeline_module_inventory.py`.
- Replay and acquisition flows appeared around `scripts/run_m029_unified_replay.py::run`, `scripts/run_m029_unified_replay.py::replay_output_paths`, `scripts/replay_m031_catalog_backed_loader_evidence.py::main`, and `scripts/replay_m027_end_to_end_mixed_replay.py::replay_end_to_end`.
- Source loading context appeared around archived `load_article_source` execution flows, which makes acquisition/source movement higher risk than pure report-output classification.
- Cache/manifest/lifecycle queries surfaced queue ownership and lifecycle style symbols such as `UniversalKBQueue.enqueue` and `_require_running_owner`; this reinforces that cache/index/manifest movement needs explicit owner/invalidation/concurrency proof, not scanner naming heuristics.
- GitNexus results were useful for planning families, but many scanner and script internals may still return UNKNOWN during impact. UNKNOWN remains non-proof and must be compensated with focused tests, generated deltas, strict drift, and final detect_changes.

## Planning constraints from GitNexus

- Run `gitnexus_impact` before editing any function/class/method selected for extraction.
- Treat replay/conversion and graph/connectivity waves as higher risk because GitNexus found active execution flows.
- Prefer exact inventory movement before script-to-src extraction, so S09 can choose the safest real seam using evidence from S03-S08.
- Run final `gitnexus_detect_changes({scope: "compare", base_ref: "HEAD~1", repo: "daily-archive"})` after commit-level changes.
