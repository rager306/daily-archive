# Baseline прогресса поглощения GROBID (M514-1lss2s/S01)

Статус: **поставлен baseline измерительной системы** (`M514-1lss2s/S01/T05`). Это точка
отсчёта для честного измерения продуктового прогресса, а не релизное решение и не
заявление о качестве продукта. Проза релиз не авторизует; `import_eligible=false`,
`deployed=false`, `production_route=forbidden`, `graph_write=forbidden`,
`auto_promote=forbidden` остаются в силе.

Числа ниже — не пересказ и не пере-округление: каждый скаляр воспроизводится из
замороженного probe и сверяется целочисленным тождеством тестом
`scripts/tests/test_absorption_progress_baseline.py`. Машиночитаемые пины живут в JSON
(`artifacts/parse_compat/absorption_progress_baseline.json`), а не в этой прозе; этот
документ — читаемая проекция.

## 1. Привязка носителей

| Роль | Путь | sha256 | identity |
|---|---|---|---|
| frozen probe (источник чисел) | `artifacts/parse_compat/external_holdout_release_probe.json` | `a937da95ef987dffdcde46b998e98549fe247999e50986e1b0627a6f485d8275` | `external-holdout-release-probe.v1` |
| baseline scorecard | `artifacts/parse_compat/absorption_progress_baseline.json` | `c4549c865cbdb1c98f55d2b7e4e924ce56812bf6611013f808af6819ff90338b` | `absorption-progress.v1` |
| контракт (схема, T01) | `data/parse/absorption_progress.schema.yaml` | `a417f88a40b8ba22ea2f70cffb9bb8cc9e914dbb9e63f17969d6268df2292cc3` | — |
| rule DATA (T04) | `scripts/governor/checks/grobid_absorption_progress.yaml` | `60b9693acc59fdc3dfb5238b3503ca989fbd6ab40a080400d352d61cc9d323ad` | `grobid-absorption-progress.v1` |
| доменный валидатор (T01) | `scripts/governor/domain/absorption_progress.py` | `4af8fd0bc67b23b149324ba9b695126dc9463adb26bacb92dcb95df1994b2047` | — |
| oracle/reader (T04) | `scripts/governor/adapters/absorption_progress_reader.py` | `2e06bf765ecfbe8b4da3d5614c8aa3b07a202bd4257a2a2292ed44c47c2ad277` | — |

Хэши зафиксированы на момент этого baseline. **Принудительно проверяются** пины из
`historical_guard_pins` в JSON, а также одна строка этой таблицы: хэш baseline scorecard
сверяется тестом `test_report_pins_the_baseline_artifact_hash` из
`scripts/tests/test_absorption_progress_baseline.py`, поэтому отчёт не может описывать
старые байты артефакта после его правки. Остальные строки — информационные (S02
заморозит пороги правкой схемы, и её хэш здесь станет stale — это ожидаемо, а не
нарушение).

## 2. Замороженная перепись (знаменатель)

`census_id = holdout-partition-610-531-79`, `read_only: true`.

```
assigned 610 = executed 531 + refused 0 + excluded 79
```

Полное назначение остаётся в знаменателе: `denominator_basis = assigned_full`,
`excluded_stays_in_denominator = true`. Исключения типизированы и перечислены:

| reason | count |
|---|---|
| `header_date_absent` | 72 |
| `header_title_absent` | 5 |
| `header_author_absent` | 2 |

Покрытие документами: `531 / 610 = 0.870492`. Пространство счёта —
`79621` occurrence в знаменателе. Scorecard, который молча выбросит или
переклассифицирует одно из 79 исключений, не сойдётся к `assigned` и падает fail-closed.

## 3. Измеренные оси

Обе строки используют `denominator_basis: assigned_full` (не `measured_only`), то есть
отношение уже сужено полосой, а не претендует на корпусную ставку.

### `preservation.occurrence_conservation` — ось `preservation`

| слой | значение | доля |
|---|---|---|
| micro (occurrence) | `74910 / 79621` | `0.940832` |
| weakest stratum `per_document` | `101 / 205` | `0.492683` |

Слабейший документ: `2501.17399` (`weakest_document_id`), `weakest_stratum_id =
per_document`. Сохранность записана **отдельной осью** и не пулится в извлечение:
`preservation_pooled_with_extraction` — типизированное нарушение.

### `extraction.native_capability_recall` — ось `extraction`

