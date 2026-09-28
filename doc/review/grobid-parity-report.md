# Паритет Rust против Java на объявленной популяции — человекочитаемый отчёт

**Дата:** 2026-09-28
**Срез:** M513-r7dg4z / S02 (задачи T01–T04)
**Claim scope:** `local_proxy_only`
**Статус:** измерение предъявлено; это **не** разрешение на релиз и **не** нативный PDF-паритет

## Зачем этот документ

Числа паритета Rust против Java живут в машинном артефакте на ~4.6 МБ
(`artifacts/parse_compat/external_holdout_release_probe.json`). Рецензент не может
прочитать 4.6 МБ и не должен доверять пересказу. Этот отчёт:

- несёт популяцию и **привязку к digest**, а не к вербальной метке;
- объявляет **область замера** (что именно измерено и чем);
- называет **79 исключений** с типизированными причинами;
- предъявляет **per-class** и **агрегатный** Java-относительный precision/recall/F1;
- фиксирует правило **source-consumed** классов и **границу маршрута**;
- содержит машиночитаемые блоки, которые контрактный тест
  `crates/da-parse/tests/parity_report_contract.rs` сверяет с артефактом **точно**,
  так что отчёт не может молча разойтись с числами артефакта.

Артефакт остаётся единственным источником чисел; отчёт — его проекция для человека.
Машиночитаемый дубль чисел отчёта (блоки ниже) существует только чтобы дрейф был
отвергнут тестом, а не замечен на глаз.

## 1. Привязка к digest (identity популяции и релиз)

Замеры привязаны к **содержимому** (digest), а не к метке релиза.

| Понятие | Значение | Где в артефакте |
|---|---|---|
| identity популяции | `canonical_dataset_id_plus_identity` | `reference_population.identity` |
| **identity_digest** | `sha256:eee622ae57286faff567b1793f31ee54c5df3118e544a4bb8edf1c7376955c6d` | `reference_population.identity_sha256` |
| декларация digest | `identity_digest_is_byte_hash: false` (identity, не байт-хеш артефакта) | `reference_population.identity_digest_is_byte_hash` |
| source manifest | `data/parse/holdout_manifests.yaml` | `reference_population.source_manifest` |
| source manifest sha256 | `sha256:ef91d3c25c7d91dda2654471140fd41fcae8e1ed87e7094ad24c31fef2b64a39` | `reference_population.source_manifest_sha256` |
| **declared_release** | `java-grobid-http@0.9.1` (version `0.9.1`) | `reference_population.reference_release.label` / `.version` |
| **declared_release.image_digest** | `sha256:223957791ac2bbe48609dcc58a689b16b60baeae13a8734ef440ae6bfb38f4cd` | `reference_population.reference_release.image_digest` |
| release pin | `.docker/grobid-release.override.yml` | `reference_population.reference_release.release_pin` |
| captured at revision | `git:52bb08a4a016e69e3224ae79f02c2f3e53897e72` | `reference_population.reference_release.captured_at_revision` |
| execution route | `java-grobid-http/0.9.1/processFulltextDocument` | `executed_population.execution_route` |
| profile/consolidation | `structural`, `consolidateCitations=0,consolidateHeader=0,teiCoordinates=figure,table,note,formula` | `executed_population.execution_route_profile`, `reference_population.reference_release.consolidation` |

Плюс digest самих носителей:

| Носитель | sha256 |
|---|---|
| `artifacts/parse_compat/external_holdout_release_probe.json` | `a937da95ef987dffdcde46b998e98549fe247999e50986e1b0627a6f485d8275` |
| `crates/da-parse/tests/fixtures/holdout/frozen_tables.json` | `d25a446a2aa1aec8fce09b988c812b95b965e98929147713cd90880fdb734ba2` |

`frozen_tables.json` несёт внутренний `artifact_sha256 = a937da95…`, то есть таблицы
провозглашают проекцией именно этих байт артефакта.

## 2. Объявленная область замера (scope)

