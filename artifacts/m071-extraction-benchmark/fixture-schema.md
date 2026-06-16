# M071 Extraction Benchmark Fixture Schema

## Purpose

Define a deterministic, metadata-only benchmark fixture format for future DSPy + MiniMax extraction work. Fixtures are local test data; they do not authorize production graph writes, fact promotion, or external model calls.

## Safety rule

Fixtures must be **metadata-only**:

- allowed: IDs, normalized labels, types, roles, evidence refs, metric numbers, diagnostic codes;
- disallowed: raw article text, raw prompts, embeddings, vectors, API keys, secrets, credentials, full model payloads.

## File layout

```text
artifacts/m071-extraction-benchmark/fixtures/
  smoke-gold.jsonl
  smoke-predictions.jsonl
  smoke-expected-metrics.json
```

## JSONL record shape

Each line is one benchmark case.

```json
{
  "case_id": "case:perfect",
  "paper_id": "arxiv:2606.13669v1",
  "source_artifact_refs": ["artifact:paper-2606.13669v1"],
  "entities": [
    {"id": "entity:method:agents_k1", "type": "Method", "label": "Agents-K1", "evidence_refs": ["evidence:perfect:method"]}
  ],
  "relations": [
    {"id": "relation:agents_k1:uses:mineru", "type": "USES_COMPONENT", "source": "entity:method:agents_k1", "target": "entity:tool:mineru", "evidence_refs": ["evidence:perfect:relation"]}
  ],
  "schema_valid": true,
  "json_valid": true,
  "operational": {"cost_estimate": 0.0, "latency_ms": 0, "retry_count": 0}
}
```

## Entity rules

Required fields:

- `id`: metadata code/ref-like string, stable within fixture;
- `type`: controlled entity type such as `Method`, `Task`, `Dataset`, `Metric`, `Limitation`, `Claim`, `Tool`;
- `label`: normalized label, not raw source text;
- `evidence_refs`: list of metadata refs.

Entity matching key for metric v1:

```text
(type, normalized label)
```

## Relation rules

Required fields:

- `id`: stable relation ID;
- `type`: relation type such as `USES_COMPONENT`, `APPLIED_TO`, `EVALUATED_ON`, `MEASURED_BY`, `HAS_LIMITATION`, `SUPPORTS`, `CONTRASTS`;
- `source`: entity ID;
- `target`: entity ID;
- `evidence_refs`: list of metadata refs.

Relation matching key for metric v1:

```text
(type, source entity normalized key, target entity normalized key)
```

## Evidence-path validity

A prediction has valid evidence if every predicted entity and relation has at least one `evidence_ref` and each ref starts with `evidence:`.

This is a fixture-level contract, not a claim that the evidence content is scientifically correct.

## Schema validity

A record is schema-valid when:

- required top-level keys exist;
- entities and relations have required fields;
- relation source/target IDs resolve to entities in the same record;
- `json_valid` and `schema_valid` are boolean values;
- operational diagnostics are non-negative numbers.

## Operational diagnostics

The evaluator records:

- `cost_estimate`
- `latency_ms`
- `retry_count`

These are used to populate queue diagnostics in M070 `payload_metadata`.

## Expected metric names

- `entity_precision`
- `entity_recall`
- `entity_f1`
- `relation_precision`
- `relation_recall`
- `relation_f1`
- `evidence_path_validity`
- `schema_validity`
- `json_validity`
- `mean_cost_estimate`
- `mean_latency_ms`
- `total_retry_count`

## Future expansion

Future work may add:

- n-ary claim fixtures;
- LLM judge outputs;
- answer containment metrics;
- multimodal anchor fixtures;
- per-type F1 breakdown.

Those are explicitly out of scope for M071 smoke fixtures.
