# M012-a7v8fw: DSPy and MiniMax Compatibility Spikes

**Vision:** Retire compatibility and callability unknowns for DSPy and MiniMax before future semantic KG integration work, without enabling either tool in the production pipeline.

## Success Criteria

- Parallel DSPy and MiniMax compatibility research completed independently.
- At least one artifact each documents requirements, callability path, risks, and blockers.
- Combined matrix maps both technologies to current KG pipeline boundaries.
- Final recommendation gives separate go/no-go/precondition verdicts for DSPy and MiniMax.
- No production import, LadybugDB write, optimizer activation, or MiniMax orchestration occurs.

## Slices

- [x] **S01: S01** `risk:medium` `depends:[]`
  > After this: A DSPy compatibility report exists with package/API requirements, minimal invocation path, fail-closed optimizer policy, and exact blockers/preconditions.

- [x] **S02: S02** `risk:high` `depends:[]`
  > After this: A MiniMax compatibility report exists with API/auth/model/modalities, structured output feasibility, adapter options, and bounded call policy.

- [ ] **S03: Integration boundary matrix** `risk:medium` `depends:[S01,S02]`
  > After this: A combined integration matrix shows where DSPy and MiniMax could fit, what each requires, and which gates must pass before activation.

- [ ] **S04: Compatibility synthesis and recommendation** `risk:medium` `depends:[S03]`
  > After this: Final recommendation states whether DSPy and MiniMax are compatible enough for future milestones, which probes passed/blocked, and exactly what to build next.

## Boundary Map

| Area | In scope | Out of scope |
|---|---|---|
| DSPy | Current package/API research, dependency constraints, minimal local boundary/probe design, fail-closed optimizer policy | Enabling DSPy optimizers, production extraction, trusted fact creation |
| MiniMax | Current API/auth/model/modalities research, minimal non-production call probe if key is available, Marker/custom adapter feasibility | MiniMax as orchestrator/source of truth, unbounded calls, production repair decisions |
| Shared pipeline | Integration matrix, cost/rate/failure modes, artifact/redaction contract, no-write/no-import guard | Production LadybugDB writes, positive KG import, unattended scaling |
| Secrets | Use secure_env_collect if a live MiniMax probe is needed | Asking user to paste keys or committing secrets |