- **claim_scope: `local_proxy_only`.** Это означает: Rust-сторона — это композиция
  fulltext поверх **admitted band**, выведенного из замороженного **Java-эталона**.
  Это **не** нативный разбор PDF: native PDF→TEI маршрут в этом замере не измерен
  (см. §10).
- **Продюсер чисел** — компактный невалидирующий скан раннера
  (`da-cli compact non-validating scan over the composed emission`).
- **Судья (authority) — независимый оракул**: dev-only `da-parse`,
  `roxmltree` (`crates/da-parse/tests/external_holdout_probe_semantics.rs`). Судья
  пере-выводит числа из emission, а не читает их у продюсера.
- **Популяция**: 610 объявленных документов = 531 измеримый + 79 типизированных
  исключений. 531 документ — это **счёт по объявленной популяции, не статистическая
  выборка**; каждое число — счёт, pooled-заявления запрещены.
- Reference band не снят против rollback-релиза: привязка идёт к digest
  объявленного релиза 0.9.1.

## 3. Популяция и 79 исключений

`reference_population`: `documents = 610`, `measurable_documents = 531`,
`excluded_documents = 79`. Исключения перечислены поимённо (не «молча выброшены»)
в `reference_population.exceptions[]`, у каждой записи есть `document_id`,
`identity_sha256`, `size_bytes` и **типизированная причина**.

| Причина | Документов | Почему исключён |
|---|---|---|
| `header_date_absent` | 72 | в замороженном TEI нет элемента даты; фатально и для проб, и для нативного header-композера |
| `header_title_absent` | 5 | нет элемента заголовка |
| `header_author_absent` | 2 | нет элемента автора |
| **Итого** | **79** | |

Исключённый документ — не «нулевая строка»: он назван причиной и не попадает ни в
числитель, ни в знаменатель. `evaluated_population.no_denominator_documents = 0`:
среди 531 измеримых нет документа с пустой знаменательной популяцией.

## 4. Per-class Java-относительный паритет

Знаменатель — **`source_population`** (все вхождения класса в замороженный
Java-эталон), а не admitted band. `missing = source_population - conserved`,
`extra = fabricated`. `recall_java = conserved / source_population`,
`precision = conserved / (conserved + fabricated)`, `f1` — гармоническое среднее.

| Класс | source_population | admitted | conserved | missing | extra | recall_java | precision | f1 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `AdmittedAuthorLine` | 7727 | 7569 | 7569 | 158 | 0 | 0.9796 | 1.0000 | 0.9897 |
| `SemanticFieldLoss::HeaderTitle` | 1062 | 531 | 531 | 531 | 0 | 0.5000 | 1.0000 | 0.6667 |
| `SemanticFieldLoss::HeaderEmail` | 908 | 908 | 908 | 0 | 0 | 1.0000 | 1.0000 | 1.0000 |
| `SemanticFieldLoss::HeaderIds` | 571 | 571 | 571 | 0 | 0 | 1.0000 | 1.0000 | 1.0000 |
| `SemanticFieldLoss::AuthorAffiliation` | 7077 | 3055 | 3055 | 4022 | 0 | 0.4317 | 1.0000 | 0.6030 |
| `source_listbibl_direct_children` | 31368 | 31368 | 31368 | 0 | 0 | 1.0000 | 1.0000 | 1.0000 |
| `CitationFieldRole::doi` | 1922 | 1922 | 1922 | 0 | 0 | 1.0000 | 1.0000 | 1.0000 |
| `CitationFieldRole::arxiv_id` | 10557 | 10557 | 10557 | 0 | 0 | 1.0000 | 1.0000 | 1.0000 |
| `CitationFieldRole::url` | 5868 | 5868 | 5868 | 0 | 0 | 1.0000 | 1.0000 | 1.0000 |
| `CitationFieldRole::article_title` | 12561 | 12561 | 12561 | 0 | 0 | 1.0000 | 1.0000 | 1.0000 |
| **Итого** | **79621** | **74910** | **74910** | **4711** | **0** | — | — | — |

