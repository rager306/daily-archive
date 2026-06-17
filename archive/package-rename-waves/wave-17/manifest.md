# Package Rename Wave 17 Manifest

Scope: universal KB workflows.

## Moves

| Old runtime path | Canonical runtime path | Archive path |
|---|---|---|
| `src/arxiv_archive/universal_kb_contracts.py` | `src/research_graph/workflows/universal_kb/contracts.py` | `archive/package-rename-waves/wave-17/src/arxiv_archive/universal_kb_contracts.py` |
| `src/arxiv_archive/universal_kb_queue.py` | `src/research_graph/workflows/universal_kb/queue.py` | `archive/package-rename-waves/wave-17/src/arxiv_archive/universal_kb_queue.py` |
| `src/arxiv_archive/universal_kb_rehearsal.py` | `src/research_graph/workflows/universal_kb/rehearsal.py` | `archive/package-rename-waves/wave-17/src/arxiv_archive/universal_kb_rehearsal.py` |
| `src/arxiv_archive/universal_kb_review_assistance.py` | `src/research_graph/workflows/universal_kb/review_assistance.py` | `archive/package-rename-waves/wave-17/src/arxiv_archive/universal_kb_review_assistance.py` |
| `src/arxiv_archive/universal_kb_sidecar_boundary.py` | `src/research_graph/workflows/universal_kb/sidecar_boundary.py` | `archive/package-rename-waves/wave-17/src/arxiv_archive/universal_kb_sidecar_boundary.py` |
| `src/arxiv_archive/universal_kb_smoke.py` | `src/research_graph/workflows/universal_kb/smoke.py` | `archive/package-rename-waves/wave-17/src/arxiv_archive/universal_kb_smoke.py` |
| `src/arxiv_archive/universal_kb_substrate_rehearsal.py` | `src/research_graph/workflows/universal_kb/substrate_rehearsal.py` | `archive/package-rename-waves/wave-17/src/arxiv_archive/universal_kb_substrate_rehearsal.py` |

## Verification Notes

- Production-write, fact-promotion, import-eligibility, and KG-readiness flags remain explicitly false where required.
- No live graph writes or fact promotion are introduced.
- Smoke tests remain local-only and no-write.
