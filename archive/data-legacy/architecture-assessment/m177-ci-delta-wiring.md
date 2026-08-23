# M177 CI Delta Wiring

## Verdict

**CI wiring implemented and locally smoked.** The architecture guardrail workflow now runs a write-path inventory delta smoke step in the mandatory M044 job.

## Workflow behavior

The step:

1. Uses `data/architecture-assessment/m177-write-path-inventory-final.json` when present.
2. Falls back to `data/architecture-assessment/m177-write-path-inventory-baseline.json` during this milestone before final inventory exists.
3. Generates current JSON, markdown, and delta markdown into temporary files only.
4. Asserts `unknown=0` and `shared-state=0`.
5. Prints a short delta preview for CI observability.

## Why this is bounded

- No tracked generated artifacts are written by CI.
- The check reuses the existing scanner CLI and generated delta renderer.
- It avoids broad inventory arithmetic in workflow YAML.
- It remains cheap because it only scans local Python AST write calls.

## Local smoke evidence

```text
command: M177 S08 local CI delta smoke
result: PASS
script-only=198
unknown=0
shared-state=0
```

Evidence: `gsd_exec[1219871c-e5de-4462-9d0f-5670e1e9ab02]`.

## Limitations

This is a smoke and guardrail visibility step, not a full drift policy. Future milestones can ratchet it into a stricter baseline comparison once a stable canonical write-path inventory baseline is chosen.