Словарь классов и их порядок читаются по ссылке из
`data/parse/comparison_contract.yaml#native_fulltext_composition_shadow.corpus_semantic_regression.class_order`
(10 классов), а не копируются в отчёт как второй источник.

### Типизированные сужения, а не молчаливые потери

Два класса имеют низкий `recall_java` не потому, что носители «потерялись» молча,
а потому что нативные парсеры **типизированно отвергли** конкретный носитель:

- **`AdmittedAuthorLine` — recall_java 0.9796** (missing 158). Отвергнутый
  `persName` несёт типизированную причину: `ambiguous:single_token` (81 запись),
  остальные — `malformed:*`/`empty`. Это записанные типизированные отказы
  (`outcomes[].typed_gaps[]`), а не тихая потеря.
- **`SemanticFieldLoss::AuthorAffiliation` — recall_java 0.4317** (missing 4022).
  Доминирующая причина — `unsupported:no_recognizable_component` (3953 записи),
  плюс `empty`/`oversize`. Нативный `NativeAffiliationParser` отвергает
  непознаваемый ключевой носитель аффилиации по имени причины.

Оба сужения видны как столбец `missing` и как `typed_gaps` по каждому документу —
рецензент видит и число, и причину.

## 5. Агрегат (aggregation `micro_occurrence`)

Агрегат — микро-счёт по вхождениям, не среднее по документам. Он **не** читается из
`admitted`-относительной метрики: знаменатель — `source_population = 79621`.

| Величина | Микро | Десятично |
|---|---:|---:|
| denominator | 79621 | — |
| tp (conserved) | 74910 | — |
| fn (missing) | 4711 | — |
| fp (fabricated) | 0 | — |
| precision | 1000000 | 1.0000 (fp = 0) |
| native_capability_recall | 945771 | 0.9458 |
| covered_union_recall | 947501 | 0.9475 |
| f1 | 972130 | ~0.9721 |

- **native_capability_recall** считает только те классы, которые нативный композер
  реально производит (source-consumed классы исключены из числителя и знаменателя).
- **covered_union_recall** — верхняя граница: source-consumed классы считаются
  покрытыми в объёме их полной source-популяции (см. §6).
- **f1** сводит precision с `native_capability_recall`, а `covered_union_recall`
  называется отдельно как верхняя граница.

### Чем coverage-число отличается от admission-relative 1.000000

`evaluated_population.aggregate` — это **другая** метрика с другим знаменателем:
`document_agreement_ratio` = согласие документов, знаменатель — `admitted`
(`tp 74910`, `fn 0`, `fp 0`, `agreeing_documents 531`, `margin_pass 1062`,
`margin_fail 0`, `document_agreement_ratio = 1.000000`). Её recall равен 1.0 **по
построению**: admitted — это и есть conserved. Она **не** является Java-относительным
recall и не должна цитироваться как «паритет 1.0».

Java-относительные числа ниже единицы именно потому, что знаменатель — полная
`source_population`, включая вхождения, которые admitted band не производит.
Оба чтения сосуществуют в артефакте: admission-relative в
`evaluated_population.aggregate`, Java-relative в
`evaluated_population.class_parity.aggregate` (и в `class_parity_declaration`).
Ни одна из них не является разрешением на релиз.

## 6. Правило source-consumed классов

`class_parity_declaration.source_consumed_classes` = ровно три класса:

- `SemanticFieldLoss::HeaderTitle`
- `SemanticFieldLoss::HeaderEmail`
- `SemanticFieldLoss::HeaderIds`

Их носители попадают в emission **через скопированное source-поддерево**, а не через
admitted band. Поэтому они считаются **покрытыми, а не потерянными**:
`zero_zero_denominator` интерпретируется как covered-union-конвенция (source-consumed
класс покрыт своей полной source-популяцией), а не как вакуумный per-class 1.0.

Следствие: **`recall_java = 0.5000` у `HeaderTitle` — не дефект.** Второй носитель
заголовка (531 из 1062) едет в копированном source-поддереве; называть его FN
означало бы сфабриковать 531 ложный промах. То же для `HeaderEmail`/`HeaderIds`
(их `recall_java` равен 1.0, потому что admitted покрывает весь носитель).

