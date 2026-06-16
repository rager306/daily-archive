# M073 Evidence Path Contract

## Purpose

M073 attaches parser-derived full-paper evidence **references** to M072 reviewed benchmark fixtures. It does not convert the fixtures into full-paper gold labels yet.

The fixture payloads remain metadata-only: IDs, labels, types, roles, numeric diagnostics, and evidence refs are allowed; raw article body text is forbidden.

## Allowed evidence refs

Evidence refs may point to durable local artifacts by path-like identifiers:

- `artifact:m061-parser-manifest:<arxiv_id>:<manifest_path>`
- `artifact:canonical-pdf:<arxiv_id>:<pdf_path>`
- `missing:parser_manifest:<arxiv_id>`
- `missing:canonical_pdf:<arxiv_id>`

Allowed diagnostic fields:

- `evidence_status`
- `canonical_pdf_exists`
- `parser_manifest_count`
- `evidence_ref_count`
- `missing_reason`
- aggregate split coverage metrics such as `train_parser_manifest_coverage` and `validation_parser_manifest_coverage`

Allowed payload values are identifiers, paths, booleans, counts, ratios, and stable status strings.

## Forbidden content

Do not persist any of the following in benchmark fixtures, audit outputs, queue metadata, or closeout artifacts:

- raw text from PDFs or parser outputs
- article body paragraphs
- prompts
- completions
- embeddings
- vectors
- model payloads
- MiniMax request or response bodies
- DSPy optimizer traces
- secrets or API keys
- graph write payloads
- fact promotion payloads

## Execution boundaries

- MiniMax stays disabled.
- DSPy stays disabled.
- Qwen/local optimizer paths stay disabled.
- FalkorDB graph writes stay disabled.
- Fact promotion stays disabled.
- Production import stays disabled.
- Network download is not part of this milestone.

## Queue metadata requirements

Queue verification must keep:

- `write_eligibility=false`
- `promotion_eligibility=false`

Queue diagnostics may include evidence coverage metrics, but must not include raw text, prompts, embeddings, vectors, model payloads, graph writes, or promotion payloads.

## Missing evidence handling

If an article has no parser manifest, S02 must emit a missing diagnostic instead of inventing a parser evidence ref.

If an article has no canonical PDF, S02 must emit a missing diagnostic instead of attempting network acquisition in this milestone.

## S02 acceptance criteria

S02 may pass only if:

1. Every fixture case gets at least one evidence reference or explicit missing diagnostic.
2. Augmented artifacts remain metadata-only.
3. Tests or checks verify that forbidden content fields are absent.
4. M071/M072 evaluator metrics remain stable.
5. Queue metadata in S03 can record coverage diagnostics without enabling writes or promotion.
