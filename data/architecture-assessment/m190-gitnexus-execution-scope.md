# M190 GitNexus Execution Scope

## Verdict

**M190 scope is bounded real-corpus metrics execution using existing local validators and tests. It does not enable DSPy/RLM optimizer work, graph import, or production persistence.**

## GitNexus planning basis

Primary query:

`bounded real corpus metrics execution selection expected metric outputs source parser chunk extraction retrieval ablation low quality source ledger`

GitNexus surfaced execution-relevant flows:

| Flow | GitNexus symbol | M190 use | Boundary |
|---|---|---|---|
| M027 current pipeline replay | `Function:scripts/replay_m027_current_pipeline_baseline.py:replay_baseline` | Candidate source/parser/current-pipeline evidence surface. | Writes local replay artifacts; no graph or production persistence claims. |
| M029 unified loader runtime smoke | `Function:scripts/run_m029_unified_loader_runtime_smoke.py:run` | Candidate runtime loader evidence surface. | Local runtime smoke; writes local summaries only. |
| Chunk baseline measurement | `Function:src/research_graph/infrastructure/repair/chunk_baseline_measurement.py:build_baseline_package` | Candidate chunk measurement/readiness surface. | Use through existing tests/artifacts unless future exact impact allows source edits. |
| Low-quality source criteria | `tests/test_m055deep_grobid_fulltext.py` low-quality source tests and M055 opendataloader criteria | Guardrail for source-quality labels. | Low-quality sources remain fail-closed; HTTP 200 or non-empty navigation markdown is not success. |

## Execution stance

M190 is allowed to:

- define a bounded local corpus selection;
- write expected metric outputs before execution;
- run existing local validators and representative tests;
- summarize observed outputs against expected outputs;
- update generated evidence artifacts if a verifier explicitly owns them.

M190 is not allowed to:

- edit functions/classes/methods without exact GitNexus impact first;
- enable DSPy/RLM optimizer behavior;
- import graph state;
- write LadybugDB or production persistence state;
- claim production hybrid retrieval quality;
- classify low-quality source as success by omission.

## Candidate execution gates

- Article catalog verifier.
- M030 requested-ref intake validate-only.
- M027 source boundary verifier.
- Extraction benchmark tests.
- Evaluation benchmark tests.
- DSPy boundary tests.
- Focused low-quality source tests.
- GitNexus detect_changes scoped to `daily-archive`.

## Scope decision

Proceed to S01 T02 by choosing the bounded selection from existing local corpus artifacts and mapping exact commands. Then S02 must write expected outputs before S03 executes the bounded gates.