Правило объявлено в DATA и читается продюсером/оракулом оттуда, а не хардкодится.

## 7. Per-document профиль (совпавшие / отсутствующие / лишние)

Per-document классы совпавших (`conserved`), отсутствующих
(`source_population - conserved`) и лишних (`fabricated`) лежат в артефакте:

- `class_profiles[]` — по каждому из 531 документа массив из 10 классов с полями
  `source_population`, `admitted`, `conserved`, `fabricated`, `lost`,
  `recall_java_micros`, `precision_micros`, `f1_micros`, `source_consumed`;
- `outcomes[]` — по каждому документу агрегатные счёты и `typed_gaps[]` с причинами.

Сводка: **531/531** документов в согласии с объявленным допуском
(`agreeing_documents = 531`), **1062/1062** margin-вердиктов PASS,
`margin_fail = 0`, `no_denominator_documents = 0`.

`crates/da-parse/tests/fixtures/holdout/frozen_tables.json` — та же проекция в
коммитнутых таблицах (2.3 МБ), с `artifact_sha256`, указывающим на артефакт.

## 8. H18: какие `CitationFieldRole` реально измерены

У `CitationFieldRole` **13 вариантов**, но реальный носитель в корпусе есть только у
четырёх:

- измерены: `CitationFieldRole::doi`, `CitationFieldRole::arxiv_id`,
  `CitationFieldRole::url`, `CitationFieldRole::article_title`;
- **не измерены и названы** (нет реального носителя в объявленном корпусе):
  `authors`, `publication_date`, `container_title`, `volume`, `issue`, `pages`,
  `article_number`, `publisher`, `place`.

Это открытый пункт H18: отчёт **не** подразумевает покрытие девяти незмеренных
ролей. Их отсутствие — свойство объявленного корпуса, а не заявка о паритете.

## 9. H21: расхождение метки и привязка к digest

Метка объявленного релиза — `java-grobid-http@0.9.1`
(`reference_release.label`/`.version`), digest —
`sha256:223957791ac2bbe48609dcc58a689b16b60baeae13a8734ef440ae6bfb38f4cd`.
Цель отката — `java-grobid-http@0.9.0-crf` (`rollback_target`, `rollback_version`
`0.9.0`). Расхождение помечено как `label_divergence_item: H21`.

Все замеры этого отчёта привязаны к **digest**, а не к метке, поэтому
расхождение меток не меняет числа. H21 остаётся открытым и является **входом для
S03** (ledger паритета), а не чем-то, что этот замер уже закрыл.

## 10. Типизированная именованная граница маршрута: нативный PDF→TEI не измерен

Прямо и в явном виде: **нативный PDF→TEI маршрут этим замером не измерен.**

- нет раннера, соединяющего PDF-вход с `compose_native_fulltext`;
- PDF-коридор проверен только на **3 документах** (`corpus_n = 3`), и это не
  популяция паритета;
- значит, этот паритет — `local_proxy_only` над admitted band, **не** заявка о
  нативном PDF-разборе.

Граница записана в `named_limitations` артефакта как типизированная. Она остаётся
**решением человека** (открыть ли PDF→TEI маршрут как отдельный замер), а не
выданным за сделанное паритетом. Никакой автоматический вывод «нативный маршрут
достигнут» из этого отчёта не следует.

## 11. Указатели

| Что | Где |
|---|---|
| Машинный артефакт (числа, per-document профиль) | `artifacts/parse_compat/external_holdout_release_probe.json` |
| Коммитнутые таблицы той же проекции | `crates/da-parse/tests/fixtures/holdout/frozen_tables.json` |
| Независимый оракул (authority) | `crates/da-parse/tests/external_holdout_probe_semantics.rs` |
| Контракт этого отчёта | `crates/da-parse/tests/parity_report_contract.rs` |
| Словарь классов/refusal-ов | `data/parse/comparison_contract.yaml#native_fulltext_composition_shadow.corpus_semantic_regression.class_order` |