| слой | значение | доля |
|---|---|---|
| micro (capability recall) | `945771 / 1000000` | `0.945771` |
| weakest stratum `per_family` | `3055 / 7077` | `0.431680` |

Слабейшее семейство: `SemanticFieldLoss::AuthorAffiliation` (`weakest_family_id`).

### Идентичность сравнения

`comparability = {corpus_id: m513-holdout-610, profile: structural, oracle:
external_holdout_probe_semantics}` — сравнивать можно только внутри этой серии. У каждой
строки `source_revision` **равен** хэшу её evidence-артефакта, то есть скаляр привязан к
байтам, а не заявлен. Другой корпус/профиль/oracle → `scope_not_accepted`, а не ложная
регрессия; две серии в одном scorecard → `scope_mixed_in_scorecard`.

## 4. Неизмеренные оси (реальные незакрытые исходы)

| ось | state | reason |
|---|---|---|
| `java_independence` | `not_measured` | `java_production_route_active` |
| `resources` | `not_measured` | `native_savings_not_measured` |
| `reliability` | `not_measured` | `rehearsal_only_not_a_product_measurement` |

`not_measured` — это не ноль: строка в состоянии `not_measured` **не имеет** поля
`score`, а постановка числа под ним (`empty_population_given_value`,
`value_with_not_measured`) — нарушение.

Дополнительно baseline не утверждает (перенесено из `named_limitations` probe):

- продуктивный маршрут остаётся Java (`java-grobid-http/0.9.1/processFulltextDocument`,
  профиль `structural`; rehearsal-fallback — `java-grobid-http@0.9.0-crf`); native PDF→TEI
  маршрут не соединён с runner'ом, поэтому java-free продуктового замера не существует;
- экономии ресурсов нет: парного CPU-замера той же нагрузки не существует
  (`savings_without_paired_cpu`), а wall-clock-only сбережение продуктовым замером не
  является; latency и cost — `bounded_not_measured`, unknown never zero;
- внешняя holdout-полоса не исполнена (`execution_status: not_executed`,
  `external_bytes: bytes_absent`); внешние строки не являются evidence, и релизное
  решение именованного человека по-прежнему требуется;
- 531 документ — не статистический корпус; каждое число — счётчик, не корпусная доля;
  pooled-заявление запрещено (`pooled_quality_claim: forbidden`);
- PDF-route корридор проверен на 3 документах (`corpus_n = 3`) — это `local_proxy_only`,
  не native-PDF claim.

## 5. Авторизация — отдельно от охвата

`authorization.state = owner_directive_research`, `decision = D491`,
`product_coverage_effect = none`, `authority = data/parse/research_authorization.yaml`.
Авторизация не даёт ни релиза/распространения, ни graph write, ни import, ни охвата:
`authorization_used_as_product_evidence` — нарушение.

## 6. Как проверяется измерительная система

Три слоя, каждый со своим закрытым словарём:

1. **контракт T01** (`data/parse/absorption_progress.schema.yaml` + `governor/domain/`)
   — знаменатель, состояния, страты, comparability, пороги, авторизация, lifecycle;
2. **oracle T04** (`adapters/absorption_progress_reader.py` + rule DATA) — привязка
   скаляра к байтам (hash/substitution/relative path), сравнение только внутри
   accepted-серии, ресурсные пары, направление регрессии;
3. **baseline T05** (этот артефакт + `test_absorption_progress_baseline.py`) — все числа
   пересчитываются из замороженного probe, а не утверждаются.

**Положительный контроль:** на реальном дереве композитный oracle даёт `0` нарушений.

**Мутационная матрица (негативные пробы):** каждая мутация вносится в *реальный*
baseline один раз и обязана дать свою метку; выжившая мутация — провал теста.

| мутация | ожидаемая метка |
|---|---|
| подменён declared evidence digest | `evidence_hash_substituted` |
| evidence path сделан абсолютным | `evidence_path_not_relative` |
| привязан несуществующий путь | `evidence_artifact_missing` |
| изменён oracle строки | `oracle_changed` |
| строка выведена из accepted-корпуса | `scope_not_accepted` |
| знаменатель сужен ниже census | `denominator_shrink` |
| удалено типизированное исключение | `unnamed_exclusions` |
| причина исключения вне словаря | `exclusion_reason_untyped` |
| терминальное GSD-состояние как «delivered» | `skipped_as_delivered` |
| preservation пулится с extraction | `preservation_pooled_with_extraction` |
| изобретён числовой порог до S02 | `threshold_invented_before_freeze` |
| измеренная ось замолчала | `coverage_regression` |
| экономия без парного CPU | `savings_without_paired_cpu` |
| авторизация подняла охват | `authorization_used_as_product_evidence` |

