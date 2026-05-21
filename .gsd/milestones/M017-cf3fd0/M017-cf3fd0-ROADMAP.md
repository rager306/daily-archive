# M017-cf3fd0: MiniMax Safe Helper Implementation

**Vision:** Turn proven MiniMax findings into safe, reusable, dev-only project helper code before returning to KG semantic readiness.

## Success Criteria

- Manus research is incorporated before code design is finalized.
- MiniMax limit helper implements verified endpoint/auth/count semantics.
- Structured helper requires forced-tool schema validation and local checks.
- No KG import/write/source-of-truth path is enabled.
- All evidence is sanitized and reproducible.

## Slices

- [x] **S01: S01** `risk:medium` `depends:[]`
  > After this: After S01, M017 has an evidence-backed design adjusted for Manus findings without bypassing local gates.

- [x] **S02: S02** `risk:medium` `depends:[]`
  > After this: After S02, the project has a tested dev-only MiniMax limit helper contract.

- [ ] **S03: MiniMax structured helper boundary** `risk:medium` `depends:[S01]`
  > After this: After S03, structured MiniMax helper output has a safe local wrapper contract.

- [ ] **S04: MiniMax helper safety review** `risk:low` `depends:[S02,S03]`
  > After this: After S04, M017 has a final guard and go/no-go recommendation for future KG work.

## Boundary Map

| Area | In scope | Out of scope |
|---|---|---|
| MiniMax docs | Official docs, global skill, Manus research synthesis | Treating external research as authoritative without local verification |
| Helper code | Dev-only limit checker and structured helper wrapper | Production pipeline activation |
| KG | Safety flags preventing import/write | Trusted fact creation or LadybugDB writes |
| Secrets | Env lookup and redacted diagnostics | Logging or persisting token values |
