# Native visual/funding: четыре семейства (figure/table/formula/funding) без self oracle

Статус: **candidate/shadow-only; Java GROBID остаётся production authority**
Волна: M507 / S06 / T01–T08
Дата: 2026-09-16

## Вывод

`da-parse` нативно проходит четыре специальных семейства поверх уже
поставленного S05-ядра: **figure** и **table** декодируются замороженными
моделями `figure`/`table` по **fulltext-ABI S05** (27 колонок), **formula**
собирается как locator-join из меток fulltext-модели (отдельной CRF-модели у
GROBID нет), **funding** декодируется моделью `funding-acknowledgement` по
собственному 17-колоночному ABI с dummy-колонкой метки и поднимается в
entity-lift.

Это **shadow-кандидат**: `deployed: false`, `import_eligible: false`,
`auto_promote: forbidden`, `decision_point: 6`, rollback-цель
`java-grobid-http@0.9.0-crf`. Ни одна production-граница не изменена:
`crates/da-cli/src/source_cli.rs`, `crates/da-ports/src/parser.rs`,
`crates/da-parse/src/parser.rs` и `crates/da-parse/src/projector.rs` не
содержат ни одного атома этих полос (проверяется governor и Rust-зеркалом).

У каждого семейства — **независимый SUT-free оракул** на SHA-256-пинах;
Java-derived self oracle нет. Документ намеренно называет и то, что offline
**не** доказано (раздел «Офлайн-ограничения»).

## Компоненты полосы

| Модуль | Роль | Schema const |
|---|---|---|
| `crates/da-parse/src/native_visual_family_cluster.rs` | общий clusteror семейств (`I-`-правило) + общая caption-link полоса | — (константы: `NATIVE_VISUAL_FAMILY_NAME_PREFIX = "I-"`, `MAX_NATIVE_VISUAL_FAMILY_ROWS = 32768`) |
| `crates/da-parse/src/native_figure_sequence.rs` | второй проход модели `figure`: блок меток, окно реферируемых колонок, `FigureSpan` | `da-parse.native-figure-sequence/v1` |
| `crates/da-parse/src/native_table_sequence.rs` | второй проход модели `table`: Java-сплит, `validateTable`-revert, типизированный cell-инвентарь | `da-parse.native-table-sequence/v1` |
| `crates/da-parse/src/native_formula_join.rs` | locator-join уравнений без декодера | `da-parse.native-formula-join/v1` |
| `crates/da-parse/src/native_funding_features.rs` | рендерер 17-колоночного ABI `FeaturesVectorFunding.printVector` + dummy-метка | `da-parse.native-funding-features/v1` |
| `crates/da-parse/src/native_funding_sequence.rs` | декод funding и entity-lift | `da-parse.native-funding-sequence/v1` |

DATA семейств — `data/parse/native_visual_families.yaml`
(`schema_version: native-visual-families.v1`); права —
`data/parse/grobid_model_rights.yaml` (D358).

## Группировка: `TaggingTokenClusteror`, а не D359

figure/table/funding группируют метки правилом GROBID
`TaggingTokenClusteror.isBeginning()`: **любой токен с префиксом `I-` открывает
новый кластер**. Это НЕ правило D359 `base_label_runs`, которым пользуются
полосы header/bibliography/fulltext:

| Последовательность | `cluster_family_label_runs` (эти семейства) | D359 `base_label_runs` |
|---|---|---|
| `<figure>, <figure>` | **1** кластер | 1 run |
| `I-<figure>, <figure>` | **2** кластера | 1 run |
| `I-<figure>, I-<figure>` | **2** кластера | 1 run |
| `<figure>, I-<figure>` | **2** кластера | 1 run |

Общим с D359 остаётся только снятие префикса (`base_label`). Расхождение
закреплено тестами `native_visual_family_cluster_contract` и не «починено»:
поведение Java — источник истины, а не удобство вызывающего.

## ABI четырёх семейств

### figure / table: fulltext 27 колонок, реферируются 0..=25

