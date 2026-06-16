# M069 S02 T01 Metrics Source Notes

## Purpose

Collect source evidence for Agents-K1 benchmark and metric definitions, then record what is verified vs still unknown before daily-archive defines a local evaluation contract.

## Sources checked

| Source | Status | Local evidence |
|---|---|---|
| arXiv HTML `https://arxiv.org/html/2606.13669v1` | reachable | `source-extracts/*.txt` |
| Section 7.5 multi-hop QA metric text | verified | `source-extracts/contain-acc-occ2.txt` |
| Section 7.6 information extraction evaluation | partially verified | `source-extracts/7-6-information-extraction-backbone-evaluation-occ2.txt` |
| Saved M069 overview | available | `PAPER_SUMMARY.md` |
| Gap analysis | available | `GAP_ANALYSIS.md` |

## Verified metric definitions from source

### Containment Accuracy or Contain-Acc

Verified text from local extract:

> Containment Accuracy (Contain-Acc.) measures whether the exact answer string is present within the generated response to assess retrieval precision.

Implication: this is a string containment metric. It is simple and reproducible, but brittle for paraphrases, aliases, units, and scientific notation.

### GPT-Judge Accuracy or GPT-Acc

Verified text from local extract:

> GPT-Judge Accuracy (GPT-Acc.) utilizes a large language model (GPT-4o-mini) to evaluate the semantic correctness of the generated answer compared to the ground truth.

Implication: semantic correctness is judged by an LLM. For daily-archive this must be treated as a judge with its own prompt, model ID, cost, and reproducibility risks.

### Information extraction F1

Verified from the paper summary and Section 7.6 extraction snippet:

- The paper evaluates NER, relation extraction, and structured NER.
- It reports average F1 over 10 datasets and 12,078 instances.
- The reported comparison includes Qwen3-4B, Qwen3-8B, Qwen3-32B, and the GRPO-trained 4B model.
- The paper reports remaining gap on relation extraction.

For daily-archive this means we need separate metrics for:

- entity extraction F1,
- relation extraction F1,
- structured extraction validity,
- relation/hyperedge quality.

## Verified benchmark classes from source summary

| Benchmark class | Verified use | daily-archive relevance |
|---|---|---|
| FrontierScience-Research | research QA accuracy by scientific domain | possible later but too broad for first local benchmark |
| Geoscience Research Questions | rationale and answer accuracy | useful pattern for domain-specific scientific QA |
| Multi-hop QA | Contain-Acc and GPT-Acc over HotpotQA, 2WikiMultiHopQA, MuSiQue | directly relevant for graph traversal evaluation |
| Information Extraction | F1 over NER, RE, structured NER | directly relevant for DSPy and MiniMax extraction |

## Unknowns that block direct reuse

The available extract does not yet provide enough detail for exact reproduction of Agents-K1 evaluation:

- Exact GPT-Acc judge prompt and rubric.
- Whether GPT-Acc uses deterministic settings, multiple judges, or calibration.
- Text normalization for Contain-Acc: case, punctuation, aliases, numeric formats, units, multiple correct answers.
- Entity normalization for NER F1.
- Relation matching rules for relation extraction F1.
- Structured NER schema validation rules.
- How n-ary or hyperedge facts are scored.
- Confidence interval or bootstrap variance reporting.
- Cost and latency accounting.

## daily-archive constraints

- No local GPU: do not plan Qwen3-4B or GRPO training.
- DSPy optimization must wait until metrics and fixtures exist.
- MiniMax calls must be measured for cost, latency, JSON validity, and schema validity.
- MiniMax-M3 multimodal remains diagnostic-only unless a later milestone explicitly expands authorization.
- Production graph writes and fact promotion stay disabled.

## Metric takeaway

For M069, the minimum useful metric contract is not just F1. It must include:

1. extraction quality,
2. relation quality,
3. structured JSON/schema validity,
4. evidence-path validity,
5. research QA answer quality,
6. cost and latency,
7. failure diagnostics.

This should become the gate before any DSPy or MiniMax prompt optimization work.
