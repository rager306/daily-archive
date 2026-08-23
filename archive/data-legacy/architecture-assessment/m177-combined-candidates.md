# M177 Combined Candidates

## Verdict

**Candidate review complete.** M177 can include all five requested directions, but not all directions imply scanner category movement. Movement is allowed only for exact reviewed source paths.

## Baseline

```text
total_records=341
script-only=235
unknown=0
shared-state=0
```

## Direction 1: R024 script inventory wave

Candidate movement: **23 script-only records** across exact R024 script paths. Existing `src/research_graph/infrastructure/graph/r024_networkx_probe.py` has 2 records already classified as `graph-probe-output` and is not part of this move.

| Source path | Records | Current category | Candidate category | Targets |
|---|---:|---|---|---|
| `scripts/build_r024_20_document_corpus_selection.py` | 3 | `script-only` | `r024-corpus-selection-output` | `OUT_SELECTION`, `OUT_EVENTS`, `OUT_SUMMARY` |
| `scripts/build_r024_53_document_corpus_selection.py` | 3 | `script-only` | `r024-corpus-selection-output` | `OUT_SELECTION`, `OUT_EVENTS`, `OUT_SUMMARY` |
| `scripts/extract_r024_entity_scale_entities.py` | 3 | `script-only` | `r024-entity-extraction-output` | `article_file`, `EVENTS_LOG`, `SUMMARY` |
| `scripts/convert_r024_53_pdf_to_text.py` | 3 | `script-only` | `r024-conversion-output` | `out_path`, `EVENTS_LOG`, `SUMMARY` |
| `scripts/build_r024_entity_networkx_probe.py` | 3 | `script-only` | `r024-networkx-probe-output` | `SUMMARY`, `MEMORY_PROFILE`, `PROBE_EVENTS` |
| `scripts/extract_r024_quality_metrics.py` | 2 | `script-only` | `r024-quality-metrics-output` | `METRICS`, `COMPARISON` |
| `scripts/extract_r024_20_document_quality_metrics.py` | 2 | `script-only` | `r024-quality-metrics-output` | `METRICS`, `COMPARISON` |
| `scripts/extract_r024_53_document_quality_metrics.py` | 2 | `script-only` | `r024-quality-metrics-output` | `METRICS`, `COMPARISON` |
| `scripts/extract_r024_entity_quality_metrics.py` | 2 | `script-only` | `r024-quality-metrics-output` | `METRICS`, `COMPARISON` |

## Direction 2: Scanner self-output ownership review

Candidate movement: **3 script-only records** from exact scanner source path.

| Source path | Records | Current category | Candidate category | Targets |
|---|---:|---|---|---|
| `scripts/inventory_write_paths.py` | 3 | `script-only` | `inventory-report-output` | `args.json`, `args.markdown`, `args.delta_markdown` |

## Direction 3: Markdown cache policy review

Candidate movement: **0 records**. Baseline has 2 records already classified as `caller-owned`. M177 should document cache policy rather than move them by broad cache target names.

| Source path | Records | Current category | Decision | Targets |
|---|---:|---|---|---|
| `src/research_graph/infrastructure/corpus/sources/markdown_converter.py` | 2 | `caller-owned` | no scanner move in M177 | `md_path`, `method_path` |

## Direction 4: Queue and smoke output ownership review

Candidate movement: **11 script-only records** across exact queue/smoke script paths. Existing `src/research_graph/workflows/universal_kb/*` records are already categorized as `database`, `caller-owned`, or `run-scoped` and should not move in M177.

| Source path | Records | Current category | Candidate category | Targets |
|---|---:|---|---|---|
| `scripts/soak_universal_kb_queue.py` | 1 | `script-only` | `queue-soak-output` | `args.json_out` |
| `scripts/verify_m072_queue_benchmark_gate.py` | 1 | `script-only` | `queue-gate-output` | `output_path` |
| `scripts/verify_m073_queue_evidence_gate.py` | 1 | `script-only` | `queue-gate-output` | `output_path` |
| `scripts/m060g_smoke_test.py` | 1 | `script-only` | `smoke-script-output` | `output_path` |
| `scripts/replay_m028_smoke_closeout.py` | 3 | `script-only` | `smoke-script-output` | `events_path`, `summary_path`, `report_path` |
| `scripts/run_m029_unified_loader_runtime_smoke.py` | 1 | `script-only` | `smoke-script-output` | `fd` |
| `scripts/verify_m029_unified_loader_runtime_smoke.py` | 1 | `script-only` | `smoke-script-output` | `fd` |
| `scripts/run_m122_mutation_smoke.py` | 2 | `script-only` | `smoke-script-output` | `spec.path` |

## Direction 5: Inventory delta CI wiring

Candidate movement: **0 inventory records**. This is workflow wiring only. It should add a cheap check to existing architecture guardrail CI if feasible, without writing tracked generated artifacts during CI.

## Explicit no-move groups

- Generic target names remain forbidden for scanner classification.
- Existing `caller-owned`, `run-scoped`, `database`, and `graph-probe-output` categories are not reclassified in M177.
- Markdown converter paths stay `caller-owned` in scanner output; M177 only documents cache policy.
- CI does not create write-path inventory records and should not be forced into scanner categories.
- Any unreviewed script remains `script-only`.

## Expected movement if all implementation slices land

```text
r024 script movement: -23 from script-only
inventory-report-output: +3
queue and smoke movement: -11 from script-only
net script-only residual target: 198
unknown=0
shared-state=0
```
