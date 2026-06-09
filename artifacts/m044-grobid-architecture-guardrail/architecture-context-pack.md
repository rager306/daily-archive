# M044 Sidecar Architecture Context Pack

- Pack ID: `m044-sidecar-architecture-context-v1`
- Candidate sidecars only: true
- Graph writes: disabled
- Production import: disabled
- Fact promotion: disabled

## Mandatory decisions

| Decision | Rule |
|---|---|
| M033 | Use bounded combined sidecar architecture; external parsers are candidate evidence only, not parser replacement or import path. |
| ADR-003 | Use durable lazy async evidence pipeline direction; do not rely on in-memory batch or agent runtime as reliability model. |
| ADR-004 | Treat GROBID, OpenDataLoader, Adaptix, and future extractors as candidate evidence producers only. |
| ADR-005 | No direct extractor/sidecar/LLM/parser writes to GraphDB; import/promotion remains blocked without future ADR gate. |
| ADR-007 | Use quant-mind as architecture pattern source only; do not adopt quant-mind runtime dependency now. |
| D078 | Represent combined sidecar outputs as candidate-only sidecar comparison packets with ready/replay/blocker statuses and false graph/import/promotion flags. |

## Required systems

- `current_baseline`
- `grobid`
- `opendataloader_pdf`
- `adaptix`
- `quant_mind_patterns`
- `combined_architecture`

## Prohibited claims

- `graph_import_authorized`
- `production_import_authorized`
- `fact_promotion_allowed`
- `sidecar_success_as_semantic_truth`
- `quant_mind_runtime_adopted`
- `raw_payload_promoted`

## Preflight command

```bash
uv run python scripts/verify_m044_sidecar_architecture_guardrail.py
```
