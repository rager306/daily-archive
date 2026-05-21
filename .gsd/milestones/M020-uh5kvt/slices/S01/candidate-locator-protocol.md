# M020 candidate locator and chunk-span provenance protocol

## Purpose

This protocol defines **candidate locators**: redacted, reviewable evidence pointers that identify where a possible Scientific KG claim, entity, relation, method, dataset, metric, or limitation may be grounded in source artifacts.

A candidate locator is **not** a KG fact. It is not import-eligible. It is a structured review packet that points to source/chunk coordinates so a later semantic review can decide whether positive import should remain blocked or proceed in a future milestone.

## Non-goals

Candidate locators must not:

- promote assets, annotations, claims, entities, or relations to trusted KG facts;
- write to LadybugDB;
- trigger production import;
- include raw paper text, chunk text, extracted claim text, embeddings, vectors, prompts, model payloads, or secrets;
- use MiniMax or any LLM as source of truth;
- claim semantic KG readiness from counts alone.

## Protocol version

```text
candidate_locator_protocol.v1
```

## Contract overview

A candidate locator artifact contains:

```text
schema_version
run_id
paper_id
source_ledger
locators
summary
safety_flags
```

The `source_ledger` records source artifact identity and transformation state. `locators` records candidate evidence pointers with exact source/chunk coordinates and review status. `summary` records redacted aggregate counts. `safety_flags` records blocked import/write/raw-payload behavior.

## Source ledger fields

Each source ledger item must include:

| Field | Required | Meaning |
|---|---:|---|
| `source_id` | yes | Stable ID for this source artifact in the run |
| `paper_id` | yes | arXiv or local paper identifier |
| `source_type` | yes | `markdown`, `pdf`, `html`, `json_manifest`, or `derived_chunk_package` |
| `source_path` | yes | Local path or artifact path; path only, no raw content |
| `source_hash` | yes | Hash of the source file or artifact when available |
| `source_hash_algorithm` | yes | e.g. `sha256` |
| `conversion_method` | yes | e.g. `existing_markdown`, `docling`, `arxiv2md`, `marker`, `unknown` |
| `conversion_status` | yes | `accepted`, `low_quality`, `missing`, `blocked`, `review_required` |
| `raw_text_embedded` | yes | Must be `false` |
| `raw_binary_embedded` | yes | Must be `false` |

## Source span fields

Each locator must include at least one `source_span`.

| Field | Required | Meaning |
|---|---:|---|
| `span_id` | yes | Stable ID within the locator |
| `source_id` | yes | Reference to a source ledger item |
| `coordinate_space` | yes | `normalized_markdown_char`, `semantic_chunk_char`, `page_index_node`, or `artifact_record` |
| `char_start` | conditional | Inclusive char start in coordinate space |
| `char_end` | conditional | Exclusive char end in coordinate space |
| `line_start` | optional | 1-indexed line start when available |
| `line_end` | optional | 1-indexed line end when available |
| `page_index_node_id` | conditional | Required for page-index/node references |
| `semantic_chunk_id` | conditional | Required for chunk-level references |
| `chunking_strategy` | conditional | Required when `semantic_chunk_id` is present |
| `evidence_path_id` | optional | Existing evidence path ID if already available |
| `span_hash` | yes | Hash over the referenced span or coordinate packet, not raw text |
| `raw_text_embedded` | yes | Must be `false` |

Coordinate rules:

- `char_start` and `char_end` must be non-negative integers when present.
- `char_end` must be greater than `char_start`.
- Span coordinates point to source artifacts; they do not embed the span text.
- Hashes are allowed; raw text is not.

## Candidate locator fields

Each locator must include:

| Field | Required | Meaning |
|---|---:|---|
| `locator_id` | yes | Stable run-local locator ID |
| `paper_id` | yes | Paper identifier |
| `candidate_type` | yes | One of allowed candidate types below |
| `route` | yes | Handling route, not a fact label |
| `state` | yes | Review/import state |
| `source_spans` | yes | Non-empty array of source-span records |
| `support_level` | yes | One of allowed support levels |
| `uncertainty_label` | yes | One of allowed uncertainty labels |
| `review_queue_reason` | yes | One of allowed review queue reasons |
| `diagnostic_codes` | yes | Redacted machine-readable diagnostics |
| `allowed_uses` | yes | Must exclude trusted import |
| `excluded_uses` | yes | Must include trusted import/write/embedding generation exclusions |
| `import_eligible` | yes | Must be `false` in M020 |
| `promoted_to_fact` | yes | Must be `false` |
| `minimax_source_of_truth` | yes | Must be `false` |

Allowed candidate types:

```text
claim_candidate
entity_candidate
relation_candidate
method_candidate
dataset_candidate
metric_candidate
limitation_candidate
citation_candidate
retrieval_only_context
repair_required_context
```

Allowed routes:

```text
claim_location
entity_location
relation_location
method_location
dataset_location
metric_location
limitation_location
citation_location
retrieval_context
repair_context
```

Allowed states:

```text
located_unreviewed
review_required
ambiguous_span
missing_span
conflicting_evidence
unsupported
retrieval_only
repair_required
rejected
```

Allowed support levels:

```text
direct_span
nearby_context
multi_span
insufficient
contradicted
not_evaluated
```

Allowed uncertainty labels:

```text
low
medium
high
unknown
```

Allowed review queue reasons:

```text
needs_semantic_review
span_missing
span_ambiguous
evidence_conflict
conversion_quality_blocker
locator_schema_error
source_hash_missing
candidate_type_uncertain
retrieval_only
repair_required
not_reviewed
```

## Safety fields

Every locator artifact must include these booleans, all with the listed values for M020:

```text
production_import_attempted=false
ladybugdb_written=false
trusted_kg_import_allowed=false
raw_text_included=false
chunk_text_included=false
raw_binary_included=false
base64_included=false
embeddings_included=false
vectors_included=false
secrets_included=false
optimizer_traces_included=false
model_payloads_included=false
minimax_source_of_truth=false
```

## Import-disabled semantics

M020 locators are always import-disabled:

```text
import_eligible=false
promoted_to_fact=false
allowed_uses=["candidate_locator_review", "provenance_diagnostics"]
excluded_uses=["trusted_kg_import", "production_ladybugdb_write", "embedding_generation", "source_of_truth_claim"]
```

A future milestone may define positive import gates, but only after independent semantic review proves locators are meaningful and evidence-backed.

## Summary fields

The artifact summary should include counts only:

```text
locator_count
source_count
located_count
review_required_count
missing_span_count
ambiguous_span_count
conflicting_evidence_count
retrieval_only_count
repair_required_count
import_eligible_count
promoted_to_fact_count
```

For M020:

```text
import_eligible_count=0
promoted_to_fact_count=0
```

## Review expectations

Reviewers should evaluate:

1. Does the locator point to a real source artifact?
2. Are coordinates present and plausible?
3. Does the locator avoid raw corpus leakage?
4. Is the candidate type appropriate?
5. Is support level justified by available span references?
6. Does uncertainty/review queue reason correctly describe the failure mode?
7. Does the artifact keep production import and LadybugDB writes blocked?

## Relationship to prior work

- M011 required chunk-span provenance and candidate locators before positive semantic KG import.
- M019 recommended protocol-bound source-ledger/review-gated workflows, especially prismAId-style protocol configuration and GPT Researcher-style source-first provenance.
- M020 uses those lessons to define locators as review evidence, not facts.
