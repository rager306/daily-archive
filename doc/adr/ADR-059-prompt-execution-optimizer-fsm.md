# ADR-059: Provider-neutral Prompt Execution and Optimizer Candidate FSM

**Status:** Accepted  
**Date:** 2026-08-12  
**Depends on:** ADR-051, ADR-053, ADR-055  
**Decisions:** D136, D238

## Context

The Rust runtime has two active generative paths. `SummaryUseCase` uses a text
`LlmClient` with an inline prompt and incomplete prompt/model lineage before
writing derived graph summaries. Figure analysis is safer: multimodal requests
already bind prompt, schema, route, model, token, latency and candidate-only
provenance. GEPA/DSPy cannot safely optimize inconsistent call surfaces, and
RuVector retrieval tuning is a separate problem.

## Decision

All active generative calls must execute through a provider-neutral
`PromptExecutionPort`. Application use cases receive a typed `PromptTaskSpec`;
adapters render templates, resolve provider/model routes from strict DATA plus
environment configuration, execute providers and return complete
`PromptExecutionLineage`.

Prompt task policy lives in `data/ontology/prompt_tasks.yaml` and binds:

- task/version and system/user templates;
- output schema and route policy identities;
- canonical `models.yaml` binding ID, temperature/max-token bounds and eval suite;
- `optimizer_enabled=false` until D136 evidence is current.

Model/provider/protocol selection comes from `models.yaml` roles. For the default
summary generator this is `paper-summary-generate-default` routed through the
OpenAI-compatible 9router proxy. Environment configuration owns only host/key
and operational timeout overrides (`NINEROUTER_URL`, `NINEROUTER_KEY`,
`DA_LLM_TIMEOUT_SECONDS`). Application code must not own provider defaults.

Optimizer candidates follow an explicit FSM:

```text
baseline_registered
  -> candidate_proposed
  -> evaluated
  -> eligible | rejected
```

There is no `deployed` transition. Eligibility is candidate evidence, not
promotion. Deployment requires a later explicit human decision and remains
outside this FSM. Every execution/candidate is non-authoritative and
`import_eligible=false`.

## Evaluation gates

Evaluation must use independent frozen fixtures and require:

- exact prompt/schema/route/model/input/output digests;
- nonzero same-n baseline and candidate sets;
- bounded validation gap and no contamination;
- hard-invariant, latency and cost budget pass;
- task-specific grounding, faithfulness, coverage and paraphrase metrics;
- dual entity/relation F1 plus semantic graph tasks where extraction is involved.

A candidate cannot score itself using the same implementation as its oracle.

## Architectural boundaries

- **da-domain:** task, lineage, evaluation and FSM value contracts.
- **da-ports:** `PromptExecutionPort` request/response boundary.
- **da-application:** orchestration and graph-write policy after validated lineage.
- **da-adapters:** DATA loader, template rendering and provider adapters.
- **da-cli:** `.env`, registry path, provider composition and offline evaluation.

RuVector remains a retrieval/vector backend. It is benchmarked through
recall/MRR/nDCG/filter/latency evidence and is never treated as a prompt
optimizer. GEPA/DSPy may later propose candidates but receive no graph, import or
deployment authority.

## Alternatives rejected

1. Inline prompts per use case: no stable optimization or cache identity.
2. Provider-specific application interfaces: violates hexagonal boundaries.
3. Auto-promote the best metric: permits overfit, contamination and regressions.
4. Use output SHA alone: different prompts/models may emit identical bytes.
5. Enable DSPy before fixtures: violates D136.

## Acceptance evidence

1. Strict registry loader and change-sensitive digest.
2. FSM property/negative tests for illegal transitions, same-n, validation gap,
   contamination, budgets, deployment and D127.
3. Summary and figure calls use `PromptExecutionPort` with complete lineage.
4. Prompt/model/schema drift invalidates cache identity.
5. Frozen offline evaluation emits candidate reports only.
6. Whole-pipeline metrics capture tokens, latency, cost and failures.
7. `optimizer_enabled=false`, `deployed=false`, `import_eligible=false`.