## 12. Ограничения (из `named_limitations` артефакта)

- 531 из 610 измерены; 79 исключены **по имени**, не молча.
- исключённые документы несут типизированную причину и не становятся нулевыми строками.
- 531 документ — не статистическая выборка; каждое число — счёт.
- продюсер не заявляет независимость от продакшн-маршрута: reference снят объявленным
  живым Java-релизом, admitted band — локальный прокси.
- latency и cost ограничены, но не измерены; неизвестное не считается нулём
  (`cost_lower_bound = null`, `cost_upper_bound = null`).
- замер не снимает `bytes_absent`-банду и не авторизует релиз: нужно именованное
  человеческое решение (`decision: pending`, `reviewer: null`).
- компактный скан — не структурный оракул; authority — dev-only оракул.

## 13. Машиночитаемые блоки (сверяются контрактным тестом)

Блок ниже — не второй источник истины, а якорь для теста: любое расхождение с
`evaluated_population.class_parity.aggregate` отвергается по имени.

<!-- parity-aggregate:begin -->
```json
{
  "denominator": 79621,
  "tp": 74910,
  "fn": 4711,
  "fp": 0,
  "precision_micros": 1000000,
  "native_capability_recall_micros": 945771,
  "covered_union_recall_micros": 947501,
  "f1_micros": 972130
}
```
<!-- parity-aggregate:end -->

Блок per-class — счёты 10 классов; тест пере-выводит их из `class_profiles`
артефакта и сверяет точно, а также проверяет, что их суммы равны агрегату выше
(`denominator`, `tp`, `fn`, `fp`).

<!-- parity-per-class:begin -->
```json
[
  {"class": "AdmittedAuthorLine", "source_population": 7727, "admitted": 7569, "conserved": 7569, "missing": 158, "fabricated": 0, "source_consumed": false},
  {"class": "SemanticFieldLoss::HeaderTitle", "source_population": 1062, "admitted": 531, "conserved": 531, "missing": 531, "fabricated": 0, "source_consumed": true},
  {"class": "SemanticFieldLoss::HeaderEmail", "source_population": 908, "admitted": 908, "conserved": 908, "missing": 0, "fabricated": 0, "source_consumed": true},
  {"class": "SemanticFieldLoss::HeaderIds", "source_population": 571, "admitted": 571, "conserved": 571, "missing": 0, "fabricated": 0, "source_consumed": true},
  {"class": "SemanticFieldLoss::AuthorAffiliation", "source_population": 7077, "admitted": 3055, "conserved": 3055, "missing": 4022, "fabricated": 0, "source_consumed": false},
  {"class": "source_listbibl_direct_children", "source_population": 31368, "admitted": 31368, "conserved": 31368, "missing": 0, "fabricated": 0, "source_consumed": false},
  {"class": "CitationFieldRole::doi", "source_population": 1922, "admitted": 1922, "conserved": 1922, "missing": 0, "fabricated": 0, "source_consumed": false},
  {"class": "CitationFieldRole::arxiv_id", "source_population": 10557, "admitted": 10557, "conserved": 10557, "missing": 0, "fabricated": 0, "source_consumed": false},
  {"class": "CitationFieldRole::url", "source_population": 5868, "admitted": 5868, "conserved": 5868, "missing": 0, "fabricated": 0, "source_consumed": false},
  {"class": "CitationFieldRole::article_title", "source_population": 12561, "admitted": 12561, "conserved": 12561, "missing": 0, "fabricated": 0, "source_consumed": false}
]
```
<!-- parity-per-class:end -->

## 14. Блокировки (не сняты этим отчётом)

`policy_locks` артефакта: `import_eligible: false`, `deployed: false`,
`auto_promote: forbidden`, `graph_write: forbidden`, `production_route: forbidden`,
`observation_mode: shadow`, `pooled_claim: forbidden`,
`fallback_target: java-grobid-http@0.9.0-crf`. Отчёт не меняет ни одну блокировку.
