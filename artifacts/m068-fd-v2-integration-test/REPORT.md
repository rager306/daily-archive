# M068: отчёт интеграционного теста fd v2

## 0. Резюме

Статус S03: **SKIP**. Выбрано документов M061: **150**; обработано через fd v2: **0**. Пропускная способность: **0.00 документов/мин**. Задержки: p50 **н/д мс**, p95 **н/д мс**, p99 **н/д мс**. Доля ошибок: **н/д**. Причина SKIP, если применимо: ключ FD_API_KEY не настроен; защищённый fd v2 запрос не может быть проверен.

## 1. Контекст

M068 закрывает проверку M062-fd-v2-verification после двух предыдущих шагов. S01 добавил поддержку новой конфигурации окружения в daily-archive, а S02 повторно прогнал контракт fd v2 и выпустил отчёт `artifacts/m062-fd-contract/fd-contract-report-v2.md`.

## 2. S01 env vars update

S01 подтвердил пять новых переменных: `FD_API_KEY`, `MODEL_ID`, `TEI_URL`, `REDIS_HOST`, `REDIS_PORT`. Ключ `FD_API_KEY` используется только как `Authorization: Bearer` из окружения и не сохраняется в артефактах. Модель берётся из `MODEL_ID`, адрес fd v2 — из `TEI_URL`, а Redis-настройки остаются env-driven для следующего этапа очереди.

## 3. S02 contract tests v2

S02 зафиксировал контрактный baseline: **total=52, passed=8, failed=0, skipped=44**. Детализация по 52 проверкам находится в `artifacts/m062-fd-contract/fd-contract-report-v2.md`; категории включают endpoints, env, error, happy, headers, performance и wrapper. Пропущенные проверки объясняются отсутствием доступного защищённого fd v2 сервиса в текущей среде.

## 4. S03 integration test

Скрипт `scripts/m068_integration_test.py` выбрал 5 anchors × 30 документов из `artifacts/m061-2hop/anchor-*/acquisition/selected-2hop-papers.json`. Результаты записаны в `artifacts/m068-fd-v2-integration-test/results.json`. Текущий статус: **SKIP**; обработано **0** из **150**; successful **0**; failed **0**; throughput **0.00 документов/мин**; latency p50/p95/p99 **н/д/н/д/н/д мс**.

## 5. v1 -> v2 comparison

v1 оставался пригоден для базового OpenAI-compatible happy path, но не доказывал полный P0/P1/P2 контракт ADR-019. v2 baseline M068 показывает, что wrapper/env-слой готов: env-проверки и wrapper-проверки проходят, а сетевые проверки честно помечаются SKIP при недоступном защищённом сервисе. Это лучше прежнего состояния: отсутствие fd v2 больше не маскируется под успешную интеграцию.

## 6. ADR-019 update

ADR-019 обновлён второй записью Amendment Log: fd v2 env config явно включает `FD_API_KEY`, `MODEL_ID`, `TEI_URL`, `REDIS_HOST`, `REDIS_PORT`. ADR index оставляет ADR-019 binding и отмечает наличие двух записей журнала поправок.

## 7. Lessons + next milestones

M064 должен подключить очередь через `REDIS_HOST` и `REDIS_PORT`, сохраняя отключённые по умолчанию опасные действия. M066+ должен продолжить PostgreSQL-интеграцию и использовать results/report как evidence для решения, когда fd v2 станет доступен. Пять safety defaults остаются выключенными:

- `graph_writes_authorized`: `false
- `production_import_authorized`: `false
- `fact_promotion_authorized`: `false
- `external_network_authorized`: `false
- `llm_calls_authorized`: `false
