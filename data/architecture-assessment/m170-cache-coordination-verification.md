# M170 Cache Coordination Verification

## Verdict

**Cache coordination track: PASS.**

M170 closes same-key cache coordination as policy-only under the S05 atomic-only decision. No lock-file or compare-and-swap code was added.

## What was verified

| Area | Result | Evidence |
|---|---|---|
| CLI stable artifacts | PASS | `gsd_exec[0aa506b3-04f4-4323-8313-8728629dd9a8]` |
| PDF cache writes | PASS | `gsd_exec[0aa506b3-04f4-4323-8313-8728629dd9a8]` |
| Write-path inventory | PASS: unknown=0, shared-state=4 | `gsd_exec[1b0c5d85-a9f8-4033-bfb8-91bec72b4d2c]` |

Focused pytest target:

```text
uv run pytest \
  tests/test_analysis.py::test_s05_subprocess_same_date_rerun_overwrites_stable_paths \
  tests/test_pdf_downloader.py::test_download_writes_pdf_with_atomic_replacement \
  tests/test_pdf_downloader.py::test_download_rejects_non_pdf_response \
  -q
```

Result:

```text
3 passed
```

Inventory counts:

```text
total_records=339
unknown=0
shared-state=4
```

Generated:

```text
data/architecture-assessment/m170-write-path-inventory-cache-check.json
data/architecture-assessment/m170-write-path-inventory-cache-check.md
```

## Policy closure

Artifacts:

- `data/architecture-assessment/m170-cache-coordination-policy.md`
- `data/architecture-assessment/m170-cli-cache-coordination-result.md`
- `data/architecture-assessment/m170-pdf-cache-coordination-result.md`
- `data/architecture-assessment/m170-shared-state-review.md`

Decision:

- D092: keep atomic-only coordination for M170; defer lock/CAS until same-key multi-writer activation or stale-overwrite authority requirement exists.

## Residual risk

Atomic-only prevents partial final files. It does not prevent duplicate same-key work or last-writer-wins semantics. This is acceptable for M170 because there is no active requirement for exactly-once CLI/PDF cache population or stale-overwrite detection.

## Future activation triggers

A future milestone should add lock/CAS if:

1. high-concurrency CLI/PDF cache population is activated;
2. duplicate same-key PDF downloads become a measured operational problem;
3. scoring payloads for the same paper id can race with non-deterministic inputs;
4. cache consumers require stale-overwrite detection;
5. a checksum, generation id, or source revision becomes part of the cache authority contract.
