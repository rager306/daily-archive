# M189 Ablation Protocol

## Verdict

**Ablation protocol established: future real-corpus expansion must compare deterministic modes, report missing IDs and evidence paths, and stop before optimizer or graph-import claims unless all metric gates pass.**

## Inputs

- Metric contract: `data/architecture-assessment/m189-metric-contract.md`
- Ablation gate baseline: retrieval ablation tests 2 passed / 6 deselected; DSPy boundary tests 9 passed.
- M188 readiness: `parser_ready=partial`, `chunk_ready=true` for M031 replay evidence scope, `graph_not_ready=true`.

## Required comparison modes

| Mode | Description | Allowed in next execution wave? | Notes |
|---|---|---:|---|
| deterministic source baseline | Source/parser/chunk evidence without retrieval augmentation | yes | Must include source quality and zero-chunk diagnostics. |
| vector-only fixture baseline | Caller-provided fixture vectors and expected IDs | yes | Deterministic fixture mode only. |
| hybrid fixture baseline | Existing fixture-level hybrid retrieval comparison | yes | Not a production retrieval claim. |
| graph import mode | Import or persist extracted graph state | no | Requires a later graph-readiness milestone. |
| DSPy optimizer mode | DSPy tuning or optimizer execution | no | Requires metrics and ablation evidence from a bounded corpus first. |
| production persistence mode | LadybugDB or production corpus writes | no | Out of scope until explicit persistence readiness proof. |

## Required ablation outputs

Every future ablation run must emit:

- run ID and corpus selection ID;
- mode name and deterministic seed/config;
- expected IDs;
- returned IDs;
- missing IDs;
- unexpected IDs;
- duplicate IDs;
- empty-result diagnostics;
- evidence path hits and misses;
- source quality labels;
- parser/chunk readiness labels;
- low-quality source ledger;
- graph/import/prod-persistence false-flag assertion.

## Stop conditions

Stop the execution wave and do not promote if any are true:

- source quality labels are missing;
- low-quality source lacks a typed fallback reason;
- parser-ready article has no substantive body text;
- chunk-ready article has zero chunks without typed refusal;
- extraction output has invalid schema;
- evidence path hit rate cannot be computed;
- retrieval expected IDs are absent from the ablation manifest;
- empty retrieval results lack diagnostics;
- graph/import or production persistence flag is true without explicit graph-readiness milestone proof;
- DSPy optimizer is invoked before this protocol has passing execution evidence.

## Promotion criteria for a later execution milestone

A future milestone may move from design to bounded execution only if it:

1. cites the M189 metric contract and this ablation protocol;
2. defines a bounded corpus selection before running;
3. writes expected metric outputs before running;
4. runs representative tests from M189 first;
5. records GitNexus detect_changes scoped to `daily-archive`;
6. preserves `graph_not_ready=true` unless graph readiness is explicitly proven in that future milestone.

## Non-goals

This protocol does not authorize:

- DSPy/RLM optimizer activation;
- production hybrid retrieval claims;
- graph import;
- LadybugDB production writes;
- direct extractor-to-graph writes;
- replacing fail-closed gates with warnings.
