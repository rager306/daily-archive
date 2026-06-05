# M033 S05 T01 Synthesis Evidence Matrix

- status: `complete`
- external runtime rerun: `false`
- candidate_only: `true`
- graph_import_allowed=false
- ladybugdb_written=false
- production_import_attempted=false
- import_eligible=false

| Slice | Verdict | Recommended role | Main unresolved gates |
|---|---|---|---|
| S01 | `baseline-established` | Defines current parser/conversion/refusal contracts used to compare external tools. | baseline is comparison substrate, not a new parser implementation |
| S02 | `grobid-scholarly-sidecar-candidate` | Scholarly TEI sidecar candidate for metadata, headers, sections, bibliography, citation/ref markers, and TEI structure. | standalone graph-ready parser; table-fidelity proof; OCR replacement proof; production import eligibility |
| S03 | `hybrid-sidecar-candidate` | Layout/OCR/table/coordinate sidecar candidate via hybrid docling-fast backend over local PDFs. | No scanned/image-only PDF was included, so OCR quality remains not proven.; No independent table ground truth was used, so table fidelity is qualitative only.; Outputs are candidate evidence only and are not graph-ready or import-eligible.; Need a larger probe before production adoption or sidecar schema commitment. |
| S07 | `adaptix-adapter-candidate` | Typed adapter candidate mapping fixed OpenDataLoader JSON into daily-archive candidate summaries. | structural mapping is not semantic validation; does not prove reading order, table fidelity, source-span correctness, or graph readiness |
| S04 | `pattern-source-not-dependency` | Architecture pattern source for TreeKnowledge/PageIndex, PaperKnowledgeCard, provenance, pipeline separation, bounded concurrency, and resolver guardrails. | M033 runtime dependency; production RAG/KB platform; semantic KG implementation; graph import trigger |

## Synthesis implication

The evidence supports a bounded combined sidecar architecture: GROBID for scholarly TEI, OpenDataLoader for layout/OCR/table/coordinate candidates, Adaptix for typed fixed-JSON mapping, quant-mind for architecture patterns, and daily-archive for all contracts, validators, review gates, and graph-readiness decisions.
