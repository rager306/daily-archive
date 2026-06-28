# M027 Conversion Quality Report

This report is metadata-only and does not embed raw article text, HTML, PDF bytes, or converted text payloads.

- Schema: `m027-conversion-quality.v1`
- Status: `completed`
- Network fetch attempted: `False`
- Production import attempted: `False`
- LadybugDB written: `False`
- Article count: 6
- Variant count: 11
- Parser ready count: 6
- Counts: `{'converted': 6, 'metadata_only': 5}`
- Diagnostics: `11`
- Command: `['uv', 'run', 'python', 'scripts/convert_m027_source_quality_boundary.py']`
- CWD: `/root/daily-archive`
- Git commit: `823b7b79d58b4a13259b71719433f4762193354d`

## Failure Modes

Filesystem read/hash failures, malformed S02 JSON, missing captured artifacts, unsafe paths, hash/size mismatches, PyMuPDF open failures, and empty/low-quality extraction all produce explicit blocked/failed/low_quality diagnostics with fail-closed flags.

## Load Profile

PDF extraction is the first expected saturation point at 10x load; extraction is bounded to 8 pages and 80000 characters per variant, with streamed hash checks for source bytes.

## Negative Tests

Covered by `tests/test_m027_conversion_quality_boundary.py`: malformed/unsafe paths, missing artifacts, hash mismatches, non-captured rows, metadata redaction, abs-page non-readiness, PDF fallback conversion, and Nature body extraction.
