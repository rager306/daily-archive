# M013-tdtle0: DSPy Optimizer and MiniMax Probe Deepening

**Vision:** Deepen the M012 compatibility work into concrete DSPy optimizer applicability and dependency evidence, while safely advancing MiniMax only through a bounded synthetic smoke-test decision.

## Success Criteria

- DSPy isolated dependency/no-LM probe completed or precisely blocked.
- DSPy optimizer catalog with applicability ratings completed.
- MiniMax synthetic smoke-test decision/probe completed or precisely blocked.
- Final recommendation separates infrastructure readiness from KG import readiness.
- No production import/write/optimizer/MiniMax orchestration occurs.

## Slices

- [x] **S01: S01** `risk:medium` `depends:[]`
  > After this: An isolated DSPy dependency probe artifact reports install/import/no-LM status and whether Predict/Evaluate can be exercised without external LM calls.

- [x] **S02: S02** `risk:medium` `depends:[]`
  > After this: A DSPy optimizer catalog explains each optimizer family and rates applicability to daily-archive with exact preconditions and blocked uses.

- [x] **S03: S03** `risk:high` `depends:[]`
  > After this: A MiniMax synthetic smoke-test artifact records whether a live synthetic call was run or intentionally deferred, with exact auth/header/schema findings if run.

- [ ] **S04: DSPy MiniMax adoption recommendation** `risk:medium` `depends:[S01,S02,S03]`
  > After this: Final recommendation states the exact next safe step for DSPy and MiniMax, and whether any work can proceed in parallel with chunk-span packet work.

## Boundary Map

| Area | In scope | Out of scope |
|---|---|---|
| DSPy dependencies | Isolated/temp environment install/import/no-LM probe, dependency inventory, failure modes | Adding DSPy to project runtime dependencies without a later decision |
| DSPy optimizers | Catalog optimizers from local source/GitNexus/docs, classify applicability to KG extraction and required metrics | Running optimizers, calling external LMs, producing optimized prompts |
| MiniMax | Synthetic bounded smoke-test decision and optional explicit live test if already-authorized and safe | Raw paper/PDF calls, MiniMax orchestration/source-of-truth |
| KG process | No-import/no-write guard and next safe recommendation | Positive KG import, production LadybugDB writes, unattended scaling |
