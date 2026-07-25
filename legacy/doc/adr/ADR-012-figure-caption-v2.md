# ADR-012: Figure Caption v2 via TeX Provenance for M058

**Status:** Accepted (binding)
**Date:** 2026-06-12
**Deciders:** agent
**Milestone:** M058-cmjp1u
**Scope:** parser-benchmark / graph-readiness / scientific-papers / figure-caption / plotextractor-v2
**Binding Level:** binding supplement to ADR-011
**Revisable:** yes, after M060 completes 2-hop BFS, fd production hardening, and ADR-002 GraphDB selection
**Supplements:** ADR-011 Content Graph via fd for M057

## 0. One-line Decision

> M058 accepts `figure_similarity_v2` as the TeX-provenance figure-caption supplement to the M057 content graph: the layer remains diagnostic-only, adds 15 normalized figure edges, and improves auditability through TeX labels and image paths.

Production import is disabled by this ADR. Graph writes, LadybugDB writes, fact promotion, external network calls, and LLM calls remain false.

## 1. Context

ADR-011 accepted the M057 content graph via fd as a diagnostic supplement to ADR-010. That graph combined 4454 citation edges, 4934 table-similarity edges, and 15 figure-similarity v1 edges for a total of 9403 edges. The v1 figure layer was useful, but it depended on regex-derived captions and did not provide TeX labels or image paths.

M058 S01 tested `plotextractor` v2 on a 5-PDF pilot. The run processed 5 local TeX sources, extracted 104 figures and 104 captions, and produced 15 inter-document figure-similarity edges. The v2 layer preserved the same safety posture as M057 while adding better provenance: label availability was 0.990385 and image path availability was 0.605769.

M058 S02 tested Marker stage 1 and produced a separate NO-GO for Marker scale-up. That Marker decision does not invalidate S01. It only means page-limited Marker evidence is insufficient for S03/S04 expansion.

## 2. Decision

We will treat M058 figure-caption v2 as an accepted diagnostic evidence layer and include it in the M058 combined graph manifest.

The accepted diagnostic graph now combines four evidence layers:

- `citation`: 4454 edges from `artifacts/m056-bfs-graph/candidate-edges.json`.
- `table_similarity`: 4934 edges from `artifacts/m057-fd-marker/table-similarity/edges.json`.
- `figure_similarity_v1`: 15 edges from `artifacts/m057-fd-marker/figure-links/edges.json`.
- `figure_similarity_v2`: 15 edges from `artifacts/m058-plotextractor/edges.json`.

The normalized edge schema is:

`{source_paper_id, source_artifact_type, source_artifact_idx, target_paper_id, target_artifact_type, target_artifact_idx, similarity_score, evidence_layer, evidence_id}`

The canonical M058 output artifacts are:

- `artifacts/m058-pilot/combined-edges.json`
- `artifacts/m058-pilot/per-layer-summary.json`
- `artifacts/m058-pilot/REPORT.md`

`figure_similarity_v2` is accepted because it improves provenance, not because it increases graph volume. Its 15 edges are the same order of magnitude as v1, but the TeX labels and image paths give later agents a stronger audit path from graph edge back to source evidence.

All five safety defaults stay false:

- `graph_writes_authorized`: false
- `production_import_authorized`: false
- `fact_promotion_authorized`: false
- `external_network_authorized`: false
- `llm_calls_authorized`: false

## 3. Consequences

M058 combined graph evidence is now 9418 normalized edges across four layers. The graph remains diagnostic-only and is not a production import candidate.

M060 can start from a stronger evidence base than M057 because the figure-caption layer now has TeX provenance. M060 should focus on 2-hop BFS, fd production hardening, and ADR-002 GraphDB selection instead of trying to rescue Marker scale-up from page-limited evidence.

Marker stage 2 and stage 3 are not authorized by this ADR. S03 and S04 are treated as cancelled per S02 gate, not as completed full-document evidence.

## 4. Status

Accepted as a binding supplement to ADR-011.

This ADR accepts the v2 figure layer and stops Marker scale-up for M058. It does not authorize production import, graph writes, fact promotion, external network calls, or LLM calls.

## 5. Date and Deciders

Date: 2026-06-12.

Decider: agent, based on M058 S01/S02 artifacts and S05 synthesis.

## 6. Safety Defaults

External network is not authorized. Graph writes is disabled. LadybugDB writes is disabled. Fact promotion is not authorized. LLM calls is disabled. Production import is disabled.

The M058 scripts bind local services to `127.0.0.1` when a loopback host is needed.
