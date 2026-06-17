# Package Rename Wave 16 Manifest

Scope: validation batch workflow.

## Moves

| Old runtime path | Canonical runtime path | Archive path |
|---|---|---|
| `src/arxiv_archive/validation_batch_provenance.py` | `src/research_graph/workflows/validation/batch_provenance.py` | `archive/package-rename-waves/wave-16/src/arxiv_archive/validation_batch_provenance.py` |
| `src/arxiv_archive/validation_batch_state.py` | `src/research_graph/workflows/validation/batch_state.py` | `archive/package-rename-waves/wave-16/src/arxiv_archive/validation_batch_state.py` |
| `src/arxiv_archive/validation_batch_workflow.py` | `src/research_graph/workflows/validation/batch_workflow.py` | `archive/package-rename-waves/wave-16/src/arxiv_archive/validation_batch_workflow.py` |
| `src/arxiv_archive/validation_logging.py` | `src/research_graph/workflows/validation/logging.py` | `archive/package-rename-waves/wave-16/src/arxiv_archive/validation_logging.py` |

## Verification Notes

- Validation batch modules are local-only, idempotent, and do not perform external network calls.
- They emit only redacted CLI provenance and structured batch state; no raw paper text or embeddings are serialized.
- Failure-state logging remains explicit and secret-safe; no MiniMax/GLM provider credentials are logged.
