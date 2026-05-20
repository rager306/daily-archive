# M014-65dlgp: MiniMax Real Test and Token Plan Limits

**Vision:** Advance MiniMax from smoke-test callability to real bounded helper-readiness evidence, while documenting Token Plan usage limits and preserving Scientific KG safety boundaries.

## Success Criteria

- Token Plan limits and how to view usage are documented from MiniMax docs/current sources.
- Real MiniMax live tests run in bounded mode and do not persist raw response/model content.
- Budget is documented as non-blocking due to subscription, while platform limits remain respected.
- MiniMax remains helper-only, not orchestrator/source-of-truth.
- No positive KG import, trusted fact creation, or LadybugDB write occurs.

## Slices

- [x] **S01: S01** `risk:medium` `depends:[]`
  > After this: After S01, we know where and how to inspect Token Plan quotas/limits and current operational rules.

- [ ] **S02: MiniMax real bounded helper probes** `risk:medium` `depends:[S01]`
  > After this: After S02, MiniMax has been exercised with real bounded helper-style API calls and sanitized evidence.

- [ ] **S03: MiniMax real-test recommendation** `risk:medium` `depends:[S01,S02]`
  > After this: After S03, the project has a reviewed go/no-go recommendation for the next MiniMax helper integration step.

## Boundary Map

| Area | In scope | Out of scope |
|---|---|---|
| MiniMax API | Real bounded live calls with synthetic/redacted payloads | Raw paper/PDF/chunk payloads |
| Token Plan | Docs and API remains endpoint discovery/verification | Browser/manual account changes |
| Budget | Subscription budget not a blocking constraint | Unbounded concurrency or unattended scaling |
| KG | Helper-readiness evidence only | Trusted fact creation, KG import, LadybugDB writes |
