# M005/S01 — Import-Ready Chunk Contract

## Purpose

This contract defines the chunk package shape that `daily-archive` must produce before any scientific KG import consumes chunks. It turns M004 graph-readiness research into an implementation target for M005.

The reader is the engineer implementing chunk measurement, improved chunking, annotation sidecars, and dry-run import validation. After reading this document, they should be able to build validators and package exporters without guessing field names, states, refusal rules, or import boundaries.

## Non-Goals

This contract does not authorize production KG writes. It also does not validate semantic retrieval, vector ranking, DSPy extraction, LLM chunking, or broad corpus scaling. It defines the package boundary that later slices must satisfy before those topics can be reconsidered.

## Core Rule

A chunk is import-ready only when it is typed, route-scoped, source-spanned, hierarchically linked, redaction-safe, and eligible for a specific downstream use.

Non-zero chunks are not import readiness. Schema-valid chunks are not import readiness. Annotations are not KG facts.

## Object Graph

```text
ImportReadyChunkPackage
├── PaperIdentity
├── ConversionRecord
├── NormalizedElement[]
├── GraphReadyChunk[]
├── ChunkAnnotation[]
├── EvidencePath[]
└── PackageDiagnostics
```

The package is emitted per paper per validation run. It may be valid but not import-ready. Validity means references resolve and diagnostics are inspectable. Import readiness means at least one chunk/route is eligible after all gates.

## ImportReadyChunkPackage

Required fields:

| Field | Type | Requirement |
|---|---|---|
| `schema_version` | string | Must be `m005-import-ready-chunk-package.v1`. |
| `contract_version` | string | Must be `import-ready-chunk-contract.v1`. |
| `run_id` | string | Stable run identifier. |
| `created_at` | string | ISO-8601 timestamp. |
| `paper_id` | string | Must match `paper.paper_id`. |
| `paper` | PaperIdentity | Canonical paper metadata. |
| `conversion` | ConversionRecord | Converter method, quality state, and warnings. |
| `elements` | NormalizedElement[] | Required, can be empty only when package state is `reject`. |
| `chunks` | GraphReadyChunk[] | Required, can be empty only when package state is `reject`. |
| `annotations` | ChunkAnnotation[] | Required array; can be empty before S04. |
| `evidence_paths` | EvidencePath[] | Required for graph-eligible chunks. |
| `diagnostics` | PackageDiagnostics | Aggregate state, counts, warnings, and redaction flags. |

Required package invariants:

1. `paper_id` matches every child object `paper_id`.
2. Every object ID is stable and deterministic for the same input artifact.
3. Every chunk parent reference resolves to an element or parent chunk in the same package.
4. Every graph-eligible chunk has a valid source span.
5. Every graph-eligible chunk has an evidence path resolving to an existing chunk and source element.
6. Every chunk declares `chunk_type`, `route`, `state`, `allowed_uses`, and `excluded_uses`.
7. No package machine artifact includes raw paper text, raw chunk text, embeddings, vectors, secrets, tokens, or credentials.
8. Package validity is separate from import eligibility.

## PaperIdentity

Required fields:

| Field | Type | Requirement |
|---|---|---|
| `paper_id` | string | arXiv identifier including version when available. |
| `title` | string or null | Optional in machine summaries; must not be used as ID. |
| `categories` | string[] | May be empty. |
| `source_artifacts` | string[] | Redacted artifact identifiers such as `normalized_markdown:<paper_id>`. |

## ConversionRecord

Required fields:

| Field | Type | Requirement |
|---|---|---|
| `conversion_id` | string | Stable ID for converter output. |
| `converter` | string | Example: `docling`, `arxiv2md`, `manual_fixture`. |
| `converter_version` | string or null | Record when known. |
| `source_artifact` | string | Redacted source artifact identifier. |
| `quality_state` | GraphReadinessState | Package-level conversion state. |
| `warnings` | QualityWarning[] | Required array. |
| `raw_text_included` | boolean | Must be `false` in machine artifacts. |
| `embeddings_included` | boolean | Must be `false`. |

A conversion can be readable but not graph-ready. For example, flattened tables or missing spans should usually yield `repair_required` for affected routes, not silent import.

## NormalizedElement

Normalized elements are typed document structures produced before chunking.

Required fields:

