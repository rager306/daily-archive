# M033 S04 QuantMind to daily-archive Pattern Map

## Verdict

`pattern-source-not-dependency`: quant-mind is useful as a reference architecture, not as a production dependency or runtime parser/graph platform for M033.

## Patterns to adopt

### TreeKnowledge / TreeNode
- daily-archive mapping: PageIndex-style root/section/subsection/leaf hierarchy
- adopt as: `schema-pattern`
- why: Matches M033 need for section hierarchy and navigable paper structure better than flat chunks alone.
- constraints:
  - daily-archive must preserve source spans and refusal diagnostics
  - leaf content cannot become graph-ready without review
  - embeddings are coarse prefilter only, not reasoning replacement

### Paper + PaperKnowledgeCard split
- daily-archive mapping: Full paper tree plus flat summary/index card for dashboard/filtering/search prefilter
- adopt as: `schema-pattern`
- why: Separates deep retrieval over tree from lightweight metadata/summary card.
- constraints:
  - summary card remains candidate artifact until review
  - bibliographic facts should consume GROBID/metadata evidence with provenance

### SourceRef / Citation / ExtractionRef
- daily-archive mapping: SourceRef/EvidencePath/provenance primitives with content hash, page/offset/tree node anchors, flow/model metadata
- adopt as: `provenance-pattern`
- why: Reinforces M031/M033 no-bare-string provenance and auditability goals.
- constraints:
  - quote/content values should not leak into diagnostics
  - source anchors must be validated independently

### preprocess.fetch + preprocess.format + flow separation
- daily-archive mapping: source_acquisition -> parser_conversion -> candidate extraction boundaries
- adopt as: `pipeline-boundary-pattern`
- why: Cleanly separates byte acquisition, format conversion, and knowledge extraction.
- constraints:
  - daily-archive should keep typed blockers for low-quality/no-substantive-body sources
  - parser output alone does not imply chunk or graph readiness

### batch_run bounded concurrency
- daily-archive mapping: bounded per-paper probe/extraction fanout with explicit success/failure counts
- adopt as: `execution-pattern`
- why: Useful lightweight concurrency primitive; memory= prohibition is a good race-hazard guardrail.
- constraints:
  - diagnostics must avoid raw text/secrets
  - stateful memory accumulation should be serial or explicitly synchronized

### magic resolver schema introspection guardrails
- daily-archive mapping: typed input resolution without inventing paths/URLs
- adopt as: `guardrail-pattern-only`
- why: The instruction “never invent file paths or URLs” matches daily-archive catalog/source safety principles.
- constraints:
  - do not adopt runtime resolver now because it calls OpenAI Agents
  - prefer deterministic CLI/API validation for production

## Reject or defer

- **quantmind as production dependency**: Runtime depends on OpenAI Agents/API and missing storage/retrieval/graph layers; integration risk medium/high.
- **paper_flow runtime in M033**: Would call model/network paths and test LLM extraction rather than architecture pattern fit.
- **GraphKnowledge**: Placeholder only; subclassing blocked by NotImplementedError.
- **storage/retrieval/RAG layers**: Not implemented in current package tree.
- **PyMuPDF PDF formatter as parser upgrade**: Plain-text extraction only; daily-archive already needs higher quality parser/layout/OCR evidence from GROBID/OpenDataLoader paths.

## S05 synthesis implication

- GROBID: scholarly TEI sidecar candidate.
- OpenDataLoader: layout/table/OCR sidecar candidate.
- Adaptix: typed adapter proof over fixed parser JSON.
- quant-mind: tree/card/provenance architecture pattern source only.

## Safety flags

- `graph_import_allowed=false`
- `ladybugdb_written=false`
- `production_import_attempted=false`
- `import_eligible=false`
