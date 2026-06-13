# M060b Graph Layer Report

## 1. Сводка

M060b подтвердил промежуточный NetworkX-слой для M058 four-layer graph: `3421` узел, `9418` рёбер, density `0.0008`, `135` weak components. S01 дал статистику и validation, S02 добавил PNG-визуализацию, алгоритмический 2-hop BFS preview и closeout для перехода к M061.

Safety boundary сохранён: Production import is not authorized. Graph writes are disabled. External network access is disabled. LLM calls are disabled. Fact promotion is disabled. Loopback bind host: `127.0.0.1`.

## 2. Визуализация

Артефакт: `artifacts/m060b-graph/graph-viz.png`.

Визуализатор читает `artifacts/m058-pilot/combined-edges.json`, строит `networkx.DiGraph`, выбирает top-degree подграф до `200` узлов и раскладывает его через `spring_layout(seed=42)`. Цвет рёбер задаётся слоем: citation — blue, table_similarity — green, figure_similarity_v1 — orange, figure_similarity_v2 — red. Размер узлов масштабируется по degree, alpha рёбер берётся из similarity или равна `0.3` для слоёв без similarity.

В текущей среде `matplotlib` отсутствует, поэтому CLI использовал deterministic stdlib PNG fallback без добавления зависимостей. Fallback сохраняет тот же NetworkX `spring_layout`, layer colors, degree-scaled nodes и alpha-by-similarity/default `0.3`.

## 3. 2-hop BFS preview

Артефакт: `artifacts/m060b-graph/two-hop-preview.json`.

Anchor: `2605.18747`. Preview является algorithm-only estimate, не acquisition. Направленный обход по существующему one-hop manifest дал:

- 1-hop unique nodes: `171`
- 1-hop unique edges: `171`
- 2-hop new unique nodes: `2487`
- 2-hop unique edges / M061 estimated edges: `4454`
- per-layer two-hop edge counts: citation `4454`

Интерпретация: M061 должен планироваться как расширение масштаба примерно до `2487` новых 2-hop узлов и `4454` traversed citation edges в рамках текущего manifest-only estimate. Это не заменяет реальную acquisition-проверку.

## 4. Gate для M061

M060b закрывает read-only graph layer: статистика, validation, visualization и BFS scale preview доступны как воспроизводимые CLI-артефакты. Следующий gate для M061: использовать `two-hop-preview.json` как оценку масштаба, сохранить пять safety defaults false, не включать graph writes без отдельного решения и не продвигать preview-оценки в факты без acquisition evidence.

M045 trajectory status должен оставаться `on_track`, M044 sidecar guardrail должен оставаться `ok`; эти проверки выполняются отдельно в финальной верификации S02.
