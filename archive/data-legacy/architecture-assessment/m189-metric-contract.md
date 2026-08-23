# M189 Metric Contract

## Verdict

**Metric contract established: real-corpus expansion must measure source, parser, chunk, extraction, evidence, and retrieval quality before any DSPy/RLM/optimizer or graph import claim.**

## Inputs

- M188 final readiness: `catalog_ready=true`, `intake_ready=true`, `source_boundary_ready=true` for tested M027 scope, `parser_ready=partial`, `chunk_ready=true` for M031 replay evidence scope, `graph_not_ready=true`.
- M189 metric baseline tests: extraction benchmark 6 passed; non-ablation evaluation metric tests 6 passed / 2 deselected.

## Required metric dimensions

| Dimension | Required measurement | Fail-closed condition |
|---|---|---|
| Source quality | substantive body availability, fallback reason, source type, conversion method | `low_quality_source`, `no_substantive_body`, missing local source bytes |
| Parser quality | parsed text/body length, structural section availability, parser errors | parser unavailable, empty body, parser exception, synthetic-only text |
| Chunk quality | chunk count, zero-chunk reason, evidence package coverage | zero chunks without typed reason, chunk package missing evidence path |
| Extraction quality | reviewed fixture precision/recall/F1 or equivalent gold comparison | invalid prediction schema, missing required entity/relation fields |
| Groundedness | evidence IDs expected vs returned, missing/unexpected evidence IDs | evidence path miss, ungrounded extracted claim |
| Retrieval quality | recall@k, returned IDs, missing IDs, duplicate handling | empty result without diagnostic, missing expected IDs, untraceable evidence path |
| Safety/governance | graph/import flags, production persistence flags, direct write checks | graph/import readiness true without proof, production write enabled, direct extractor-to-graph write |

## Minimum thresholds for future execution waves

These thresholds are gates for a future real-corpus expansion execution milestone, not claims that M189 met them on new corpus data:

- Every selected article has an explicit source quality label.
- Every low-quality source has a typed fallback reason.
- Parser-ready articles must produce substantive body text and structured diagnostics.
- Chunk-ready articles must have nonzero chunks or typed zero-chunk refusals.
- Extraction metrics must include schema validity and evidence grounding.
- Retrieval metrics must report expected IDs, returned IDs, missing IDs, and empty-result diagnostics.
- Any graph/import or production persistence flag remains false until a later milestone proves it explicitly.

## Required evidence bundle for future real-corpus expansion

A future execution wave must produce:

- selection manifest;
- source quality summary;
- parser quality summary;
- chunk evidence summary;
- extraction benchmark summary;
- retrieval ablation summary;
- low-quality source ledger;
- graph/import false-flag assertion;
- GitNexus detect_changes output;
- GSD validation artifact.

## Non-goals

This metric contract does not authorize:

- DSPy optimizer activation;
- RLM/hybrid retrieval production claims;
- graph import or LadybugDB production writes;
- direct extractor-to-graph writes;
- treating M188 `parser_ready=partial` as broad parser readiness;
- treating HTTP 200 or non-empty arXiv navigation markdown as source success.

## Promotion rule

A later milestone may only promote from readiness design to execution if it can cite this contract, run the representative metric tests, and define a bounded corpus selection with expected metric outputs before running any optimizer or graph import work.