| Field | Type | Requirement |
|---|---|---|
| `element_id` | string | Stable deterministic ID. |
| `paper_id` | string | Required. |
| `element_type` | ContentType | Required. |
| `parent_element_id` | string or null | Null only for root-level elements. |
| `section_path` | string[] | Required; can be empty only for root metadata. |
| `order_index` | integer | Required document order. |
| `source_span` | SourceSpan or null | Required for graph routes; null allowed only with warning and non-graph state. |
| `quality_state` | GraphReadinessState | Required. |
| `warnings` | QualityWarning[] | Required array. |

Content-bearing elements may have bounded review snippets in human review artifacts, but machine diagnostics should store identifiers, counts, spans, and warnings rather than raw text.

## GraphReadyChunk

A `GraphReadyChunk` is the smallest unit that future import/retrieval/extraction stages may consume. It must not be a generic text blob.

Required fields:

| Field | Type | Requirement |
|---|---|---|
| `chunk_id` | string | Stable deterministic ID derived from paper, parent element, route/type, and order. |
| `paper_id` | string | Required. |
| `parent_chunk_id` | string or null | Required for child chunks; null for top-level chunks. |
| `parent_element_ids` | string[] | Required and all must resolve. |
| `section_path` | string[] | Required. |
| `chunk_type` | ChunkType | Required. |
| `route` | ChunkRoute | Required. |
| `state` | GraphReadinessState | Required. |
| `allowed_uses` | DownstreamUse[] | Required. |
| `excluded_uses` | DownstreamUse[] | Required. |
| `order_index` | integer | Required. |
| `source_span` | SourceSpan or null | Required when `state=ok_for_graph`. |
| `source_artifact` | string | Redacted artifact identifier. |
| `evidence_path_id` | string or null | Required when `state=ok_for_graph`. |
| `quality_warnings` | QualityWarning[] | Required array. |
| `redaction` | RedactionFlags | Required. |

Required chunk invariants:

1. `state=ok_for_graph` requires `source_span`, `evidence_path_id`, non-empty `parent_element_ids`, and route-compatible `allowed_uses`.
2. `state=ok_for_retrieval_only` may allow retrieval diagnostics but must exclude trusted KG import.
3. `state=repair_required` must include at least one repair/blocking warning.
4. `state=reject` must exclude all extraction and import uses.
5. `route=claim_extraction` cannot be used for references, metadata, navigation noise, availability statements, author affiliations, or administrative prose.
6. Table, figure, equation, and reference content must be typed away from ordinary prose claim chunks unless explicitly represented as context-only chunks.
7. Parent-child relations are provenance and context links, not permission to import every sibling.

## ChunkAnnotation

Annotations are deterministic sidecars that help route, review, and diagnose chunks. They are weak signals and must not be imported as trusted KG facts without a later extraction validation step.

Required fields:

| Field | Type | Requirement |
|---|---|---|
| `annotation_id` | string | Stable deterministic ID. |
| `paper_id` | string | Required. |
| `chunk_id` | string | Must resolve to a chunk. |
| `method` | AnnotationMethod | Required. |
| `method_version` | string or null | Record when known. |
| `annotation_type` | string | Example: `keyphrase`, `numeric_marker`, `citation_marker`, `route_hint`. |
| `values` | object[] | Redacted values or hashes; no raw long text. |
| `confidence_class` | string | One of `high_precision`, `diagnostic`, `experimental`. |
| `promoted_to_fact` | boolean | Must be `false` in M005. |
| `warnings` | QualityWarning[] | Required array. |

Allowed annotation methods in M005 are deterministic or local classical methods: rules, regex, TF-IDF/BM25, YAKE, RAKE, TextRank, spaCy-like lexical analysis, and citation/number/unit detectors. LLM-generated annotations are out of scope for M005 defaults.

## EvidencePath

Evidence paths connect chunks to normalized elements and source artifacts.

Required fields:

| Field | Type | Requirement |
|---|---|---|
| `evidence_path_id` | string | Stable deterministic ID. |
| `paper_id` | string | Required. |
| `chunk_id` | string | Must resolve. |
| `source_element_ids` | string[] | Must resolve. |
| `source_artifact` | string | Redacted artifact identifier. |
| `source_span` | SourceSpan | Required for graph import eligibility. |
| `provenance_chain` | string[] | Must include conversion, element, chunk, and evidence IDs. |

## SourceSpan

Required fields:

