# M014 independent evidence review

## Verdict: PASS

The prior FLAG is resolved. The corrected M014 MiniMax evidence now includes the missing Token Plan weekly quota, peak-hour traffic-rule guidance, and the caveat that the current key’s active plan tier / purchase timestamp is unknown.

## Findings

- **Weekly quota detail present**
  - `token-plan-limits-report.md`, `token-plan-docs-summary.json`, and `token-plan-limits-guard.json` all state weekly usage quota is `10 × the 5-hour quota`.
  - The cutoff caveat is captured: purchases before `2026-03-22 23:59:59` are not subject; purchases from `2026-03-23 00:00:00` onward are subject.

- **Peak-hour traffic-rule detail present**
  - Report and guard artifacts include dynamic peak-hour controls, typical weekday peak window `15:00–17:30`, and approximate continuous-agent guidance:
    - Starter / Plus: about 1 continuous agent
    - Max: about 2 continuous agents
    - Ultra: about 4 continuous agents

- **Unknown current plan / purchase timestamp caveat present**
  - `current_key_plan_or_purchase_timestamp_known: false` appears in both the docs summary and limits guard.
  - The report correctly treats this as an operational constraint to verify in account UI/API before sustained use.

- **Evidence hygiene looks good**
  - JSON artifacts parse successfully.
  - No credential-looking values were detected.
  - Artifacts consistently mark raw responses, raw model content, prompt content, paper text, project text, embeddings, and vectors as not persisted.
  - The remains probe records only sanitized status/shape/hash metadata for the `403` response.

- **No material overclaiming found**
  - The `403` Token Plan remains probe is interpreted cautiously as likely unauthorized / not a Token Plan key, not as proof of actual quota state.
  - S02 guard keeps MiniMax scoped as helper-only, not orchestrator/source-of-truth/unattended batch use.
  - Schema reliability is appropriately caveated as usable only with local validation and retry.

## Risks

- Actual active plan tier, purchase timestamp, and current remaining quota are still unknown because the remains endpoint returned `403`.
- MiniMax dynamic traffic controls may change based on cluster load, so the captured peak-hour guidance should remain operational guidance, not a hard guarantee.
- S02 real helper evidence shows truncation/length edge cases, so downstream use still needs bounded retry and local schema validation.

## Recommendation

Accept the correction. Keep the guardrails as written: bounded live calls, redacted metadata only, local schema validation, response-hash-only artifacts, and no production/source-of-truth role until plan/quota state is verified through account UI or an authorized Token Plan key.
