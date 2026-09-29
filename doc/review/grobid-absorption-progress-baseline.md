# Baseline прогресса поглощения GROBID (M514-1lss2s/S01)

Статус: **поставлен baseline измерительной системы** (`M514-1lss2s/S01/T05`),
**пороги зафиксированы как prospective regression floor и привязаны к ревизии
носителя** (`M514-1lss2s/S02/T02`). Это точка
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
| reference pins (независимый floor, не кандидат) | `artifacts/parse_compat/absorption_progress_reference_pins.json` | `2befd08dd8b095a6e8e6ea17e6b73e92e78e282ab98cbad64c4727218d6fe158` | `absorption-progress-reference-pins.v1` |
| baseline scorecard (кандидат) | `artifacts/parse_compat/absorption_progress_baseline.json` | `cc6eb1e9e9524d1df8b9cd02142eee742e61bddc736e40bd75dacbc5f515e29a` | `absorption-progress.v1` |
| контракт (схема, T01/T02) | `data/parse/absorption_progress.schema.yaml` | `4ed78e34cb5863cf0fcbf6c09fed1351e1bb722659c73b881a36b1a16317641a` | — |
| rule DATA (T04/T03) | `scripts/governor/checks/grobid_absorption_progress.yaml` | `ddfd918331cac1ec2312b674f2d61751753bd74dbbac53aba2f166d66df9abbe` | `grobid-absorption-progress.v1` |
| доменный валидатор (T01/T02) | `scripts/governor/domain/absorption_progress.py` | `8c3702bfefa97d893730d60f12c9fac19c2f54e8c1a68efbc19a8bf5d0ab8a19` | — |
| oracle/reader (T04/T03) | `scripts/governor/adapters/absorption_progress_reader.py` | `9c2eef6f400556564fc47caad6ecc8267824af302251686ff3a3a0bb8f6f4eff` | — |

Хэши зафиксированы на этой ревизии (S02/T03). При правке §1 в T03 исправлены уже
разъехавшиеся строки `rule DATA` и `oracle/reader`, а также строка доменного валидатора:
прошлая редакция объявляла байтами то, чего на диске уже не было, — это и есть дрейф
проекции, а не «информационная строка». **Принудительно проверяются** пины из
`historical_guard_pins` в JSON, хэш baseline scorecard (тест
`test_report_pins_the_baseline_artifact_hash` из
`scripts/tests/test_absorption_progress_baseline.py`), и **ревизия заморозки порогов**:
`threshold_freeze.revision` в scorecard равен sha256 этой строки таблицы и
пересчитывается reader'ом на каждом прогоне. Остальные строки — информационные; после
любой следующей правки схемы её строка и строка baseline станут stale, и это уже будет
не «ожидаемой устареваемостью», а находкой `threshold_freeze_revision_mismatch`.
Строка reference pins проверяется тем же тестом: sha256 артефакта floor должен совпадать
с объявленным в схеме `threshold_policy.reference_pins.sha256`, иначе это дрейф floor, а не
тихий ре-базлайн. Остальные строки прозы не перечитываются тестами, но обновлены здесь,
чтобы проекция не описывала несуществующие байты.

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

### 2.1 Продокументная проекция census (S02/T03)

Агрегатные `610/531/79` сами по себе не доказывают, что описана именно та популяция:
потерянный документ или переставленная диспозиция оставили бы счётчики верными. Поэтому
scorecard публикует `population_census`: раздел `assigned/executed/refused/excluded`,
закрытую семантику отказа с **явно перечисленным** (сейчас пустым) набором
`refused_document_ids`, типизированные причины исключений, привязку авторитета (path +
sha256 для probe и manifest), правило вывода и `projection_sha256` над отсортированной
продокументной проекцией `(document_id, disposition)`.

Правило вывода (исполняемое, в `derivation`): assigned — из
`data/parse/holdout_manifests.yaml#articles[*].article_id`; executed — из
`external_holdout_release_probe.json#outcomes[*].document_id`; excluded — из
`...#reference_population.exceptions[*]` с типизированной причиной; refused =
assigned − executed − excluded. Дайджест считается как sha256 по компактной UTF-8
JSON-записи проекции (`separators=(",", ":")`), и то же правило записано в артефакте.

Оракул пересчитывает проекцию и дайджест из этих двух источников и падает на:
`census_projection_mismatch` (проекция/дайджест/раздел/авторитет/census_id не те,
что выведены, или собственные агрегаты probe не сходятся с его списками),
`census_document_dropped` (выведенная идентичность отсутствует в объявленной проекции),
`census_disposition_untyped` (причина вне закрытого словаря),
`refused_set_unbound` (граница отказа не равна явно перечисленному выведенному набору).
Счётчики `610/531/79` при этом по-прежнему проверяет домен
(`census_mismatch`/`denominator_shrink`), а `historical_guard_pins` держит байты обоих
источников: продокументная половина не заменяет агрегатную, а дополняет её.