Второй проход **не имеет собственного feature-builder'а**: строки — те же
27-колоночные строки S05 (`render_fulltext_features`), а мёртвый
`FeaturesVectorFigure.printVector` (32 колонки) не воспроизводится, потому что
в grobid-core он не вызывается.

Ловушка, которую полоса закрывает явно: модель ссылается на колонки только
через свои шаблоны `%x[offset,column]`, и объявленное окно
(`#rdr#85/25/0` = максимальный реферируемый индекс 25) **уже** ширины строки
(27). Загрузка `CrfModel` не несёт поля `#rdr#`, поэтому окно читается из
шаблонов самой модели; несоответствие арности, объявленного окна или
отсутствие референса — типизированный отказ, а не тихое расширение входа.
`template_excludes: [26]` фиксирует: колонка 26 (superscript) шаблонами
figure/table не упоминается.

| Семейство | checkpoint | `#rdr#` | меток | Колонки |
|---|---|---|---|---|
| `figure` | `#mdl#2#521` | `#rdr#85/25/0` | 7 | 27 эмитируемых, 0..=25 реферируемых |
| `table` | `#mdl#2#1391` | `#rdr#85/25/0` | 10 | то же |

Table добавляет к figure метки `<note>`/`I-<other>` и Java-контракт
`TableParser`: один кластер может разрезаться на несколько таблиц (повторный
`<figure_head>` при открытом заголовке и теле), а невалидная таблица получает
`reverted: label_not_integer | head_not_tab_prefixed | empty_content`, а не
исчезает молча.

### formula: модели нет — locator-join

GROBID 0.9.0 не содержит модели `formula` (`models/` не имеет каталога
`formula`, `GrobidModels.FORMULA` отсутствует). Уравнения — это метки
fulltext-модели, поэтому семейство реализовано как join **по уже
декодированной** последовательности (`join_native_equations`), без
`CrfModel`-входа:

- `<equation>` открывает уравнение;
- `<equation_label>` приклеивается к открытому уравнению (или открывает
  label-only span, если уравнение не открыто — сирота сохраняется, не
  теряется);
- `<equation_marker>` — маркер, а не уравнение;
- любая другая базовая метка закрывает уравнение.

Нет equation-баз во всём декоде ⇒ типизированный `NotObserved`, а не «ноль
формул». Границы — кластеры (`<equation>` всегда открывает новый), не runs.

### funding: 17 колонок + dummy-метка `0`

funding — единственное семейство со **своим** feature-builder'ом
(`#rdr#100/16/0`): 17 печатаемых колонок (индексы 0..=16) в порядке
`FeaturesVectorFunding.printVector` плюс слот метки по индексу **17**, куда на
инференсе печатается литерал **`0`** (18-полевая строка).

| # | Колонка |
|---|---|
| 0 | текст токена |
| 1 | lowercased |
| 2–5 | prefix(1..4) |
| 6–9 | suffix(1..4) |
| 10 | `lineStatus` (`LINESTART`/`LINEIN`/`LINEEND`) |
| 11 | capitalisation (`INITCAP`/`ALLCAP`/`NOCAPS`; `ALLDIGIT` ⇒ `NOCAPS`) |
| 12 | digit (`ALLDIGIT`/`CONTAINSDIGITS`/`NODIGIT`) |
| 13 | singleChar (`1`/`0`) |
| 14 | knownFunder (`1`/`0`) |
| 15 | knownInfrastructure (`1`/`0`) |
| 16 | punctType |
| 17 | метка — **всегда литерал `0`** |

Рендерер **не принимает метку на вход** (`FundingFeatureToken` не имеет
label-поля), поэтому training-форма (`features.label = tag`) этим ABI
невоспроизводима by construction: константа
`NATIVE_FUNDING_LABEL_COLUMN_DUMMY = "0"` — единственное, что может быть
напечатано. Декодер, получив строку с не-`0` в слоте 17, отказывает
(`training shape unreachable`).

