# M033 S06 T03 Artifact Contracts and Diagnostics

- status: `complete`
- graph_import_allowed=false
- ladybugdb_written=false
- production_import_attempted=false
- import_eligible=false

## Artifact tree

- `run_manifest.json`: selected corpus, local source paths, sha256, size, article keys, class labels, no-network setting
- `backend-preflight.json`: GROBID/OpenDataLoader versions, health, image/cache inventory, network-disabled behavior
- `per-paper/<article_key>/grobid.tei.xml`: raw TEI sidecar candidate, not graph-ready
- `per-paper/<article_key>/opendataloader/original.json`: layout/OCR/table sidecar candidate, fixed upstream schema
- `per-paper/<article_key>/typed-candidates.json`: Adaptix/daily-archive typed candidate summaries with candidate_only flags
- `per-paper/<article_key>/quality-review.json`: reviewed metric results and typed failures per quality dimension
- `review-packets/<article_key>/`: candidate graph-readiness review inputs, not import eligibility
- `events.jsonl`: bounded diagnostics with no secrets and no raw article bodies
- `summary.json`: aggregate pass/fail, blocker taxonomy, and false safety flags

## Logging rules

- No secrets in artifacts or logs
- No raw article bodies in diagnostics or aggregate summaries
- Use excerpts only when bounded/redacted and justified
- Record hashes/paths/IDs instead of full content

## Diagnostic taxonomy

- `missing_local_source` (`source_acquisition`): selected row lacks safe local PDF/source path
- `unsafe_or_stale_source_hash` (`source_acquisition`): source hash missing, mismatched, or stale
- `backend_unhealthy` (`runtime_preflight`): GROBID/OpenDataLoader backend health check failed
- `model_cache_missing_no_network` (`runtime_preflight`): required cache absent while no-network mode is enforced
- `tei_parse_failed` (`grobid_quality`): GROBID returned invalid or unusable TEI
- `bibliography_quality_below_gate` (`grobid_quality`): reference/citation/header quality below threshold
- `layout_quality_below_gate` (`opendataloader_quality`): reading order/layout/coordinate quality below threshold
- `table_fidelity_below_gate` (`opendataloader_quality`): table structure/cell/caption quality below threshold
- `ocr_quality_below_gate` (`opendataloader_quality`): OCR control did not meet adequacy threshold
- `adaptix_mapping_failed` (`adapter_contract`): fixed parser output could not be mapped to typed candidates
- `invalid_evidence_path` (`evidence_contract`): candidate anchor lacks stable source hash/path/span/coordinate linkage
- `low_quality_source` (`refusal_preservation`): substantive body/source quality inadequate despite parser output
- `review_packet_incomplete` (`review_boundary`): review packet missing completed review/output contract
- `graph_readiness_postcheck_failed` (`review_boundary`): graph-readiness review validate-only post-check failed

## No-write import rehearsal

- accepted_count=0
- import_eligible_count=0
- ladybugdb_write_attempts=0
