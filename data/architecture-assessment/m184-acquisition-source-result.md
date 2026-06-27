# M184 Acquisition Source Result

## Verdict

**Acquisition-source exact wave: PASS.**

## Movement

```text
script-only: 89 -> 79
source-acquisition-evidence-output: 0 -> 10
unknown=0
shared-state=0
total_records=341
```

## Category added

`source-acquisition-evidence-output` covers exact reviewed source paths only:

- `scripts/acquire_linked_target_pdfs.py`
- `scripts/acquire_m056_wave.py`
- `scripts/audit_m054_pdf_acquisition.py`
- `scripts/capture_m027_mixed_source_sources.py`
- `scripts/convert_m027_source_quality_boundary.py`
- `scripts/convert_m029_unified_source_quality_boundary.py`
- `scripts/emit_m056_candidate_edges.py`

## Verification

| Check | Result | Evidence |
|---|---|---|
| Fresh baseline | PASS | `gsd_exec[cf39c953-bbd5-41b2-aca6-a1ab72236bf4]` |
| Candidate write context extraction | PASS | `gsd_exec[ffd59946-cecc-45cb-94a7-8a860fa4e881]` |
| All candidate records current categories | PASS | `gsd_exec[afb23952-ff50-491c-9778-3dcf2b46477b]` |
| Focused tests after scanner movement | PASS: 33 passed | `gsd_exec[e1fde01c-7510-48e5-81b6-bd7257c24c17]` |
| Ruff scanner and tests | PASS | `gsd_exec[bf45db66-ace4-4e37-b787-b44ef66d5967]` |
| Generated delta before canonical refresh | PASS | `gsd_exec[c677a660-2318-49bb-88cf-26f9f08124d5]` |
| Canonical refresh, lowered ratchet, strict drift | PASS | `gsd_exec[06a2a013-725a-4375-bfb3-662407833e36]` |

## Guardrails

- No broad source/acquire/pdf/tmp/fd/output/report rule.
- No runtime code movement.
- Ratchet lowered to `script-only <= 79`.
- Canonical baseline refreshed.
