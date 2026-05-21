# M021 deterministic candidate locator design

## Purpose

Implement the M020 candidate locator protocol as deterministic code that can generate review-only locator artifacts from redacted target metadata and local source files.

The implementation must produce reproducible source ledgers, source spans, locator records, summaries, and safety guards. It must explain ambiguity rather than treating coordinate coverage as semantic support.

## Non-goals

M021 must not:

- promote locators to KG facts;
- write to LadybugDB;
- attempt production KG import;
- embed raw paper text, chunk text, extracted claim text, embeddings, vectors, secrets, prompts, or raw model payloads in machine artifacts;
- use MiniMax, DSPy optimizers, or LLM output as source of truth;
- claim semantic KG readiness from locator counts.

## Existing primitives to reuse

### `src/arxiv_archive/evidence.py`

Relevant concepts:

- `SemanticChunk`: deterministic chunk identity, paper ID, source path provenance, `char_start`, `char_end`, and chunking strategy.
- `EvidencePath`: paper -> page-index node -> semantic chunk trace.
- `validate_evidence_path`: existing validation pattern for broken references.

Decision: do **not** modify `SemanticChunk` in M021. GitNexus impact for `SemanticChunk` is MEDIUM with five direct importers. New locator code may refer to semantic chunk IDs/coordinates but should avoid changing existing dataclasses.

### `src/arxiv_archive/import_boundary_rehearsal.py`

Relevant concepts:

- explicit safety flags;
- forbidden raw/embedding/vector/secret/optimizer fields;
- `ImportCandidate.to_contract()` pattern for accepted/rejected/import-disabled records;
- redacted diagnostic records.

Decision: reuse the safety style and import-disabled semantics, but do not depend on import-boundary rehearsal schema directly because candidate locators are evidence pointers, not import candidates.

### `src/arxiv_archive/validation_batch_workflow.py`

Relevant concepts:

- deterministic local artifact preparation;
- source readiness and hash/path checks;
- redacted summaries and JSONL diagnostics;
- no source acquisition, conversion, import, or LadybugDB writes in helper functions.

Decision: mirror this style: pure helpers return dictionaries/dataclasses; file writing is explicit and bounded.

## Proposed module

```text
src/arxiv_archive/candidate_locators.py
```

## Proposed tests

```text
tests/test_candidate_locators.py
```

## Public API draft

```python
CANDIDATE_LOCATOR_PROTOCOL_VERSION = "candidate_locator_protocol.v1"

@dataclass(frozen=True)
class LocatorSource:
    source_id: str
    paper_id: str
    source_path: Path
    expected_sha256: str | None = None
    source_type: str = "markdown"
    conversion_method: str = "existing_markdown"

@dataclass(frozen=True)
class LocatorRouteSpec:
    route_name: str
    candidate_type: str
    route: str
    signal_patterns: tuple[str, ...]

@dataclass(frozen=True)
class LocatorBuildResult:
    artifact: dict[str, Any]
    diagnostics: tuple[LocatorDiagnostic, ...]

build_candidate_locator_artifact(...)
validate_candidate_locator_artifact(...)
write_candidate_locator_artifact(...)
```

The exact API can be adjusted in S02, but it should remain small, deterministic, and testable.

## Data model

### Source ledger

Generated source records should include:

- `source_id`
- `paper_id`
- `source_type`
- `source_path`
- `source_hash`
- `source_hash_algorithm=sha256`
- `conversion_method`
- `conversion_status`
- `raw_text_embedded=false`
- `raw_binary_embedded=false`

### Source spans

Generated spans should include:

- `span_id`
- `source_id`
- `coordinate_space`
- `char_start`/`char_end` when available
- `line_start`/`line_end` when available
- optional `semantic_chunk_id` and `chunking_strategy`
- `span_hash` over coordinate packet, not span text
- `raw_text_embedded=false`
- `ambiguity_diagnostics`