Словарь — 10 базовых меток в обоих спеллингах (20 меток):
`<funderName>`, `<grantName>`, `<grantNumber>`, `<programName>`,
`<projectName>`, `<person>`, `<affiliation>`, `<institution>`,
`<infrastructure>`, `<other>`. Объявленные Java-enum'ом и отсутствующие в
замороженной модели метки (`<funderAbbrv>`, `<programAbbrv>`,
`<projectAbbrv>`, `<url>`) не имеют веток и дают типизированный отказ
`UnmodeledJavaLabel`, а не молчаливый маппинг.

### Gazetteer-полосы funding (D363)

`knownFunder`/`knownInfrastructure` питаются из
`lexicon/organisations/funders.txt` (~3.3 MB) и
`lexicon/organisations/research_infrastructures.txt`, которые **D358 запрещает
бандлить**. Поэтому покрытие — caller-supplied типизированные **инклюзивные**
span'ы (`start <= n <= end`), а не выдуманные `0`/`1`:

- многословное окно `[2, 4]` даёт флаг ровно токенам 2..=4;
- инвертированный span (`start > end`) и выход за длину — типизированный
  отказ, не тихая потеря покрытия (fail-open исключён);
- **пустое покрытие валидно** и означает `offline_limit` (оба флага `0`), но
  никогда не подаётся как «наблюдённый негатив»: в DATA это зафиксировано как
  `gazetteer_coverage_source: caller_supplied_typed_spans`, а реальные
  лексиконы читает только opt-in harness.

## Caption-link полоса и граница S04

`postProcessFigureCaptions`/`postProcessTableCaptions` представлены общей
типизированной полосой `NativeCaptionLinks { markers, absence }`:

- инвентарь маркеров передаёт **вызывающий** (`inventory_source: caller`);
- маркеры — digest-only (`digest` = hex-64, дубликаты и битые digest'ы
  отклоняются типизированно), порядок сохраняется;
- **пустой инвентарь всегда даёт `absence: Some(NotObserved)`** и никогда
  «ноль ссылок» как успех;
- разрешение marker → reference в S06 **не реализуется**: владелец —
  поставленный S04 `native_callout_matching`
  (`caption_link_resolve_owner: native_callout_matching`). Граница названа
  явно, а не замолчана.

## Typed absence ≠ ноль (H12-N1)

| Полоса | Отсутствие | Ноль |
|---|---|---|
| фигуры/таблицы | `NativeFigureDecodeOutcome::NotObserved` / `NativeTableDecodeOutcome::NotObserved` | — (пустой `Decoded` не выдаётся за отсутствие) |
| ячейки таблицы | `NativeTableCells::NotObserved { reason }` | `Provided { cells }`; пустой инвентарь **запрещён** |
| caption-links | `absence: Some(NotObserved)` | — |
| funding | `NativeFundingDecodeOutcome::NotObserved` | — |
| формулы | `NativeFormulaJoinOutcome::NotObserved` | `Joined { equations: [] }` недостижим |

Правило: отсутствие наблюдения — типизированный факт, а не ноль. Негативы на
это (включая `NotObserved`-полярность) входят в каждый пер-семейный контракт.

## Регистрация: два ключа карантина, Rust-зеркало и SHA-пины

Новые полосы запрещены в четырёх production-границах **двумя отдельными
DATA-ключами** (D362; не слиты с `visual_heads_quarantine`, у которого нет
lock-полей и только слабый CamelCase-токен `NativeVisual`):

| Ключ `data/parse/grobid_compatibility.yaml` | Токенов | Границ | ABI-метаданные |
|---|---|---|---|
| `defaults.visual_family_sequence_quarantine` | 38 | 4 | `emitted_columns: 27`, `referenced_columns: 26` |
| `defaults.funding_sequence_quarantine` | 25 | 4 | `emitted_columns: 17`, `referenced_columns: 17`, `label_column_index: 17`, `label_column_value: "0"` |

