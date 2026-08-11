# GEPA hub production batch (FdApiEmbedder)

## Ingest

| Metric | Value |
|--------|--------|
| PDFs found | 1 |
| OK | 1 |
| Fail | 0 |
| Duration ms | 52334 |
| Sections (ingest) | 55 |
| Citations | 40 |
| CITES resolved | 17 |
| References written | 40 |
| Snapshot | `artifacts/article_context/paper_2608.07067.sgsnap` |

## Node counts (ontology labels)

| Label | Count |
|-------|------:|
| `Category` | 1 |
| `Citation` | 17 |
| `Paper` | 1 |
| `Reference` | 40 |
| `Section` | 55 |
| `Source` | 1 |
| **total** | **115** |

## Edge histogram (wire strings)

| Edge type | Count |
|-----------|------:|
| `CITES` | 17 |
| `FROM_SOURCE` | 1 |
| `hasPart` | 95 |
| `inCategory` | 1 |
| `next_sibling` | 32 |
| `parent_section` | 1 |
| **total** | **147** |

## Notes

- Embedder: **FdApiEmbedder** (real TEI bge-m3, 1024d).
- Labels/edges driven by domain ontology + edge_contracts (not hardcoded CLI lists).
- `import_eligible=false` (D127, durable).
