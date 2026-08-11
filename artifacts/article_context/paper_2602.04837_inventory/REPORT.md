# GEPA hub production batch (FdApiEmbedder)

## Ingest

| Metric | Value |
|--------|--------|
| PDFs found | 1 |
| OK | 1 |
| Fail | 0 |
| Duration ms | 57234 |
| Sections (ingest) | 29 |
| Citations | 53 |
| CITES resolved | 20 |
| References written | 53 |
| Snapshot | `artifacts/article_context/paper_2602.04837.sgsnap` |

## Node counts (ontology labels)

| Label | Count |
|-------|------:|
| `Category` | 1 |
| `Citation` | 20 |
| `Paper` | 1 |
| `Reference` | 53 |
| `Section` | 29 |
| `Source` | 1 |
| **total** | **105** |

## Edge histogram (wire strings)

| Edge type | Count |
|-----------|------:|
| `CITES` | 20 |
| `FROM_SOURCE` | 1 |
| `hasPart` | 82 |
| `inCategory` | 1 |
| `next_sibling` | 17 |
| `parent_section` | 1 |
| **total** | **122** |

## Notes

- Embedder: **FdApiEmbedder** (real TEI bge-m3, 1024d).
- Labels/edges driven by domain ontology + edge_contracts (not hardcoded CLI lists).
- `import_eligible=false` (D127, durable).
