# M170 Integrated Verification

## Verdict

**Integrated verification status: PASS.**

All M170 tracks coexist:

1. architecture backlog remediation evidence is complete;
2. cache coordination is closed as atomic-only policy;
3. longer queue soak harness and runtime proof pass;
4. guardrails remain green.

## Verification matrix

| Check | Result | Evidence |
|---|---|---|
| Integrated focused pytest | PASS: 28 passed | `gsd_exec[39a54e20-f500-4711-a8c0-babd2e4cfb01]` |
| Soak harness smoke | PASS: 16/16 completed | `gsd_exec[29956f46-cf8c-45c2-9b50-bcc8a7281c0a]` |
| Test architecture guard | PASS: dynamic=0, legacy=0, violations=0 | `gsd_exec[189bdc44-5dbd-49ae-a017-112b118ec15d]` |
| Onion guard | PASS: violation_count=0, allowed_violation_count=0 | `gsd_exec[78ff0c76-abaa-4d26-af58-c8321cd3f6f9]` |
| Write-path inventory | PASS: unknown=0, shared-state=4, total_records=340 | `gsd_exec[10041010-875b-4473-ad62-64e80840cc6f]` |

## Focused pytest target

```text
uv run pytest \
  tests/test_universal_kb_queue.py \
  tests/test_analysis.py::test_s05_subprocess_same_date_rerun_overwrites_stable_paths \
  tests/test_pdf_downloader.py::test_download_writes_pdf_with_atomic_replacement \
  tests/test_pdf_downloader.py::test_download_rejects_non_pdf_response \
  -q
```

Result:

```text
28 passed
```

## Soak evidence

Long runtime proof:

```text
data/architecture-assessment/m170-queue-soak-result.json
```

Integrated smoke:

```text
jobs_per_round=8
processes=4
rounds=2
total_jobs=16
total_completed=16
unique_completed=16
worker_errors=[]
stuck_workers=[]
timeout_exceeded=false
```

## Final counts before quality stack

```text
allowlisted_dynamic_script_import=0
allowlisted_legacy_mixed=0
test_architecture_violations=0
onion_violation_count=0
onion_allowed_violation_count=0
write_path_unknown=0
write_path_total_records=340
shared-state=4
```

## Residual risks to carry into closeout

1. Atomic-only cache coordination does not prevent duplicate same-key work.
2. Queue soak is bounded local SQLite process proof, not production-duration stress.
3. M170 closes bounded architecture backlog targets, not full repository strict architecture compliance.
