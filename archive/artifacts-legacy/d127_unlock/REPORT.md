# GEPA hub production batch (FdApiEmbedder)

## Ingest

| Metric | Value |
|--------|--------|
| PDFs found | 24 |
| OK | 24 |
| Fail | 0 |
| Duration ms | 76796 |
| Sections (ingest) | 965 |
| Citations | 1332 |
| CITES resolved | 419 |
| References written | 1332 |
| Snapshot | `artifacts/d127_unlock/hub_unlocked_t04.sgsnap` |

## Node counts (ontology labels)

| Label | Count |
|-------|------:|
| `Category` | 1 |
| `Citation` | 325 |
| `Paper` | 24 |
| `Reference` | 1255 |
| `Section` | 965 |
| `Source` | 1 |
| **total** | **2571** |

## Edge histogram (wire strings)

| Edge type | Count |
|-----------|------:|
| `CITES` | 419 |
| `FROM_SOURCE` | 24 |
| `first_child` | 65 |
| `hasPart` | 2297 |
| `inCategory` | 1 |
| `next_sibling` | 693 |
| `parent_section` | 179 |
| **total** | **3678** |

## Notes

- Embedder: **FdApiEmbedder** (real TEI bge-m3, 1024d).
- Labels/edges driven by domain ontology + edge_contracts (not hardcoded CLI lists).
- `import_eligible=false` (D127, durable).
