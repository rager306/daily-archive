# M069 S02 T02 daily-archive Benchmark Contract

## Purpose

Define the minimum metrics and benchmark shape required before future DSPy + MiniMax extraction work. This contract adapts Agents-K1's metric patterns to daily-archive without requiring local GPU or Qwen3-4B/GRPO training.

## Non-goals

- No production graph writes.
- No fact promotion.
- No DSPy optimization yet.
- No local Qwen3-4B or GRPO training.
- No claim that daily-archive matches Agents-K1 benchmark results.

---

## Benchmark phases

### Phase 0: fixture and gold-set creation

Before any optimizer runs, create a reviewed local benchmark over canonical PDFs.

Recommended initial size:

| Split | Size | Source |
|---|---:|---|
| Train examples | 30 papers | M061 canonical papers, diverse categories |
| Validation examples | 10 papers | held out from train |
| Smoke examples | 3 papers | fast local CI or dev verification |

Each example should include:

- paper ID and canonical PDF path,
- source text/artifact references,
- gold entities,
- gold relations,
- gold evidence paths,
- optional research QA question and expected answer,
- notes on ambiguity.

### Phase 1: extraction metrics

| Metric | Definition | Required diagnostic |
|---|---|---|
| Entity F1 | precision/recall/F1 over normalized entity candidates | per type: Task, Method, Dataset, Metric, Limitation, Claim |
| Relation F1 | precision/recall/F1 over normalized relation tuples | per relation type and evidence path |
| Structured validity | percent outputs parse as JSON and match schema | parse errors, missing fields, extra fields |
| Evidence-path validity | percent claims with valid source artifact/span references | missing source, stale path, ambiguous span |
| Hyperedge validity | percent n-ary claims represented without endpoint loss | arity, missing participants, wrong role |

### Phase 2: research QA metrics

Adapt Agents-K1's multi-hop metrics:

| Metric | daily-archive definition | Caveat |
|---|---|---|
| Contain-Acc | exact normalized answer string appears in generated answer | brittle; use only as a lower-bound metric |
| GPT-Acc style judge | LLM judge compares generated answer with gold answer and evidence | must record judge model, prompt hash, seed/settings, cost |
| Evidence coverage | answer cites required evidence nodes or artifacts | required for scientific trust |
| Multi-hop path validity | answer path traverses required paper/evidence/concept chain | needed for graph reasoning claims |

### Phase 3: operational metrics

| Metric | Why |
|---|---|
| Cost per paper | DSPy/MiniMax optimization can silently become expensive |
| Latency per paper | queue scheduling and throughput planning |
| Retry count | service reliability and circuit-breaker tuning |
| JSON parse failure rate | structured extraction reliability |
| Schema invalid rate | schema drift detection |
| Empty/low-quality output rate | catches silent failure modes |

---

## DSPy + MiniMax gate

DSPy + MiniMax work is allowed only after all of these are true:

1. benchmark fixtures exist,
2. metric functions exist or are specified in executable pseudocode,
3. evaluation can run without production writes,
4. cost and latency are logged,
5. schema invalid outputs fail visibly,
6. outputs are written as research artifacts only,
7. secrets are read from environment only.

Allowed optimizers for the first spike:

- `BootstrapFewShot`
- `MIPRO`
- `BootstrapRandomSearch`

Disallowed for current constraints:

- Qwen3-4B local inference or training,
- GRPO training,
- production write eligibility,
- fact promotion from optimized outputs.

---

## MiniMax evaluation dimensions

Future S03 must compare at least two paths:

| Path | Use | Expected tradeoff |
|---|---|---|
| MiniMax text extraction | schema/entity/relation extraction from text | lower cost, loses image-specific evidence |
| MiniMax-M3 multimodal judge | figure/table/equation diagnostics | richer evidence, higher cost and diagnostic-only status |

Each run must report:

- model ID,
- prompt/program version,
- input token estimate if available,
- output token estimate if available,
- cost estimate,
- latency,
- retry count,
- parse validity,
- schema validity,
- extraction metrics,
- evidence-path metrics.

---

## Stop conditions

Stop the future DSPy/MiniMax spike if any of these occur:

- schema invalid rate > 20% after one repair attempt,
- cost per paper exceeds the predeclared budget,
- evidence-path validity < 70% on validation set,
- relation F1 does not improve over baseline prompt,
- MiniMax API failures prevent stable evaluation,
- outputs include secrets or unverifiable source claims,
- optimized prompts rely on validation labels.

---

## Baseline before optimization

Before running any DSPy optimizer, record baseline results for:

1. hand-written MiniMax text prompt,
2. existing MiniMax-M3 diagnostic judge where applicable,
3. deterministic parser-only extraction where applicable.

DSPy improvement must be measured against these baselines, not against subjective quality.

---

## Suggested artifact layout for future work

```text
artifacts/m069-agents-k1-research/
  benchmark-contract.md
  benchmark-fixtures/
    train.jsonl
    validation.jsonl
    smoke.jsonl
  eval-runs/
    baseline-minimax-text/
    baseline-minimax-m3/
    dspy-bootstrapfewshot/
    dspy-mipro/
  reports/
    cost-latency-quality.md
    schema-validity.md
    evidence-path-validity.md
```

---

## M064 queue implications

The queue should be able to carry benchmark metadata, not just graph payloads:

- `schema_version`
- `metric_bundle_id`
- `extractor_version`
- `prompt_program_hash`
- `source_artifact_refs`
- `evidence_path_refs`
- `cost_estimate`
- `latency_ms`
- `write_eligibility=false`
- `promotion_eligibility=false`

If M064 cannot carry these fields, it should be adjusted before execution.

---

## Contract verdict

M069 S03 may reassess M064 using this contract. Future DSPy + MiniMax implementation should be a separate spike after M069 closes or after the user explicitly authorizes it.
