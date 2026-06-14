# M062 fd production hardening — итоговый отчёт

## 0. Резюме M062

M062 закрывает production-hardening интеграции daily-archive с fd embedding service. Завершены 4 slice: S01 wrapper hardening, S02 ADR-019, S03 contract tests и S04 closeout. Цепочка коммитов M062: `5dd23ad`, `0184ab4`, `353ff21`, `0122b83`, плюс текущий S04 closeout commit.

Ключевой результат: интеграция теперь имеет единый embedder wrapper, retry/backoff, circuit breaker, graceful degradation, метрики, формальный binding-контракт ADR-019 и 52 contract test cases. Всего подтверждено больше 25 регрессионных проверок; только S03 даёт 52 contract cases, из них 40 passed, 5 failed, 7 skipped. По feedback S01v2 конфигурация fd вынесена в environment variables, без hardcoded endpoint/model/dimensions в source code.

## 1. Контекст

M062 был запущен как fd production hardening после состояния fd v1: рабочий `/v1/embeddings`, но неполная production-поверхность для observability, versioning, error semantics и response headers. Авторитетная спецификация fd v2 — `/root/fd-v2.md`: 32KB, 873 lines, 34 requirements. В текущей среде contract report также фиксирует readable mirror `/root/fd/docs/fd-v2.md`, потому что основной путь мог отсутствовать во время S03 execution.

Цель M062 — не переписать fd, а сделать daily-archive готовым к production use: caller-side wrapper должен выдерживать transient failures, контракт должен быть binding, а gaps fd v1 vs fd v2 должны быть измерены contract tests.

## 2. S01 wrapper hardening

S01 ввёл единый canonical `Embedder` в `src/arxiv_archive/embedder.py`. Wrapper покрывает retry/backoff, circuit breaker, graceful degradation, per-request timeout, batch-size limits и metrics snapshot для диагностики. Safety defaults сохранены закрытыми: graph writes, production import, fact promotion, external network и LLM calls остаются `False` по умолчанию.

S01/S01v2 verification: 10 unit tests wrapper hardening + 4 env override tests = 14 tests. Commit history: `5dd23ad` добавил wrapper hardening, `0184ab4` вынес fd configuration в environment variables.

## 3. S02 ADR-019

S02 оформил `doc/adr/ADR-019-fd-embedding-service-contract.md` как binding-контракт daily-archive для fd embedding service. ADR-019 принимает fd v2 contract как authoritative: endpoints, OpenAI-compatible request/response shape, health/metrics, response headers, machine-readable errors, OpenAPI sketch и acceptance suite.

Verification S02: 8 tests подтверждают наличие ADR-019, binding status, OpenAPI sketch, error catalog, ADR index, codebase-memory mirror linkage и safety defaults. Commit: `353ff21`.

## 4. S03 52 contract tests

S03 добавил `scripts/test_fd_contract.py` и machine-readable outputs в `artifacts/m062-fd-contract/`. Suite содержит 52 checks: 45 fd-side contract cases, 3 wrapper-side checks и 4 env override checks. Категории: endpoints 5/5 passed, env 4/4 passed, happy path 10/10 passed, wrapper 3/3 passed, headers 8/10 passed или skipped, performance 3/5 passed, error semantics 7/15 passed или skipped.

Фактический итог S03: `total=52`, `passed=40`, `failed=5`, `skipped=7`. Это не блокирует M062: цель S03 — измерить fd v1 gaps относительно fd v2, а не скрыть отсутствие `/version`, `/info`, `/openapi.json`, `/docs` или полный набор production headers. Regression layer для M062 теперь покрывает 24 обычных pytest checks из S01/S02/S03 без live fd integration. Commit: `0122b83`.

## 5. Env-driven config

Главный урок S01v2: fd service configuration не должна быть hardcoded. В runtime environments отличаются endpoint, base URL, model id, dimensions, batch size, timeout, retry policy и circuit breaker thresholds. Поэтому M062 закрепляет 10 contract-level `FD_*` variables:

- `FD_EMBEDDINGS_ENDPOINT`
- `FD_EMBEDDINGS_ENDPOINT_BASE`
- `FD_MODEL_NAME`
- `FD_DIMENSIONS`
- `FD_BATCH_SIZE`
- `FD_REQUEST_TIMEOUT_SECONDS`
- `FD_MAX_RETRIES`
- `FD_RETRY_BACKOFF_SECONDS`
- `FD_CIRCUIT_FAILURE_THRESHOLD`
- `FD_CIRCUIT_OPEN_SECONDS`

Defaults используют `http://127.0.0.1:8000` и сохраняют backward compatibility. Реализация читает `os.environ` через helpers `_env_str`, `_env_int`, `_env_float`, `_env_bool`, `_env_list`. Обновлены embedder и 4 fd-related scripts (`scripts/m057_*`, `scripts/m058_*`). ADR-019 S04 amendment фиксирует это как 12-factor app pattern.

## 6. Пятислойная архитектура

После M062 структура интеграции стала явной:

1. `Embedder` wrapper с env-driven config, retry, circuit breaker, graceful degradation и metrics.
2. fd v1 service как текущая фактическая реализация: embeddings работают, но `/version` и часть production endpoints отсутствуют.
3. Contract tests как executable evidence: 52 checks отделяют daily-archive wrapper health от fd implementation gaps.
4. ADR-019 как binding contract: будущие изменения fd сверяются с ним, а не с ad hoc expectations.
5. Closeout artifacts как handoff layer: REPORT, SUMMARY, VALIDATION и codebase-memory mirror дают будущим агентам быстрый вход.

Эта архитектура позволяет продолжать M063 GraphDB selection без смешивания graph decision и fd hardening.

## 7. ADR-018 trigger evaluation, lessons и следующие milestones

ADR-018 trigger evaluation: M062 закрывает fd hardening scope. M063 (GraphDB selection) остаётся следующим шагом, потому что fd-side риски теперь формализованы и не должны блокировать выбор graph backend.

Lessons:

- Hardening wrapper и formal contract должны идти вместе: retries без error catalog создают ложную устойчивость.
- Contract tests полезны даже при failures: они показывают, какие fd v2 requirements уже MET, PARTIAL или MISSING.
- Env-driven config нужно требовать сразу; hardcoded endpoint/model/dimensions быстро ломают CI, staging и production.
- Safety defaults должны оставаться закрытыми, даже когда включается live-service integration.

Next milestone queued: `M062-fd-v2-verification` после fd v2 deploy. Его задача — повторить 52 contract checks против fd v2 и подтвердить, что gaps из `fd-actual-vs-required.md` закрыты либо явно deferred новым ADR.
