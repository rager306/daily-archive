# Pipeline Continuity Audit

- overall: `partial`
- import_eligible: `False`
- graph_writes_allowed: `False`
- falkor_touched: `False`

| Layer | Health | Present | Missing | Gaps |
|-------|--------|---------|---------|------|
| source | partial | 4 | 0 | 2 |
| parser | partial | 2 | 0 | 1 |
| structure | partial | 2 | 0 | 1 |
| extraction | partial | 4 | 0 | 1 |
| graph | partial | 5 | 0 | 3 |
| review | partial | 4 | 0 | 2 |
| agents | partial | 3 | 0 | 2 |

## Gap codes

- `source:pdf_body_not_fulltext`
- `source:no_batch_filter_in_core_loader`
- `parser:real_pdf_quality_variance`
- `structure:real_corpus_chunk_quality_not_continuously_gated`
- `extraction:live_llm_optional_not_fleet`
- `graph:no_live_falkor_driver_by_policy`
- `graph:composition_root_missing_pre_m209`
- `graph:cli_not_wired_to_projection`
- `review:operator_cli_not_wired`
- `review:pilot_eligible_not_import_eligible`
- `agents:symfsm_not_cli_wired`
- `agents:experience_store_deferred`