Оба ключа несут одинаковые локки: `production_authority: java_http`,
`observe_mode: shadow_receipt`, `production_route: forbidden`,
`graph_write: forbidden`, `import_eligible: false`,
`auto_promote: forbidden`, `deployed: false`, `decision_point: 6`,
`independently_selectable: [header, references]`, плюс
`data_binding`, `offline_limits`, `independent_oracle`, `real_model_harness`.

Границы (обе полосы): `crates/da-cli/src/source_cli.rs`,
`crates/da-ports/src/parser.rs`, `crates/da-parse/src/parser.rs`,
`crates/da-parse/src/projector.rs`.

Тройная фиксация одного и того же множества — намеренная:

1. **DATA** (`grobid_compatibility.yaml`) — единственный источник истины;
2. **Governor** — `scripts/governor/adapters/checks/scholarly_parse_plane.py`
   (`_visual_family_sequence_quarantine_*` / `_funding_sequence_quarantine_*`,
   подключены в `_check_scholarly_parse_plane`) → флаги
   `policy_stamp` с evidence `…#visual_family_sequence_quarantine.<token>` /
   `…#funding_sequence_quarantine.<token>`;
3. **Rust-зеркало** — `crates/da-parse/tests/architecture_contract.rs`
   (`VISUAL_FAMILY_SEQUENCE_QUARANTINE_TOKENS` /
   `FUNDING_SEQUENCE_QUARANTINE_TOKENS`, форма S05
   `fulltext_sequence_quarantine_is_data_bound_and_lock_closed`): 1:1-равенство
   с DATA, детект синтетического атома, пустые production-границы,
   disjointness со всеми sibling-ключами и проверка, что policy-FSM по-прежнему
   содержит ровно шесть DecisionPoint.

Пара pytest+Rust ловит снятый DATA-ключ: чекер fail-closed молчит без DATA,
поэтому исчезновение ключа видно именно в равенствах DATA↔Rust↔pytest, а не
принимается тихо.

SHA-256-пины (D358/ADR-127) перевязаны в
`scripts/governor/checks/scholarly_parse_plane.yaml` (обе копии
`grobid_compatibility.yaml`: `rest_batch_asset_quarantine.source_bindings` и
`bindings.source_data`; плюс `native_fulltext_composition.yaml` и новый
`native_visual_families.yaml` в `bindings.source_data`) и в
`governor/fixtures/scholarly_parse_plane/contract.positive.json`.

`funding` добавлен аддитивно в `strata.lanes` в
`data/parse/native_fulltext_composition.yaml` (R125 санкционирует superset):
`component_ids` остаётся ровно 7, `admission.current_matrix` и
`independently_selectable` не тронуты.

## Независимый оракул и opt-in сверка с реальными моделями

- `crates/da-parse/tests/native_visual_family_same_n.rs` + фикстура
  `crates/da-parse/tests/fixtures/native_visual_family_oracle.yaml`:
  SUT-free expectation builder между маркерами S05, SHA-256-пины на документ,
  синтетические in-file `.wapiti`-модели, тест «в регионе builder'а нет
  SUT-символов». SUT физически не может произвести значения, с которыми
  сравнивается, — этим и выполняется запрет на Java-derived self oracle.
- `crates/da-adapters/tests/native_visual_family_real_model.rs` (opt-in,
  `DA_GROBID_MODEL_ROOT`): без env-переменной — `MISSING`, а не pass; читает
  `#mdl#`/`#rdr#` и label-блок самого артефакта, заново хеширует метки и
  сравнивает с `observed_*` в DATA, поэтому запись в DATA не может быть
  само-сертифицирующейся. Модельные байты в tree не попадают (D358).

## Офлайн-ограничения

- Row/Cell/LinePart merge-геометрия (пороги X/Y, `mergeMulticolumnCells`,
  colspan) не воспроизводится: ячейки принимаются **готовым инвентарём** от
  вызывающего; для table это названный `offline_limit`.
- `postProcessFulltextFixInvalidTableOrFigure` не воспроизводится (D366 —
  названо, а не замолчано).
