# M033 S06 T03 Adoption and Rollback Criteria

- adoption_decision_allowed_by_M033: `false`
- graph_import_allowed=false
- ladybugdb_written=false
- production_import_attempted=false
- import_eligible=false

## Future adoption minimums

- all selected corpus classes evaluated
- GROBID and OpenDataLoader gates pass or typed blockers explain failures
- Adaptix/daily-archive typed contracts pass
- review packet post-check passes
- no-write import rehearsal remains clean
- human/agent decision records adoption separately

## Rollback or no-adoption triggers

- implicit network/model download required
- untyped backend/parser failure
- raw text/secrets leaked to diagnostics
- low_quality_source/refusal bypass
- table/OCR/reading-order gates fail without acceptable typed blocker
- invalid/stale EvidencePath accepted
- review packet incomplete
- any graph/import/write flag becomes true

## Explicit non-authorizations

- M033 does not authorize production integration
- M033 does not authorize dependency adoption
- M033 does not authorize graph import
- M033 does not authorize LadybugDB writes
- M033 does not authorize import eligibility