**Граница:** если блок `population_census` отсутствует, продокументная половина остаётся
*непроверенной* (агрегатный знаменатель всё ещё проверяется доменом). Это consistency-
слой, а не tamper-evident attestation (см. §6).

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

## 3.1 Замороженные пороги как prospective regression floor (S02/T02)

`threshold_policy.state: frozen`, `freeze_owner: S02`, `frozen_at: 2026-09-29`,
`freeze_scope: prospective_regression_floor`, `tuned_after_scoring: forbidden`,
`pass_requires_frozen_threshold: true`.

Что именно зафиксировано и чего здесь **не** утверждается: корпус из 610 документов
измерен в S01, поэтому заморозка — это floor для последующих прогонов по тому же
замороженному корпусу, а не «историческая заморозка до скоринга», не доказательство
приёмки продукта и не доказательство независимости. Равенство текущего кандидата и floor
сегодня — следствие того, что оба выведены из одного замороженного probe, и не является
заявлением об абсолютном качестве.

Floor выведен из замороженного probe в отдельный артефакт
`artifacts/parse_compat/absorption_progress_reference_pins.json`, независимо запинованный
по sha256 в `threshold_policy.reference_pins`. Кандидат (baseline scorecard) в вычислении
floor не участвует: домен отказывает порогу, чей `reference.path` не равен
`reference_pins.path` (`threshold_reference_not_independently_pinned`), поэтому
переписанный score кандидата не может сдвинуть собственный порог. Проверка этого —
негативный тест `test_threshold_reference_does_not_follow_the_candidate_scorecard`:
кандидат переписан согласованно (score изменён, evidence-digest по-прежнему совпадает с
неизменными байтами probe), и резолвнутый порог остаётся запрошенной долей reference.

| metric_id | stratum | family_id | direction | margin | derivation (из pinned reference) |
|---|---|---|---|---|---|
| `preservation.occurrence_conservation` | `micro_occurrence` | `parse.component.native_fulltext_composition_shadow` | `at_least` | `0` | `74910 / 79621` → `0.940832` |
| `extraction.native_capability_recall` | `micro_occurrence` | `stage.reference_segmentation` | `at_least` | `0` | `945771 / 1000000` → `0.945771` |
| `extraction.native_capability_recall` | `weakest_stratum` | `stage.header_composition` | `at_least` | `0` | `3055 / 7077` → `0.431680` |

Числа в контракт **не вписаны** и не читаются из кандидата: каждая запись хранит
`reference` (path + pointer) на pinned reference-артефакт, reader резолвит значение тем же
правилом `score_number`, что и домен, а оракул падает на неразрешимой ссылке
(`threshold_reference_unbound`). Значения пере-выводятся из замороженного probe и сверяются
с пином тестом `test_reference_pins_are_independently_pinned_and_recomputed_from_the_probe`;
значение каждой опубликованной строки берётся из reference, а не из замера строки
(`test_baseline_threshold_values_come_from_the_independently_pinned_reference`), где
равенство замера и floor записано как noninferiority-факт против floor, а не как источник
числа.

Привязка к носителю: `threshold_freeze.revision` в scorecard — sha256 файла схемы,
reader пересчитывает его на каждом прогоне. Правка схемы после заморозки (то есть
повторная подгонка порога) даёт `threshold_freeze_revision_mismatch`, а не тихий проход.
Семейство, которое корпус не измеряет (`threshold_pending`), порога не имеет вовсе: порог
на нём был бы ложным pass (D496), и домен отвергает такую запись
(`family_claimed_without_required_scope`).

`freeze_revision` в схеме — это sha256 замороженного probe (та же ревизия, что и у
evidence baseline-строк): контракт привязывает числа к замороженному прогону, а ревизия
самого носителя — к scorecard, потому что файл не может содержать собственный хэш.
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
| объявлена чужая ревизия заморозки | `threshold_freeze_revision_mismatch` |
| ссылка порога не резолвится | `threshold_reference_unbound` |
| строка публикует порог вне замороженного контракта | `threshold_missing_for_required_metric` |
| измеренная ось замолчала | `coverage_regression` |
| экономия без парного CPU | `savings_without_paired_cpu` |
| авторизация подняла охват | `authorization_used_as_product_evidence` |
| документ выброшен из продокументной проекции census | `census_document_dropped` |
| диспозиция census переставлена против выведенной | `census_projection_mismatch` |
| причина исключения census вне закрытого словаря | `census_disposition_untyped` |
| документ переведён через границу отказа без привязки набора | `refused_set_unbound` |

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
слое, — провал, а не тихое согласие. S02/T03 добавила в adapter-слой четыре метки census
(`census_projection_mismatch`, `census_document_dropped`, `census_disposition_untyped`,
`refused_set_unbound`), поэтому `ADAPTER_LABELS` и `violations` обязаны расти вместе.

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

