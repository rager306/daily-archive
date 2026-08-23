# M184 Remaining Residual Triage

## Verdict

**Movement decision: move 29 remaining non-cache process-boundary outputs; keep 4 manifest/cache-like records no-move.**

## Baseline

```text
script-only=33
unknown=0
shared-state=0
```

## GitNexus

- `_classify` impact: UNKNOWN, not safety proof.
- GitNexus surfaced governance sync and queue/diagnostic flows; S08 keeps behavior unchanged and edits only scanner/tests.
- Manifest/cache-like records remain fail-closed for S11 lifecycle proof.

## Move buckets

### Governance sync output: 4

- `scripts/augment_m073_evidence_paths.py`
- `scripts/sync_codebase_memory_governance.py`

### Experiment probe output: 12

- `scripts/compare_m055_header_vs_fulltext.py`
- `scripts/m052_rlm_e2e.py`
- `scripts/m058_marker_extract_5.py`
- `scripts/m058_plotextractor_embed.py`
- `scripts/m058_plotextractor_similarity.py`
- `scripts/m059_e2e_test.py`
- `scripts/m068_integration_test.py`
- `scripts/m103_extraction_prototype.py`

### Misc architecture artifact output: 13

- `scripts/build_m043_sidecar_packets.py`
- `scripts/m061_synthesis.py`
- `scripts/run_pipeline_architecture_acceptance.py`
- `scripts/synthesize_m029_unified_readiness.py`
- `scripts/update_m043_target_subset_post_m053.py`
- `scripts/update_m043_target_subset_post_m054.py`
- `scripts/verify_article_catalog.py`
- `scripts/verify_m022_final_gate.py`
- `scripts/verify_m023_artifact_scaffold_gate.py`
- `scripts/verify_m025_article_catalog.py`
- `scripts/verify_m025_baseline_recovery_outputs.py`

## No-move cache/manifest-like records: 4

- `scripts/benchmark_m055_corpus_manifest.py`
- `scripts/build_m055deep_corpus_manifest_20.py`
- `scripts/m058_build_graph_manifest.py`
- `scripts/m059_build_manifest.py`

## Boundaries

- No broad governance, experiment, misc, manifest, cache, path, output, report, summary, or target-name rule.
- No runtime code movement in S08.
- No cache/manifest movement until S11 proof gate.
