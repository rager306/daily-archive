# M029 Unified Conversion Quality Report

This report is metadata-only and does not embed raw article text, HTML, PDF bytes, or converted text payloads.

- Schema: `m029-conversion-quality.v1`
- Status: `completed_with_diagnostics`
- Network fetch attempted: `False`
- Production import attempted: `False`
- LadybugDB written: `False`
- Article count: 18
- Variant count: 29
- Parser ready count: 15
- Counts: `{'blocked': 7, 'converted': 15, 'metadata_only': 7}`
- Diagnostics: `29`
- Command: `['uv', 'run', 'python', 'scripts/convert_m029_unified_source_quality_boundary.py']`
- CWD: `/root/daily-archive`
- Git commit: `cf48c2b982c0139fa54a6df2f37abfc8b2a47cc1`

## Failure Modes

Filesystem read/hash failures, malformed S02 JSON, missing captured artifacts, unsafe paths, hash/size mismatches, PyMuPDF open failures, and empty/low-quality extraction all produce explicit blocked/failed/low_quality diagnostics with fail-closed flags.

## Load Profile

PDF extraction is the first expected saturation point at 10x load; extraction is bounded to 8 pages and 80000 characters per variant, with streamed hash checks for source bytes.

## Negative Tests

Covered by the M029 converter/verifier replay contract: malformed/unsafe paths, missing artifacts, hash mismatches, non-captured rows, metadata redaction, abs-page non-readiness, PDF/HTML fallback conversion, semantic-body rejection, and fail-closed parser-ready gates.