### 10.4 Заморозка порогов S02/T02 (после правки §1, §3.1, §6, §11)

Прогоны ниже — на том же дереве, что и §10.3, плюс правки S02/T02. Rust-исходники этой
задачей не менялись, поэтому `cargo fmt` не запускался.

| # | Проверка | Команда | Exit | Верификация | Evidence |
|---|---|---|---|---|---|
| 24 | целевые сьюты S02/T02 (домен + oracle + baseline) | `pytest scripts/tests/test_absorption_progress.py scripts/tests/test_absorption_progress_governor.py scripts/tests/test_absorption_progress_baseline.py -q` | 0 | **94 passed** | `gsd_exec 6eef5b67` |
| 25 | governor YAML (новые метки, маркеры, fixtures) | `python3 -m governor --validate-yaml` | 0 | PASS (65 rules, 20 dimensions) | `gsd_exec e504fbb7` |
| 26 | preflight (реальное дерево) | `python3 -m governor --phase preflight` | 0 | DRIFT_RISK, 9 флагов, **0** `absorption_progress_*` | `gsd_exec e52f4def` |
| 27 | whitespace | `git diff --check` | 0 | clean | `gsd_exec ceff1c95` |

Что изменилось в измерительной системе (в редакции §10.5 формулировка уточнена):
пороги зафиксированы как prospective regression floor и выведены из pinned
reference-артефакта по `reference` (а не вписаны числом и не взяты из кандидата), freeze
привязан к sha256 схемы и пересчитывается
reader'ом, `scorecard.required` стал `true`, а мутационная матрица baseline-сьюта выросла
на три негатива (`threshold_freeze_revision_mismatch`, `threshold_reference_unbound`,
`threshold_missing_for_required_metric`) вместо снятого
`threshold_invented_before_freeze` — тот остаётся достижимым только под
`not_frozen`-контрактом и проверяется домен-сьютом.

### 10.5 Устранение самосравнения порогов (S02/T02, повторная проверка)

§10.4 описывает ревизию, в которой `reference` порога указывал на сам baseline scorecard:
порог резолвился из того же измеряемого артефакта, поэтому переписанный score кандидата
сдвигал собственный порог (self-comparison). Ниже — проверка после разделения floor и
кандидата; §10.4 сохранён как история и не переписывается.

| # | Проверка | Команда | Exit | Верификация | Evidence |
|---|---|---|---|---|---|
| 28 | целевые сьюты S02/T02 после разделения floor | `pytest scripts/tests/test_absorption_progress.py scripts/tests/test_absorption_progress_governor.py scripts/tests/test_absorption_progress_baseline.py -q` | 0 | **98 passed** | `gsd_exec 2a36526f` |
| 29 | governor YAML | `python3 -m governor --validate-yaml` | 0 | PASS | `gsd_exec 718fd346` |
| 30 | preflight (реальное дерево) | `python3 -m governor --phase preflight` | 0 | DRIFT_RISK, 9 флагов, **0** `absorption_progress_*` | `gsd_exec f403051a` |
| 31 | whitespace | `git diff --check` | 0 | clean | финальное действие T02 |

Что изменилось: floor выведен в отдельный артефакт
`artifacts/parse_compat/absorption_progress_reference_pins.json`, независимо запинованный
по sha256 в `threshold_policy.reference_pins`; `threshold_policy.source` указывает на него,
а домен отказывает порогу, чей `reference.path` не равен `reference_pins.path`
(`threshold_reference_not_independently_pinned`). Негативный тест
`test_threshold_reference_does_not_follow_the_candidate_scorecard` переписывает score
кандидата согласованно (evidence-digest по-прежнему совпадает с неизменными байтами
probe) и показывает, что резолвнутый порог не следует за кандидатом; тест
`test_reference_pins_are_independently_pinned_and_recomputed_from_the_probe` пере-выводит
все три значения из замороженного probe и сверяет их с пином. Формулировка «заморожено
до скоринга» снята: корпус измерен в S01, заморозка — prospective floor для последующих
прогонов, не доказательство приёмки или независимости.

Число целевых тестов выросло с 94 до 98: разделение floor/кандидата
(`test_threshold_reference_cannot_point_back_at_the_candidate_scorecard`,
`test_freeze_scope_must_be_declared`), пере-вывод floor из probe
(`test_reference_pins_are_independently_pinned_and_recomputed_from_the_probe`), значение
строки из reference
(`test_baseline_threshold_values_come_from_the_independently_pinned_reference`) и
негативный probe самосравнения
(`test_threshold_reference_does_not_follow_the_candidate_scorecard`).

