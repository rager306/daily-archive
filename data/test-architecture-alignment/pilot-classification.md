# Pilot Test Classification

Schema: `daily-archive-test-pilot-classification.v1`

## Summary

- Total pilot files: `13`
- Strictly checked: `13`

| Intended layer | Count |
|---|---:|
| `acceptance` | 1 |
| `application` | 6 |
| `infrastructure` | 5 |
| `script-wrapper` | 1 |

## Files

| Path | Intended layer | Current bucket | Strict | Status |
|---|---|---|---:|---|
| `tests/test_catalog_ingest_filesystem_adapter.py` | `infrastructure` | `infrastructure` | true | `aligned` |
| `tests/test_catalog_ingest_m056.py` | `infrastructure` | `infrastructure` | true | `aligned` |
| `tests/test_catalog_ingest_use_case.py` | `application` | `application` | true | `aligned` |
| `tests/test_corpus_coverage_report_writer.py` | `infrastructure` | `infrastructure` | true | `aligned` |
| `tests/test_corpus_coverage_use_case.py` | `application` | `application` | true | `aligned` |
| `tests/test_graph_probe_use_case.py` | `application` | `application` | true | `aligned` |
| `tests/test_m122_property_mutation_guards.py` | `application` | `application` | true | `aligned` |
| `tests/test_networkx_graph_probe_adapter.py` | `infrastructure` | `infrastructure` | true | `aligned` |
| `tests/test_parser_replay_adapters.py` | `infrastructure` | `infrastructure` | true | `aligned` |
| `tests/test_parser_replay_use_case.py` | `application` | `application` | true | `aligned` |
| `tests/test_pipeline_architecture_acceptance.py` | `acceptance` | `acceptance` | true | `aligned` |
| `tests/test_pipeline_script_inventory.py` | `application` | `application` | true | `aligned` |
| `tests/test_riskratchet_gate.py` | `script-wrapper` | `script-wrapper` | true | `aligned` |
