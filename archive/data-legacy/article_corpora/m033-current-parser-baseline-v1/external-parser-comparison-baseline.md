# M033 S01 T04: External Parser Comparison Baseline

This matrix defines what GROBID, OpenDataLoader, and quant-mind research must answer against the current daily-archive baseline.

| Capability | Current baseline | GROBID question | OpenDataLoader question | quant-mind pattern question | Must preserve |
|---|---|---|---|---|---|
| scholarly_metadata_and_bibliography | Limited in M031 parser/conversion replay; bibliography/citation extraction is not the current baseline strength. | Can GROBID TEI provide reliable title/authors/abstract/references/citations with stable source anchors? | Does OpenDataLoader expose bibliography blocks or only visual/layout text? | Can PaperKnowledgeCard and provenance schemas represent bibliographic summary cards? | SourceRef hash/path provenance<br>pending review before graph claims |
| section_hierarchy_and_page_index | Current replay can produce parser-ready text and chunks, but external research must assess richer section hierarchy and PageIndex-like structure. | Does TEI section structure map cleanly to PageIndex nodes and section anchors? | Does layout output preserve headings, reading order, and page/block coordinates? | Can TreeKnowledge inspire root/section/leaf hierarchy and summary-card split? | source spans<br>section anchors<br>zero-chunk refusal for non-ready rows |
| tables_and_layout | M031 safety notes treat table/figure/layout handling as not globally validated; this is a key improvement target. | What table/figure support exists and where is it weak? | Can OCR/layout/table recognition produce machine-readable table artifacts and coordinates? | Which typed knowledge/card pattern could represent table candidates without graph import? | candidate artifact review state<br>no trusted fact promotion |
| reading_order_and_ocr_quality | Low-quality source detection exists; non-empty text is explicitly insufficient. | How does GROBID handle scanned/poor OCR PDFs? | Can OCR backend improve scanned/layout-heavy PDFs and expose health/failure diagnostics? | Does fetch/format separation help isolate OCR quality from knowledge extraction? | typed low-quality diagnostics<br>backend health/blocker state |
| provenance_and_evidence_paths | M031 requires paths, hashes, source spans, diagnostics, and no-write flags through the chain. | Can TEI elements be linked back to page/offset/source spans? | Can layout blocks/tables expose coordinates usable as EvidencePath anchors? | Can SourceRef/Citation/ExtractionRef patterns improve our schema without adopting the framework? | local path<br>sha256<br>diagnostic code<br>graph flags false |
| runtime_complexity_and_local_first_operation | Current replay is local artifact-based and no-write; external services must be treated as probes. | What Java/service/container/runtime burden is required? | Which backend is required: built-in, docling, hancom, hancom-ai, or typed blocker? | Which patterns can be adapted without runtime dependency or LLM extraction coupling? | bounded local probe<br>no production adoption from research alone |

## Downstream use

- **S02_GROBID**: Answer GROBID-specific questions in the matrix.
- **S03_OpenDataLoader**: Run 3-PDF probe and score against matrix capabilities.
- **S04_quant_mind**: Extract patterns answering pattern questions without dependency adoption.
- **S05_synthesis**: Choose GROBID-only, OpenDataLoader-only, combined sidecar, or reject/further probe.

## Safety constraints

- External parser probes are local bounded research only.
- No graph import, LadybugDB write, production import, or positive graph-readiness claim is allowed in M033.