**Граница (E3, без изменений):** это consistency/fail-closed контракт поверх того, что
scorecard заявляет о себе, плюс пересчитанный content hash. Это **не** tamper-evident
attestation: актор, способный переписать scorecard, evidence и DATA-правило, удовлетворит
контракт. Ценность в том, что отсутствие, дрейф, смешение scope и самоопровержение
перестают проходить как compliance, а число охвата обязано называть породивший его
артефакт.

## 7. Исторические носители (read-only, не тронуты)

`historical_carriers.mode = read_only`, `rewriting_forbidden: true`,
`new_evidence_is_separate: true`. Семь пинов (побайтово, проверяются baseline-тестом):

| носитель | sha256 |
|---|---|
| `doc/review/M513-S07-release-decision-package.md` | `e6aa03ccbcafd09d02c48e504e35883891196cba3b27bc261ae8f83da8ec5ad7` |
| `doc/review/HUMAN-REVIEW-TEMPLATE.md` | `05fc2461db2bc8ea0cc0b090ff326646c1ed80b97cc5177290afa2604701ce91` |
| `artifacts/parse_compat/grobid_retirement_release_decision_package.json` | `f4ff294193a41c42988aaa135ec1b728f04603c6a585365c97a2d82869f0aa9a` |
| `data/parse/acceptance_manifest.yaml` | `4d1c55ea9dd873b4aada48aa964609395f59e8a726bdf42c65d2b0f36865ca06` |
| `data/parse/grobid_compatibility.yaml` | `de50c48ab8539dbc559bb077e4913e83656f5ecb47d5bd03f6b721cfb004fe03` |
| `data/parse/holdout_manifests.yaml` | `ef91d3c25c7d91dda2654471140fd41fcae8e1ed87e7094ad24c31fef2b64a39` |
| `artifacts/parse_compat/external_holdout_release_probe.json` | `a937da95ef987dffdcde46b998e98549fe247999e50986e1b0627a6f485d8275` |

Живая запись человека (`M513/S07`) остаётся `pending_human_review`;
`reviewer=null`, `decision=pending`. Новая evidence по поглощению публикуется отдельно и
никогда не переписывает историю.

`M513/S03 UAT-4` (`id: m513-s03-uat-4`, `kind: uat_check`) сохранён как **идентичность**:
он остаётся согласованным со всеми поверхностями, но никогда не переисполняется и не
переиздаётся как свежее evidence.

## 8. Словарь меток: два слоя (D492)

`violations` в rule DATA — это **только** adapter-слой T04 поверх доменного словаря T01
из `data/parse/absorption_progress.schema.yaml`/`domain/absorption_progress.py`. Тест
`test_every_committed_label_is_registered_in_a_governed_vocabulary` пиннит разделение:
каждая adapter-метка объявлена в rule DATA, каждая доменная метка существует литералом в
валидаторе T01, и множество меток не пересекает оба слоя. Метка, заявленная не в своём
слое, — провал, а не тихое согласие.

## 9. Воспроизведение

```bash
# baseline-контроль (числа, негативы, позитив, исторические пины)
PYTHONPATH=scripts python3 -m pytest scripts/tests/test_absorption_progress_baseline.py -q

# композитный oracle (T01 domain + T04 identity/direction)
PYTHONPATH=scripts python3 -m pytest \
  scripts/tests/test_absorption_progress_governor.py \
  scripts/tests/test_absorption_progress.py -q

# governor: реальное дерево не должно давать absorption_progress_* флагов
PYTHONPATH=scripts python3 -m governor --phase preflight
```

## 10. Свидетельства проверки

### 10.1 Что и как проверялось (факты)

Все прогоны — на `25743c7c455938113dda9067b767e1074e45c42e` плюс незакоммиченные
T05-файлы. Rust-исходники этой задачей не менялись; раздел 10.2 объясняет, почему это
не превращается в утверждение об отсутствии регрессии. Строки 1–14 — запись прогонов, на
которых baseline был поставлен, **до** правки T05; раздел 10.3 содержит воспроизводимые
числа с текущей ревизией и не переписывает эти строки.

