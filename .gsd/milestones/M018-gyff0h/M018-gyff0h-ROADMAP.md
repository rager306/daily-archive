# M018-gyff0h: ML Dependency Security Triage

**Vision:** Turn M017's broad dependency-audit debt into actionable, evidence-backed security triage without destabilizing the ML stack or relaxing Scientific KG safety gates.

## Success Criteria

- ML dependency vulnerability debt from M017 is assessed with evidence rather than ignored.
- The project knows whether torch/transformers findings are reachable in active runtime paths.
- A follow-up update/remove/isolate/defer plan exists and is safe to execute later.
- No existing KG or MiniMax safety boundary is weakened.

## Slices

- [x] **S01: S01** `risk:medium` `depends:[]`
  > After this: After S01, dependency/audit inventory exists with package versions, advisory summary, and sanitized tool evidence.

- [x] **S02: S02** `risk:medium` `depends:[]`
  > After this: After S02, repo import/use sites identify whether vulnerable ML packages are active runtime, dev/test, probe-only, or unused.

- [x] **S03: S03** `risk:medium` `depends:[]`
  > After this: After S03, the project has an actionable dependency security recommendation and R046 is validated or explicitly blocked.

## Boundary Map

Not provided.
