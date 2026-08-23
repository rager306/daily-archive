# M176 Integrated Verification

## Verdict

**Integrated verification status: PASS.**

## Verification matrix

| Check | Result | Evidence |
|---|---|---|
| Focused inventory tests | PASS: 13 passed | `gsd_exec[b8854ca0-e8a0-4b98-a74c-4e9bd37d3b45]` |
| Test architecture guard | PASS: dynamic=0, legacy=0, violations=0 | `gsd_exec[635c0766-52ba-4dd7-a656-1d4e22dfb136]` |
| Onion guard | PASS: violation_count=0, allowed_violation_count=0 | `gsd_exec[e7c1bc98-25d1-4ec8-b5de-efba0c894845]` |
| Final artifact assertions | PASS: final counts and generated delta lines match | `gsd_exec[525a76bb-f6ad-4036-b58a-d00b96943c22]` |

## Final counts

```text
total_records=341
unknown=0
shared-state=0
script-only=235
m061-acquisition-pipeline-output=11
figure-extraction-benchmark-output=13
m028-acquisition-evidence-output=6
```

## Generated delta highlights

```text
m061-acquisition-pipeline-output +11
figure-extraction-benchmark-output +13
m028-acquisition-evidence-output +6
script-only -30
total delta +0
```

## Boundary checks

- Existing inventory JSON schema remains unchanged.
- Rules classify exact script source paths only.
- Generic script targets remain unclassified.
- M029, R024, architecture audit, and scanner self-output scripts remain `script-only`.
- `unknown=0` and `shared-state=0` remain true.
