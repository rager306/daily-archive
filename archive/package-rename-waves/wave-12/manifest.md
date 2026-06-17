# Package Rename Wave 12 Manifest

Scope: repair-cluster modules whose callers fit inside `src/arxiv_archive` or are already canonical. This wave keeps cross-slice dependencies on `universal_kb_contracts` (S09) and `evidence` (S12) intact, deferring `chunk_repair_contract`, `chunk_import_contract`, and `chunk_baseline_measurement` to a later wave.

## Moves

| Old runtime path | Canonical runtime path | Archive path |
|---|---|---|
| `src/arxiv_archive/bounded_chunk_repair.py` | `src/research_graph/repair/bounded_chunk_repair.py` | `archive/package-rename-waves/wave-12/src/arxiv_archive/bounded_chunk_repair.py` |
| `src/arxiv_archive/candidate_locators.py` | `src/research_graph/repair/candidate_locators.py` | `archive/package-rename-waves/wave-12/src/arxiv_archive/candidate_locators.py` |
| `src/arxiv_archive/chunking_benchmark.py` | `src/research_graph/repair/chunking_benchmark.py` | `archive/package-rename-waves/wave-12/src/arxiv_archive/chunking_benchmark.py` |

## Verification Notes

- Repair-cluster move follows the M022 review-only contract constraints: no payload text, no graph writes, no fact promotion.
- `chunking_benchmark` is a fixture-level deterministic benchmark, not production corpus retrieval.
- `chunk_repair_contract` and `chunk_import_contract` are intentionally retained under `src/arxiv_archive` until S09 retires `universal_kb_contracts` and S12 finalises `evidence` ownership.
