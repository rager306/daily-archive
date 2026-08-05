# GEPA hub production batch (FdApiEmbedder)

## Ingest

| Metric | Value |
|--------|--------|
| PDFs found | 24 |
| OK | 24 |
| Fail | 0 |
| Duration ms | 118040 |
| Sections (ingest) | 965 |
| Citations | 1332 |
| CITES resolved | 419 |
| References written | 1332 |
| Snapshot | `artifacts/gepa_hub_production/hub.sgsnap` |

## Graph inventory (ontology wire strings)

| Metric | Value |
|--------|--------|
| Paper | 24 |
| Reference | 1255 |
| Section | 965 |
| Category | 1 |
| Citation | 325 |
| Entity | 0 |
| Source | 1 |
| hasPart | 2297 |
| CITES | 419 |
| FROM_SOURCE | 24 |
| inCategory | 1 |
| MENTIONS | 0 |
| Edge histogram | `{"hasPart": 2297, "inCategory": 1, "FROM_SOURCE": 24, "CITES": 419}` |

## Notes

- Embedder: **FdApiEmbedder** (real TEI bge-m3, 1024d) — not MockEmbedder.
- `import_eligible=false` (D127).
- MENTIONS=0 until extract-to-graph runs.
- Process-local store; snapshot is durable artifact.
