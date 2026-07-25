# ADR-011: Content Graph via fd for M057

**Status:** Accepted (binding)
**Date:** 2026-06-11
**Deciders:** agent
**Milestone:** M057-s70wkm
**Scope:** parser-benchmark / graph-readiness / scientific-papers / content-graph / fd-embeddings
**Binding Level:** binding supplement to ADR-010
**Revisable:** yes, after M059 completes chart extraction, Marker re-extraction, and 2-hop BFS/content graph evaluation
**Supplements:** ADR-010 BFS Scale Evidence from 167-PDF 1-hop Run

## 0. One-line Decision

> M057 accepts content-graph v1 via fd as supplementary evidence to the M056 citation graph: citation, table_similarity, and figure_similarity are combined into one diagnostic graph manifest, with OpenDataLoader tables treated as the primary content evidence layer.

Production import is disabled by this ADR. Graph writes, LadybugDB writes, fact promotion, external network calls, and LLM calls remain false.

## 1. Context

ADR-010 concluded that M056 was sufficient parser-scale evidence but insufficient graph-readiness evidence. The 1-hop BFS run from anchor `2605.18747` produced 149 unique PDFs and 4454 citation candidate edges, but citation-only overlap was too weak to justify graph-readiness on its own.

M057 fills that gap with a first content-graph pass. It validates local fd embeddings at `http://127.0.0.1:8000`, builds table-similarity evidence from OpenDataLoader tables, builds figure-similarity evidence from figure captions, and then normalizes those layers together with the M056 citation layer.

The evidence produced by M057 is diagnostic. It is intended to decide whether content layers are worth carrying into the next graph-readiness milestone, not to authorize production import.

## 2. Decision

We will treat M057 content-graph v1 as accepted supplementary graph-readiness evidence.

The accepted diagnostic graph combines three evidence layers:

- `citation`: 4454 edges from `artifacts/m056-bfs-graph/candidate-edges.json`.
- `table_similarity`: 4934 edges from `artifacts/m057-fd-marker/table-similarity/edges.json`.
- `figure_similarity`: 15 edges from `artifacts/m057-fd-marker/figure-links/edges.json`.

The normalized edge schema is:

`{source_paper_id, source_artifact_type, source_artifact_idx, target_paper_id, target_artifact_type, target_artifact_idx, similarity_score, evidence_layer, evidence_id}`

OpenDataLoader tables are the primary content evidence layer for this decision because they produced 1468 embedded tables and 4934 similarity edges, including 2591 inter-doc edges. Figure similarity is accepted as a low-volume supplementary layer because it produced 15 inter-doc edges and demonstrates that the same graph contract can carry visual/caption evidence.

All five safety defaults stay false:

- `graph_writes_authorized`: false
- `production_import_authorized`: false
- `fact_promotion_authorized`: false
- `external_network_authorized`: false
- `llm_calls_authorized`: false

## 3. Consequences

Graph-readiness gate v1 is unlocked for diagnostic use. Future graph-readiness work should not evaluate the corpus only through citation overlap; it must include content-derived evidence layers, especially table similarity.

The next milestone should be M059 or its equivalent follow-up:

- chart extraction via PlotExtract;
- Marker environment repair for `transformers.onnx`;
- Marker re-extraction and comparison against OpenDataLoader;
- 2-hop BFS/content graph evaluation to test whether a larger expansion improves internal connectivity.

This ADR does not permit production graph writes. Production import is disabled. LadybugDB writes are not authorized. Fact promotion is disabled.

## 4. Status

Accepted (binding).

This ADR is binding as a supplement to ADR-010. It changes the graph-readiness interpretation from citation-only evidence to citation plus content evidence, while preserving the same safety posture.

## 5. Date and Deciders

- Date: 2026-06-11
- Deciders: agent

## 6. Safety Defaults

The binding safety defaults for this ADR are:

```json
{
  "graph_writes_authorized": false,
  "production_import_authorized": false,
  "fact_promotion_authorized": false,
  "external_network_authorized": false,
  "llm_calls_authorized": false
}
```

The diagnostic fd endpoint is `http://127.0.0.1:8000`. The endpoint is local-only and does not authorize external network calls.
