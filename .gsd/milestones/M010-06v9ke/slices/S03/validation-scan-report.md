# M010 validation scan report

## Inputs

- Batch state: `.gsd/milestones/M010-06v9ke/slices/S02/run-evidence/source-ready-batch-state.json`
- Active milestone id: `M010-06v9ke`
- Batch id: `m010-next-plus-ten-materialized`
- Quota gate: `True` with shortage `0`

## Scan result

- Paper count: `10`
- Chunk count: `1477`
- Outlier count: `7`
- Import-eligible chunk count: `0`
- Structure-aware baseline chunk delta: `-354`
- Mixed benchmark chunk delta: `-994`

## Provenance and freshness

- Valid run id: `m010-s03-scan-002`
- Freshness verdict: `fresh`
- Freshness diagnostics: `0`
- Expected metadata: `milestone_id=M010-06v9ke`, `batch_id=m010-next-plus-ten-materialized`

The first provenance attempt (`m010-s03-scan-001`) returned stale because expected metadata checks included JSONL diagnostics and the response wrapper. The accepted proof is `m010-s03-scan-002`, which covers metadata-bearing JSON outputs.

## Safety

- Raw text embedded: `false`
- Chunk text embedded: `false`
- Embeddings/vectors embedded: `false`
- Secrets embedded: `false`
- Optimizer traces embedded: `false`
- Production import attempted: `false`
- LadybugDB written: `false`
- Positive import allowed: `false`
- Semantic KG readiness claimed: `false`

## Recommendation

Proceed to independent review of S03 artifacts. This is operational scan evidence only; positive KG import and semantic KG readiness remain blocked.