### Locator records

Generated locator records should include:

- `locator_id`
- `paper_id`
- `candidate_type`
- `route`
- `state`
- `source_spans`
- `support_level`
- `uncertainty_label`
- `review_queue_reason`
- `diagnostic_codes`
- `allowed_uses=[candidate_locator_review, provenance_diagnostics]`
- `excluded_uses=[trusted_kg_import, production_ladybugdb_write, embedding_generation, source_of_truth_claim]`
- `import_eligible=false`
- `promoted_to_fact=false`
- `minimax_source_of_truth=false`

## Ambiguity diagnostics

M021 should distinguish at least these classes:

| Diagnostic | Meaning | Likely next action |
|---|---|---|
| `source_missing` | Source path absent | source acquisition/repair |
| `source_hash_mismatch` | File exists but hash differs | provenance refresh or stale artifact review |
| `signal_missing` | No route signal found | locator heuristic or chunking repair |
| `broad_signal_many_matches` | Too many matching regions | refine route signals or chunk segmentation |
| `overlapping_signal_window` | Span overlaps with other candidate route windows | structural/chunking review |
| `candidate_type_uncertain` | Route metadata maps poorly to locator type | reviewer decision or route map update |
| `review_required` | Coordinate exists but support not semantically verified | semantic review |

## State mapping

- `source_missing`, `source_hash_mismatch`, or `signal_missing` -> `missing_span`, `support_level=insufficient`, `review_queue_reason=span_missing`.
- `broad_signal_many_matches` or `overlapping_signal_window` -> `ambiguous_span`, `support_level=nearby_context`, `review_queue_reason=span_ambiguous`.
- source OK with bounded signal -> `review_required`, `support_level=not_evaluated`, `review_queue_reason=needs_semantic_review`.
- retrieval-only route -> `retrieval_only`, `support_level=nearby_context`, `review_queue_reason=retrieval_only`.
- route metadata already marked repair-heavy -> `repair_required`, `support_level=insufficient`, `review_queue_reason=repair_required`.

## Safety guard

A validation helper should reject or diagnose:

- any forbidden exact payload key: `text`, `raw_text`, `chunk_text`, `paper_text`, `claim_text`, `embedding`, `vector`, `secret`, `token`, `api_key`, `credentials`, `optimizer_trace`, `raw_model_payload`, `raw_minimax_response`;
- `import_eligible=true`;
- `promoted_to_fact=true`;
- `ladybugdb_written=true`;
- `production_import_attempted=true`;
- missing source ledger fields;
- invalid coordinate ranges;
- raw text embedding flags set true.

## Artifact outputs

S02 implementation should support generating an artifact shaped like:

```text
schema_version
run_id
paper_id
source_ledger
locators
per_paper_summary
summary
safety_flags
recommendation
```

The writer should not serialize raw source text. Tests should scan output recursively for forbidden exact keys.

## Test plan

S02 tests should cover:

1. valid one-source locator artifact generation;
2. source hash mismatch produces blocked/missing-span diagnostic;
3. broad repeated signal produces `ambiguous_span` and `broad_signal_many_matches`;
4. missing signal produces `missing_span` and `signal_missing`;
5. invalid coordinate ranges are rejected by validation;
6. forbidden exact payload keys are detected recursively;
7. import/fact-promotion flags are rejected if true;
8. writer output has no raw text/chunk/claim fields.

## CLI/callable integration plan

S03 may add a bounded callable or CLI subcommand, but S02 should first expose stable pure functions. If CLI is added, it should be under existing `validation-batch` or a new explicit `candidate-locators` command only after impact analysis for CLI symbols.

## Recommended implementation boundary

Add:

```text
src/arxiv_archive/candidate_locators.py
tests/test_candidate_locators.py
```

Avoid editing existing modules in S02 unless tests prove an import/export surface is needed. This keeps blast radius low and leaves CLI integration to S03.
