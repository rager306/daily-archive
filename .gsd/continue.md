# Continue — M429 implemented (pending GSD seal)

## Last action

M429-zz7b1b S01+S02 implemented (code green; GSD skip-delivered closeout next):

- `write_node_embedding` → `GraphResult<NodeEmbedWrite>`: двойной провал
  записи и провал sha-штампа больше не глотаются. Красный тест с
  failing-store double доказал: старый код возвращал `Replaced` при
  полностью провалившейся записи (фейковый успех); новый — `Err`.
- Потребители: ingest фейлит материализацию с контекстом; entity-loop
  логирует видимый `entity_embedding_write_failed` и продолжает
  (политика как у embed-ошибок).
- STUB-AUDIT fake-fixed; governor пин `embedding_write_honest`
  (required `GraphResult<NodeEmbedWrite>`, forbidden `let _ = graph_store`).
- da-application lib 304; batch_ingest 20; embedding 5; clippy clean;
  governor ON_TRACK flags=0.

Волна: M420→M429 (9 milestones; 5 продуктовых багов).

## Next action (priority)

1. GSD skip-delivered closeout for M429.
2. Продолжить волну: grep-скан da-cli/da-adapters на `let _ =` /
   `.ok()` на критичных путях того же класса.

## Do not

Unlock D127. Pin a fake. `if let Ok` / `let _ =` на engine/store
вызовах. Governor-пины писать по фактической сигнатуре (алиас
GraphResult, не развёрнутый Result).
