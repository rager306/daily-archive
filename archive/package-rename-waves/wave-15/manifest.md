# Package Rename Wave 15 Manifest

Scope: external API boundary modules.

## Moves

| Old runtime path | Canonical runtime path | Archive path |
|---|---|---|
| `src/arxiv_archive/arxiv_client.py` | `src/research_graph/infrastructure/corpus/sources/arxiv_client.py` | `archive/package-rename-waves/wave-15/src/arxiv_archive/arxiv_client.py` |
| `src/arxiv_archive/semantic_scholar.py` | `src/research_graph/infrastructure/corpus/sources/semantic_scholar.py` | `archive/package-rename-waves/wave-15/src/arxiv_archive/semantic_scholar.py` |
| `src/arxiv_archive/ladybug_client.py` | `src/research_graph/infrastructure/graph/ladybug_client.py` | `archive/package-rename-waves/wave-15/src/arxiv_archive/ladybug_client.py` |
| `src/arxiv_archive/telegram_sender.py` | `src/research_graph/infrastructure/ops/notifications/telegram_sender.py` | `archive/package-rename-waves/wave-15/src/arxiv_archive/telegram_sender.py` |

## Verification Notes

- All four modules are external-API side-effect boundaries; tests must keep network and external services mocked.
- `ladybug_client` continues to be a real LadybugDB client; the call sites must stay inside `research_graph.graph` only when the calling module has explicit graph-write authorization.
- `telegram_sender` is an explicit notification side-effect; it never mutates KG state or performs retrieval.
- `arxiv_client` and `semantic_scholar` are network read clients; they never write to KG or queue state.
