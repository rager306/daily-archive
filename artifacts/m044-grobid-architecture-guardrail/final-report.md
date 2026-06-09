# M044 Final Report — Live GROBID Probe and Architecture Guardrail

## Verdict

GROBID is live locally and architecture drift is now guarded by an executable context-pack verifier. M044 produced candidate-only live GROBID evidence for the one target article with a local PDF and typed missing_pdf blockers for the five linked target records without local PDFs. No graph import is authorized.

## Service status

- GROBID service status: `live_ready`
- Service URL: `http://127.0.0.1:8070`
- Background process: `099f0de5`
- Keep running: true, per user request

## Architecture guardrail

- Context pack: `m044-sidecar-architecture-context-v1`
- Preflight: `uv run python scripts/verify_m044_sidecar_architecture_guardrail.py`
- Mandatory decisions: M033, ADR-003, ADR-004, ADR-005, ADR-007, D078, D079

## Live candidate packets

- Article count: 6
- Status counts: {'live_success': 1, 'missing_pdf': 5}
- Raw TEI/full text persisted: false
- Candidate only: true
- Graph writes: disabled
- Production import: disabled
- Fact promotion: disabled

| Article | Status | TEI bytes | biblStruct | div | ref | Blockers |
|---|---|---:|---:|---:|---:|---|
| 1804.02767 | missing_pdf | 0 | 0 | 0 | 0 | local_pdf_missing |
| 2108.12409 | missing_pdf | 0 | 0 | 0 | 0 | local_pdf_missing |
| 2109.10862 | missing_pdf | 0 | 0 | 0 | 0 | local_pdf_missing |
| 2111.00396 | missing_pdf | 0 | 0 | 0 | 0 | local_pdf_missing |
| 2203.14465 | missing_pdf | 0 | 0 | 0 | 0 | local_pdf_missing |
| 2512.24601 | live_success | 266172 | 50 | 61 | 139 | none |

## Remaining blockers

- `bounded_local_pdf_acquisition_for_five_linked_records` before full target GROBID TEI coverage.
- Future OpenDataLoader/Adaptix target-specific evidence still depends on local PDFs/fixed JSON for linked records.
- ADR-005 remains binding: sidecar success is not graph readiness or import eligibility.