| Field | Type | Requirement |
|---|---|---|
| `coordinate_space` | string | Example: `canonical_normalized_markdown`. |
| `char_start` | integer | Inclusive offset. |
| `char_end` | integer | Exclusive offset; must be greater than `char_start`. |
| `page_start` | integer or null | Optional but preferred for PDF-derived artifacts. |
| `page_end` | integer or null | Optional but preferred. |

A source span is not raw text. It is a coordinate reference into a controlled source artifact.

## QualityWarning

Required fields:

| Field | Type | Requirement |
|---|---|---|
| `code` | string | Stable machine-readable warning code. |
| `severity` | WarningSeverity | Required. |
| `message` | string | Short redacted diagnostic. |
| `object_id` | string | Paper, element, chunk, annotation, or evidence path ID. |
| `route` | ChunkRoute or null | Required when warning affects a route. |
| `blocks_import` | boolean | Required. |

Warnings should be specific enough to drive refusal evidence. Avoid generic `invalid` when `missing_source_span`, `unresolved_parent_element`, or `reference_pollutes_claim_route` is known.

## RedactionFlags

Required fields:

| Field | Type | Required value in machine artifacts |
|---|---|---|
| `raw_text_included` | boolean | `false` |
| `chunk_text_included` | boolean | `false` |
| `embeddings_included` | boolean | `false` |
| `vectors_included` | boolean | `false` |
| `secrets_included` | boolean | `false` |

Human review artifacts may include bounded snippets, but must be clearly separate from machine logs and must not contain secrets or embeddings.

## Enums

### GraphReadinessState

```text
ok_for_graph
ok_for_retrieval_only
repair_required
reject
```

State meanings:

| State | Meaning | Allowed downstream use |
|---|---|---|
| `ok_for_graph` | Structure, typing, source span, route, evidence path, and quality are sufficient for future trusted extraction/import rehearsal. | Dry-run import and later extraction for matching route. |
| `ok_for_retrieval_only` | Useful for search or context but lacks graph-grade provenance, atomicity, structure, or type. | Retrieval diagnostics only. |
| `repair_required` | Contains substantive content with fixable blockers. | Block import until repaired or explicitly excluded. |
| `reject` | Wrong, empty, untraceable, noisy, landing-page-like, or unsafe. | Diagnostic record only. |

### WarningSeverity

```text
info
warn
repair_required
blocker
```

### ContentType

```text
paper_metadata
abstract
section_heading
paragraph
table
table_caption
table_row
table_cell
figure
figure_caption
equation
reference_entry
citation_marker
author_affiliation
availability_statement
ethics_statement
competing_interests
acknowledgements
appendix
supplementary
boilerplate
navigation_noise
unknown
```

### ChunkType

```text
claim_candidate
method_candidate
result_candidate
definition_candidate
table_context
table_row_group
figure_caption_context
equation_context
citation_context
reference_entry
metadata
administrative
retrieval_context
noise
unknown
```

### ChunkRoute

```text
claim_extraction
method_extraction
entity_candidate_extraction
relation_extraction
table_extraction
citation_graph
metadata_graph
retrieval_only
exclude_from_extraction
```

### DownstreamUse

```text
trusted_kg_import
claim_extraction
entity_extraction
relation_extraction
table_fact_extraction
citation_graph_import
metadata_import
retrieval_diagnostics
review_only
none
```

### AnnotationMethod

```text
rules
regex
tfidf
bm25
yake
rake
textrank
spacy_lexical
citation_detector
numeric_unit_detector
section_heuristic
```

## Import Eligibility Rules

A chunk is eligible for future dry-run KG import only when all of the following are true:

1. `state == ok_for_graph`.
2. `trusted_kg_import` is present in `allowed_uses`.
3. `trusted_kg_import` is absent from `excluded_uses`.
4. `route` is not `retrieval_only` or `exclude_from_extraction`.
5. `chunk_type` is compatible with `route`.
6. `source_span` is present and valid.
7. `evidence_path_id` resolves and its source span matches or contains the chunk source span.
8. Every parent element reference resolves.
9. No warning with `severity in {repair_required, blocker}` affects the chunk or route.
10. Redaction flags are all safe.

If any rule fails, the dry-run importer must refuse the chunk with a specific reason.

## Required Refusal Reasons

Future validators and dry-run importers should use these stable refusal reason names where applicable:

```text
missing_chunk_id
missing_paper_id
missing_parent_element
unresolved_parent_element
missing_source_span
invalid_source_span
missing_evidence_path
unresolved_evidence_path
missing_chunk_type
missing_route
invalid_state_for_import
route_excluded_from_import
retrieval_only_not_importable
repair_required_not_importable
rejected_not_importable
warning_blocks_import
raw_text_leakage
embedding_leakage
vector_leakage
annotation_promoted_to_fact
reference_pollutes_claim_route
metadata_pollutes_claim_route
table_flattened_into_claim_route
figure_or_equation_untyped
schema_version_mismatch
contract_version_mismatch
```

## Route Compatibility Matrix

| ChunkRoute | Compatible ChunkTypes | Import boundary |
|---|---|---|
| `claim_extraction` | `claim_candidate`, `result_candidate`, `definition_candidate` | Only graph-eligible prose claim chunks. |
| `method_extraction` | `method_candidate` | Methods/procedures with enough context. |
| `entity_candidate_extraction` | `claim_candidate`, `method_candidate`, `result_candidate`, `definition_candidate` | Candidate generation only; not final facts. |
| `relation_extraction` | `claim_candidate`, `result_candidate`, `table_context`, `table_row_group` | Requires subject/object/metric context. |
| `table_extraction` | `table_context`, `table_row_group` | Requires table structure and labels. |
| `citation_graph` | `citation_context`, `reference_entry` | Citation/reference graph only. |
| `metadata_graph` | `metadata`, `administrative` | Metadata only, not scientific claims. |
| `retrieval_only` | `retrieval_context`, most non-noise types | Retrieval diagnostics only. |
| `exclude_from_extraction` | `noise`, `unknown`, unsafe administrative content | No import. |

## Diagnostics Required in PackageDiagnostics

Required fields:

| Field | Type | Requirement |
|---|---|---|
| `package_state` | GraphReadinessState | Aggregate package decision. |
| `valid_package` | boolean | References and required arrays are structurally valid. |
| `import_eligible_chunk_count` | integer | Count after eligibility rules. |
| `refused_chunk_count` | integer | Count refused for import. |
| `counts_by_state` | object | Required. |
| `counts_by_route` | object | Required. |
| `counts_by_chunk_type` | object | Required. |
| `refusal_counts` | object | Required. |
| `source_span_coverage` | number | 0.0 to 1.0. |
| `parent_reference_resolution_rate` | number | 0.0 to 1.0. |
| `evidence_path_resolution_rate` | number | 0.0 to 1.0. |
| `raw_text_included` | boolean | Must be `false`. |
| `embeddings_included` | boolean | Must be `false`. |
| `ladybugdb_written` | boolean | Must be `false` in M005. |
| `production_import_attempted` | boolean | Must be `false` in M005. |

## Machine Artifact Redaction Policy

Machine JSON/JSONL artifacts may include:

- IDs;
- enum states;
- route/type values;
- source span coordinates;
- source artifact identifiers;
- counts;
- warning codes;
- short redacted diagnostics;
- hashes for optional dedupe diagnostics.

Machine JSON/JSONL artifacts must not include:

- raw paper text;
- raw chunk text;
- long claim text;
- embeddings;
- vectors;
- secrets;
- API keys;
- optimizer traces;
- unbounded model outputs.

Review markdown artifacts may include bounded snippets only when needed for semantic review and must remain separate from machine logs.

## S01 Validator Expectations

The S01 validator should prove the contract catches at least these cases:

1. A valid synthetic package with one graph-eligible claim chunk passes.
2. A chunk missing `chunk_id` fails.
3. A graph-eligible chunk missing `source_span` fails.
4. A chunk referencing a missing parent element fails.
5. A graph-eligible chunk referencing a missing evidence path fails.
6. A retrieval-only chunk with `trusted_kg_import` fails.
7. Any artifact with raw text, embeddings, or vectors fails.
8. Any annotation with `promoted_to_fact=true` fails.
9. Metadata/reference chunks routed to claim extraction fail.
10. A package with no import-eligible chunks may still be valid, but must report `import_eligible_chunk_count=0` and should not be claimed as import-ready.

## Reader Test

A fresh implementer should be able to derive three immediate next actions from this contract:

1. Write dataclasses or typed dictionaries for package, chunk, annotation, warning, source span, and diagnostics.
2. Implement a validator that returns structured refusal diagnostics using the reason names above.
3. Export baseline and improved chunking packages without production KG writes or raw-text/embedding leakage.

If an implementation cannot decide whether a chunk may enter dry-run KG import from this document alone, the implementation should refuse the chunk and add a specific diagnostic rather than guessing.