| # | Проверка | Команда | Exit | Вердикт | Evidence |
|---|---|---|---|---|---|
| 1 | baseline-сьют (числа, негативы, пины) | `pytest scripts/tests/test_absorption_progress_baseline.py -q` | 0 | 22 passed | `gsd_exec 320432c8` |
| 2 | absorption-сьюты T01+T03+T04+T05 | `pytest ... test_absorption_progress{,_governor}.py test_absorption_test_inventory.py -q` | 0 | 84 passed | `gsd_exec a23a18e8` |
| 3 | governor YAML | `python3 -m governor --validate-yaml` | 0 | PASS (65 rules, 20 dimensions) | `gsd_exec 04dfcce7` |
| 4 | preflight (реальное дерево) | `python3 -m governor --phase preflight` | 0 | DRIFT_RISK, 9 флагов, **0** `absorption_progress_*` | `gsd_exec f68444fd` |
| 5 | architecture gate, прогон #1 | `python3 -m governor architecture` | **1** | **18/19 шагов**; упал шаг `da-parse tests` | не сохранён (10.2) |
| 6 | architecture gate, прогон #2 | `python3 -m governor architecture` | 0 | 19/19 шагов, ARCHITECTURE GATE: PASS (1903 s) | `/tmp/m514-architecture-gate2.log` |
| 7 | полный governance-сьют | `pytest scripts/tests/ -q` | 0 | **1087 passed** (1393 s) | `/tmp/m514-pytest-full.log` |
| 8 | UAT-1 baseline чист под композитным oracle | node-selection pytest | 0 | 2 passed | `gsd_exec 3ef7cdd5` |
| 9 | UAT-2 числа пересчитаны из frozen probe; полный знаменатель и `not_measured` оси | node-selection pytest | 0 | 2 passed | `gsd_exec ead1daf4` |
| 10 | UAT-3 негативная матрица (14 мутаций + регистрация меток) | node-selection pytest | 0 | 15 passed | `gsd_exec 9a3b15b6` |
| 11 | UAT-4 исторические пины побайтово; носители read-only | node-selection pytest | 0 | 2 passed | `gsd_exec c00c40fe` |
| 12 | изоляция провала гейта | `cargo test -p da-parse --test backend_contract` | 0 | 23 passed, 0 failed | `gsd_exec 71a6e424` |
| 13 | точный шаг гейта в изоляции | `cargo test -p da-parse --tests` | 0 | 0 failed (263 s) | `gsd_exec d51a35ed` |
| 14 | rustfmt | `cargo fmt --all --check` | 0 | clean | последнее действие задачи |

Строки 1–4 и 7–13 — то, что baseline утверждает о **себе**: они воспроизводятся из
замороженного probe и негативных проб, а не заявляются. Строки 5–6 — история
**инструмента**, не продукта: прогон #1 упал, прогон #2 прошёл.

### 10.2 Провал прогона #1: что известно и что нет

**Наблюдавшиеся факты:**

- прогон #1 дал `exit 1`: 18/19 шагов, упал шаг `da-parse tests`
  (`cargo test -p da-parse --tests`);
- детальный capture провала **не сохранён**: гейт печатал только последние пять строк
  `stderr` и выбрасывал остальное, поэтому лог не содержит ни имени упавшего теста, ни
  его вывода;
- следовательно, **конкретный failing test прогона #1 неизвестен**, как неизвестен и
  его режим отказа;
- прогон #2, серийный и без параллельной нагрузки, дал `exit 0`: 19/19 шагов.

**Чего из этого не следует:**

- отсутствие регрессии **не установлено** — запись прогона #1 слишком бедна, чтобы это
  утверждать или опровергать;
- причина провала #1 **не установлена**; тезис «это флейк под нагрузкой» — гипотеза, а
  не вывод из записанных данных;
- зелёный прогон #2 **не является** доказательством причины и не объявляется
  устранением флейка: это второе наблюдение, а не `root-cause proof`.

**Гипотеза (не подтверждена, записана как гипотеза).** Провал мог быть
средозависимым взаимодействием внутри тестового бинарника `da-parse`, а не регрессией
исходников. Один из кандидатов — форма bind-then-drop в
`http_backend_surfaces_transport_outage_as_unavailable`: тест биндит эфемерный порт,
немедленно закрывает его и ожидает отказа соединения, поэтому между `drop` и `connect`
порт теоретически может быть переиспользован. Ни одна запись прогона #1 эту гипотезу
не подтверждает; изолированные прогоны того же шага и `backend_contract` дали `0 failed`
— это совместимо с гипотезой, но её не доказывает.

