# PLAN: параллельный эксперимент «план и контроль без GSD» (2026-10-11)

Изолированный worktree: ветка feat/learning-critic-e1. Основной чекаут
(/root/daily-archive) занят GSD-движком M516 — НЕ ТРОГАЕМ.
3 задачи, непересекающиеся файлы, параллельные субагенты.

## Контроль (гейты каждой задачи)
1. cargo: check/clippy(-D warnings)/test по своему крейту
2. governor --phase active --check-only (до и после)
3. acceptance-критерии задачи (ниже)
4. финал: ревью владельцем, мерж в основную линию

## T-A: P0 — дискриминация learning-критика
- Файлы: tools/learning-critic-ruvllm/
- Проблема (аудит Н-1): критик даёт const 1.0 — пустые профили скорятся 1.0,
  gain недостижим, гейт s07_measured_no_gain не закрывается честно.
- Acceptance: (1) well-formed vs degraded subjects -> gap метрик >= 0.15;
  (2) пустой/деградированный профиль < 0.8; (3) детерминизм (повтор = тот же
  скор); (4) юнит-тесты дискриминации; (5) регресс: frozen corpus прогон.
- Модель исполнителя: glm-5.3 (max)

## T-B: da-mcp — resources/prompts наружу через rmcp router
- Файлы: crates/da-mcp/
- Проблема (аудит ADR-054): 14 tools наружу, resources/prompts только внутренние
  методы, HTTP-транспорта нет.
- Acceptance: MCP-контракт расширен resources+prompts без поломки tools;
  тесты протокола; cargo check/test -p da-mcp.
- Модель исполнителя: gpt-6-luna (max)

## T-C: pluggable rerankers для hybrid search
- Файлы: crates/kg-embeddings/
- Проблема (аудит ADR-073): RRF есть (k=60), pluggable rerankers
  (node_distance, episode_mentions) = 0 реализаций.
- Acceptance: trait Reranker + >= 2 реализаций + тесты + интеграция в hybrid_rrf
  (не ломая существующий контракт).
- Модель исполнителя: glm-5.3-flash

## Порядок мержа
Все три — в эту ветку последовательными коммитами. Ревью владельцем.
Основная линия (feat/da-parse) мержится ПОСЛЕ закрытия M516.
