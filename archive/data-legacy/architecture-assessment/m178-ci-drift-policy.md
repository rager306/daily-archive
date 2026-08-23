# M178 CI Drift Policy

## Verdict

**Policy upgraded.** Architecture guardrail CI now uses a strict drift mode when the committed M178 final inventory exists, while retaining preview mode before final inventory is generated inside this milestone.

## Behavior

- If `data/architecture-assessment/m178-write-path-inventory-final.json` exists, CI compares generated current inventory against it and asserts zero category drift.
- If final inventory does not exist yet, CI uses the M178 baseline and prints a preview without enforcing zero drift.
- CI always asserts `unknown=0` and `shared-state=0`.
- CI writes generated JSON, markdown, and delta markdown only to temporary files.

## Strict drift rule

In strict mode, generated delta must contain:

```text
Total delta: `+0`
all category delta rows end with `+0`
```

## Local pre-final smoke evidence

```text
mode=preview
script-only=170
unknown=0
shared-state=0
result=PASS
```

Evidence: `gsd_exec[f593fded-fd24-4f7f-b6e4-f7483850181d]`.

## Follow-up check

After S09 generates final inventory, S10 must rerun the same workflow-equivalent command and prove strict mode passes against final baseline.