**Что изменено, чтобы следующее падение атрибутировалось** (`M514/S01/T05`,
`scripts/governor/__main__.py`): гейт сохраняет полный `stdout` и `stderr` каждого
упавшего шага в gitignored `target/architecture-gate-logs/`, печатает имена упавших
libtest-тестов (`test <name> ... FAILED`) и обе хвостовые части потоков. Поведение
закреплено детерминированным негативным тестом с fake subprocess —
`scripts/tests/test_absorption_test_inventory.py::test_gate_runner_persists_failing_step_capture_and_names_the_test`.
Машиночитаемая запись обоих прогонов, гипотезы и её статуса живёт в `gate_run_history`
baseline-артефакта; этот раздел — читаемая проекция.

Гонка не подтверждена, поэтому тест не переписывался: объявить «детерминизм» без
доказанной причины было бы таким же необоснованным заявлением, как прежнее
«доказанная гонка».

### 10.3 Повторная проверка исправления T05 (после правки §10 и диагностики гейта)

Числа ниже — фактические на этой ревизии дерева. Они не переписывают 10.1: те строки
описывают прогоны до правки.

| # | Проверка | Команда | Exit | Верификация | Evidence |
|---|---|---|---|---|---|
| 15 | целевые сьюты (verify задачи) | `pytest scripts/tests/test_absorption_test_inventory.py scripts/tests/test_absorption_progress_baseline.py -q` | 0 | 39 passed | `gsd_exec d174e0b4` |
| 16 | inventory-сьют (обязательная проверка finding) | `pytest scripts/tests/test_absorption_test_inventory.py -q` | 0 | 14 passed | `gsd_exec a5ae8aef` |
| 17 | baseline-сьют отдельно (+3 новых теста) | `pytest scripts/tests/test_absorption_progress_baseline.py -q` | 0 | 25 passed | `gsd_exec a5ae8aef` |
| 18 | absorption-сьюты T01+T03+T04 | `pytest scripts/tests/test_absorption_progress.py scripts/tests/test_absorption_progress_governor.py scripts/tests/test_absorption_test_inventory.py -q` | 0 | 64 passed | `gsd_exec a5ae8aef` |
| 19 | обязательная проверка finding | `cargo test -p da-parse --test backend_contract` | 0 | 23 passed, 0 failed (0.11 s) | `gsd_exec c7636509` |
| 20 | governor YAML | `python3 -m governor --validate-yaml` | 0 | PASS (65 rules, 20 dimensions) | `gsd_exec f2e6411e` |
| 21 | preflight (реальное дерево) | `python3 -m governor --phase preflight` | 0 | DRIFT_RISK, 9 флагов, **0** `absorption_progress_*` | `gsd_exec f2e6411e` |
| 22 | rustfmt | `cargo fmt --all --check` | 0 | clean | `gsd_exec 38da8f6a` |
| 23 | whitespace | `git diff --check` | 0 | clean | `gsd_exec 38da8f6a` |

Число тестов выросло ровно на пять, по трём новым тестам отчёта
(`test_report_pins_the_baseline_artifact_hash`,
`test_gate_run_history_keeps_the_first_failure_unexplained`,
`test_report_records_the_gate_failure_as_unresolved`) и двум новым тестам диагностики
(`test_gate_runner_persists_failing_step_capture_and_names_the_test`,
`test_gate_runner_keeps_no_capture_for_a_passing_step`); прежние негативные матрицы
проходят без изменений. Полный `governor architecture` намеренно **не** переисполняется
ради снятия замечания: зелёный повтор не является `root-cause proof`, а прогон #2 уже
записан.

## 11. Известные ограничения

1. `scorecard.required` остаётся `false` до reviewable DATA-правки (S02 или конец S01):
   сейчас удаление baseline-артефакта гасит чек вместо находки.
2. Пороги не заморожены (`threshold_policy.state: not_frozen`, owner S02). Любое
   числовое значение до freeze — типизированное нарушение.
3. Пины и evidence-digest живут в том же репозитории, что и носители (см. §6, граница E3).
4. `lineage` в scorecard пуст (`{}`) намеренно: привязка идёт через `evidence.sha256` и
   `source_revision`, а не через отдельный lineage-блок.
5. `gate_run_history` в scorecard — это provenance **инструмента** (два прогона гейта,
   гипотеза и её статус), а не измерение продукта: ни одна продуктовая ось его не
   читает, контракт T01 его игнорирует. Он там, чтобы история обоих прогонов была
   машиночитаемой, а не только в прозе.
