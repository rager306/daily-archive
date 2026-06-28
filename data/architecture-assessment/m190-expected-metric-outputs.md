# M190 Expected Metric Outputs

## Verdict

**Expected outputs are locked before bounded execution. S03 must compare observed outputs against this contract rather than redefining success afterward.**

## Contract inputs

- Metric contract: `data/architecture-assessment/m189-metric-contract.md`
- Ablation protocol: `data/architecture-assessment/m189-ablation-protocol.md`
- Bounded selection map: `data/architecture-assessment/m190-bounded-selection-command-map.md`

## Bounded execution selection

Primary bounded corpus:

- M027 local mixed-source six-article corpus.

Companion gates:

- M030 requested-ref intake validate-only.
- Representative extraction/evaluation/DSPy boundary tests.
- Focused low-quality source criteria tests.

## Expected command outcomes

| Gate | Expected outcome | Fail condition |
|---|---|---|
| M027 current pipeline replay | Command exits 0 and writes metadata-first replay artifacts under `data/architecture-assessment/m190-m027-current-pipeline-replay/` | Network fetch, graph write, production write, missing summary/report, untyped parser/chunk failure |
| M027 source boundary verifier | Command exits 0 and preserves fail-closed graph/production flags | Source boundary failure, raw payload leakage, graph/production flag true |
| M030 validate-only | Command exits 0 with 4 refs, 3 cataloged, 1 typed blocker, graph/import fail-closed | Fetch/write behavior, missing typed blocker, graph/import claim true |
| Extraction/evaluation/DSPy boundary tests | Combined tests pass | Test failure or optimizer activation |
| Low-quality source criteria tests | Focused low-quality tests pass | Low-quality source accepted as success without fallback reason |
| GitNexus detect_changes | LOW or expected artifact-only change; zero source-code symbols | Source symbol changes without prior impact analysis |

## Required observed labels

S03 execution summary must report:

- `source_quality_labels_present`
- `low_quality_source_fail_closed`
- `parser_ready_scope`
- `chunk_ready_scope`
- `extraction_metric_gate_passed`
- `retrieval_ablation_gate_passed`
- `dspy_boundary_gate_passed`
- `graph_import_ready=false`
- `production_persistence_ready=false`
- `optimizer_enabled=false`
- `direct_extractor_to_graph_write=false`

## Expected readiness result

M190 may claim only:

- bounded source boundary execution evidence exists;
- bounded parser/chunk replay evidence exists for M027 local replay scope if the replay command passes;
- metric/ablation/boundary gates pass as representative tests;
- graph/import/persistence/optimizer remain not ready and disabled.

M190 must not claim:

- broad parser readiness beyond the bounded M027 replay scope;
- production retrieval quality;
- graph import readiness;
- LadybugDB production persistence readiness;
- DSPy/RLM optimizer readiness.

## Stop conditions

Stop and mark needs-attention if any execution result shows:

- network fetch during local-only replay;
- missing or untyped low-quality source fallback;
- parser-ready article with no substantive body text;
- zero chunks without typed diagnostic;
- raw source payload in metadata artifact;
- graph/import flag true;
- production persistence flag true;
- optimizer invocation;
- GitNexus HIGH or CRITICAL risk.