- `inferAcronyms()` (funding) не воспроизводится: акронимы не выводятся.
- `UnmodeledJavaLabel` — типизированный отказ, а не маппинг.
- Source-bound parity по весам не заявляется и структурно невозможна (D358):
  parity — по формату/словарю/ширине, а не по байтам модели.
- Реальные label-последовательности — структурные инварианты, не записанные
  goldens.
- Лексиконы и модельные байты никогда не бандлятся
  (`bundled_model_bytes: false`), читаются только opt-in harness'ом.
- Caption marker → reference resolution — S04, не S06.

## Failure signals (что означает красный тест)

| Красный | Что произошло |
|---|---|
| `native_visual_family_cluster_contract` | уехало `I-`-правило (граница кластера) |
| `native_figure_sequence_contract` / `native_table_sequence_contract` | арность / declared-column проверка выродилась в панику или усечение |
| `native_formula_join_contract` | join перестал закрывать уравнение на посторонней метке |
| `native_funding_features_contract` | сломан порядок 17 колонок или dummy-метка перестала быть `0` |
| `native_funding_sequence_contract` | словарь разошёлся с моделью (`ModelLabelMismatch`, fail-closed) или потерян `NotObserved` |
| `native_visual_family_same_n` | дрейф SHA-256 фикстур или расхождение независимого знаменателя |
| `architecture_contract` | атом полосы просочился в одну из четырёх production-границ, либо DATA↔Rust разошлись, либо появилась седьмая DecisionPoint |
| governor `policy_stamp` с `…#…_sequence_quarantine.<token>` | то же просачивание, но на стороне траектории |
| `data_contracts d358` | модельные байты попали в tree |

Каждый отказ называется своим типом, а не строкой: `NotObserved`-негатив
красный ⇒ потерян H12-N1 (отсутствие снова выдаётся за ноль, включая cells и
caption-links); `ModelLabelMismatch` ⇒ словарь разошёлся с реальной моделью.

## Команды

```bash
# Пер-семейные контракты (новые)
cargo test -p da-parse --test native_visual_family_cluster_contract
cargo test -p da-parse --test native_visual_families_data_contract
cargo test -p da-parse --test native_figure_sequence_contract
cargo test -p da-parse --test native_table_sequence_contract
cargo test -p da-parse --test native_formula_join_contract
cargo test -p da-parse --test native_funding_features_contract
cargo test -p da-parse --test native_funding_sequence_contract
cargo test -p da-parse --test native_visual_family_same_n

# Регистрация + регрессия существующего
cargo test -p da-parse --test architecture_contract
cargo test -p da-parse --test data_contracts
cargo test -p da-parse --test native_fulltext_composition_data_contract
cargo test -p da-parse --lib native_visual

# Opt-in сверка с реальными моделями (вне CI; без env — MISSING, не pass)
DA_GROBID_MODEL_ROOT=/path/to/grobid-home/models \
  cargo test -p da-adapters --test native_visual_family_real_model

# Governance
PYTHONPATH=scripts python3 -m pytest scripts/tests/test_scholarly_parse_governance.py -q
PYTHONPATH=scripts python3 -m governor --validate-yaml
PYTHONPATH=scripts python3 -m governor --phase preflight
```

Все проверки локальны и воспроизводимы: per-crate, без сети, Java, GROBID и
docker. `DA_GROBID_MODEL_ROOT` задаётся **только** для opt-in прогона.

## Связанные документы

- `doc/parse/native-fulltext.md` — S05-ядро, чей ABI переиспользуют figure/table.
- `doc/parse/bibliography-abi.md` — D359 `base_label_runs` (правило, которое
  здесь **не** применяется).
- `doc/parse/scalar-crf.md` — seam `da-ports::scalar_crf` (`CrfModel`,
  `map_input`, `viterbi_decode`, `MAX_CRF_*`).
- `doc/parse/grobid-model-rights.md` — D358, права на модели/лексиконы.