### 10.6 Продокументный census (S02/T03)

| # | Проверка | Команда | Exit | Верификация | Evidence |
|---|---|---|---|---|---|
| 32 | целевые сьюты S02/T03 (baseline + governor) | `pytest scripts/tests/test_absorption_progress_baseline.py scripts/tests/test_absorption_progress_governor.py -q` | 0 | **80 passed** | `gsd_exec 34907671` |
| 33 | домен-сьют T01 | `pytest scripts/tests/test_absorption_progress.py -q` | 0 | 33 passed | `gsd_exec d80e9d90` |
| 34 | governor check DATA (новые метки/маркеры/fixtures) | `python3 -m governor --validate-yaml` | 0 | PASS (65 rules, 20 dimensions) | `gsd_exec 9950efd1` |
| 35 | preflight (реальное дерево) | `python3 -m governor --phase preflight` | 0 | DRIFT_RISK, 9 флагов, **0** `absorption_progress_*` | `gsd_exec bf4bcfea` |
| 36 | whitespace | `git diff --check` | 0 | clean | `gsd_exec 4fb2409d` |

Что изменилось: baseline публикует `population_census` — продокументную проекцию
610 назначенных документов (assigned/executed/refused/excluded, явный набор отказа,
типизированные причины, привязка авторитета к probe+manifest, правило вывода и
`projection_sha256`). Reader (`recompute_population_census`) пересчитывает проекцию и
дайджест из этих двух замороженных источников и передаёт их в observations; оракул
сравнивает с объявленным и падает на `census_projection_mismatch`,
`census_document_dropped`, `census_disposition_untyped`, `refused_set_unbound`.
Baseline-сьют пере-выводит проекцию независимо от reader'а, проверяет стабильность
replay и добавляет четыре негатива (выброшенный документ, переставленная диспозиция,
не‑типизированная причина, перевод через границу отказа); fixtures получили позитивный
census-контроль и четыре негативных записи. Rust-исходники этой задачей не менялись,
поэтому `cargo fmt` не запускался.

## 11. Известные ограничения

1. `scorecard.required: true` с S02/T02: удаление governed baseline-артефакта — это
   находка (`absorption_progress_scorecard_not_measured`), а не тишина. До флипа
   отсутствие артефакта гасило чек.
2. Пороги зафиксированы (`threshold_policy.state: frozen`, owner S02/T02) как
   prospective regression floor и привязаны к ревизии схемы: правка схемы после заморозки
   инвалидирует freeze (`threshold_freeze_revision_mismatch`), а не тихо перенастраивает
   порог. Floor выведен из pinned reference-артефакта, не из кандидата: порог,
   ссылающийся на сам scorecard, отвергается доменом
   (`threshold_reference_not_independently_pinned`).
3. Пины и evidence-digest живут в том же репозитории, что и носители (см. §6, граница E3).
4. `lineage` в scorecard пуст (`{}`) намеренно: привязка идёт через `evidence.sha256` и
   `source_revision`, а не через отдельный lineage-блок.
5. `gate_run_history` в scorecard — это provenance **инструмента** (два прогона гейта,
   гипотеза и её статус), а не измерение продукта: ни одна продуктовая ось его не
   читает, контракт T01 его игнорирует. Он там, чтобы история обоих прогонов была
   машиночитаемой, а не только в прозе.
6. Остаточный зазор заморозки: governor проверяет **форму** опубликованного порога
   (id, direction, margin, числовое значение), привязку порога к контракту и то, что
   threshold не ссылается на кандидата
   (`threshold_reference_not_independently_pinned`), но не сверяет само значение с
   резолвнутым из reference — это делает baseline-тест
   (`test_baseline_threshold_values_come_from_the_independently_pinned_reference`). Ручная
   правка одного числа в артефакте не даст governor-флага; она даст падение теста.
7. Пороги покрывают только reference-измеримые оси (`preservation`, `extraction`).
   `java_independence`, `resources`, `reliability` остаются `not_measured` без порога:
   порог без измеримого знаменателя был бы выдуманным числом.
8. Продокументная проекция census (S02/T03) проверяется только при наличии блока
   `population_census`: scorecard, который его удалит, оставит продокументную половину
   непроверенной (агрегатный знаменатель при этом по-прежнему упадёт на domain-проверках
   `census_mismatch`/`denominator_shrink`, а байты обоих источников держит
   `historical_guard_pins`). Это заявленная граница, а не подразумеваемая: чтобы закрыть и
   её, потребуется отдельная метка «блок отсутствует», которой в закрытом словаре T03 нет.
