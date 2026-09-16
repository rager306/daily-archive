# Native body/fulltext: 27-колоночный ABI, D359-группировка и межстраничный reading order

Статус: **candidate/shadow-only; Java GROBID остаётся production authority**
Волна: M507 / S05 / T01–T06
Дата: 2026-09-16

## Вывод

`da-parse` умеет нативно пройти полосу тела: собрать точные 27-колоночные
feature-строки тела, декодировать их через scalar-CRF замкнутого 12-базового
словаря, сгруппировать серии по **равенству базовых имён** (D359), склеить
разрозненные страницы в один документный reading order с типизированным
page-span для continued-run, и честно отказаться от предложенных
sentence/language-ресурсов, которых у GROBID в этой полосе нет.

Это **shadow-кандидат**: `deployed: false`, `import_eligible: false`,
`auto_promote: forbidden`, `decision_point: 6`, rollback-цель
`java-grobid-http@0.9.0-crf`. Никакой production route не изменён: тело не
сплайсится ни в `compose_native_fulltext`, ни в `parse_pdf`/`ParserPort`,
`NativeBodyAccountant` и `native_fulltext_blocks` не переписаны.

Документ намеренно называет и то, что offline **не** доказано (см. раздел
«Офлайн-ограничения»), а не только успехи.

## Компоненты полосы

| Модуль | Роль | Schema const |
|---|---|---|
| `crates/da-parse/src/native_fulltext_features.rs` | рендерер 27-колоночного ABI `FeaturesVectorFulltext.printVector` (чистый: без I/O, env, time, tracing, модельных байт) | `da-parse.native-fulltext-features/v1` |
| `crates/da-parse/src/native_fulltext_sequence.rs` | bounded runner: rows → `map_input` → `viterbi_decode` → серии по базовым именам | `da-parse.native-fulltext-sequence/v1` |
| `crates/da-parse/src/native_fulltext_reading_order.rs` | склейка страниц в документный порядок, `ColumnShift`, `CrossPageRun`, digest чтения | `da-parse.native-fulltext-reading-order/v1` |
| `crates/da-parse/src/native_fulltext_sentence.rs` | типизированные non-CRF шаги sentence/language + реестр версий поверхностей тела | `da-parse.native-fulltext-sentence/v1` |

Общий per-token факт-слой (`crates/da-parse/src/native_feature_facts.rs`) и
примитив группировки (`base_label_runs` в `native_bibliography_sequence.rs`)
**переиспользуются**, а не копируются: второй копии D359-правила и второго
набора фактов в полосе тела нет.

Домен-типы (CRF reader/decoder) приходят в `da-parse` **только через**
`da-ports`: прямой импорт `da-domain` запрещён машинно
(`crates/da-parse/tests/architecture_contract.rs`), поэтому seam зафиксирован
реэкспортом `crates/da-ports/src/scalar_crf.rs`.

## 27-колоночный ABI

Порядок колонок — ровно тот, что печатает Java `printVector`; ширина строки
фиксирована `NATIVE_FULLTEXT_FEATURE_COLUMNS = 27`.

| # | Java field | источник |
|---|---|---|
| 0 | `string` | текст токена |
| 1 | lowercased `string` | `String.toLowerCase` |
| 2–5 | `prefix(1..4)` | `TextUtilities.prefix` |
| 6–9 | `suffix(1..4)` | `TextUtilities.suffix` |
| 10 | `blockStatus` | producer |
| 11 | `lineStatus` | producer |
| 12 | `alignmentStatus` | producer (`ALIGNEDLEFT` / `LINEINDENT`) |
| 13 | `fontStatus` | stateful: смена **имени** шрифта |
| 14 | `fontSize` | stateful: знак усечённого изменения размера |
| 15 | `bold` | producer |
| 16 | `italic` | producer |
| 17 | `capitalisation` | лестница, `ALLDIGIT` печатается как `NOCAPS` |
| 18 | `digit` | лестница, `ALLDIGIT` переопределяет `CONTAINSDIGITS` |
| 19 | `singleChar` | длина == 1 |
| 20 | `punctType` | пунктуационная лестница |
| 21 | `relativeDocumentPosition` | `linearScaling(nn, fulltextLength, 12)` |
| 22 | `relativePagePosition` | `linearScaling(lineY, pageHeight, 12)`, клампится |
| 23 | `bitmapAround` | producer |
| 24 | `calloutType` | `UNKNOWN`, когда неизвестно |
| 25 | `calloutKnown` | producer |
| 26 | `superscript` | producer |

**Ловушка D363.** `printVector` содержит ещё четыре поля, чей append
закомментирован в Java, и одно присваиваемое-но-ненапечатанное. Они **не**
эмитятся: `vectorAround`, `spacingWithPreviousBlock` (`NBBINS_SPACE = 5`),
`characterDensity` (`NBBINS_DENSITY = 5`), training-label placeholder и
`relativePagePositionChar`. Замороженная модель обучена ровно на этих 27
полях, поэтому «полезное» расширение строки было бы тихой подменой ABI.

Две полосы унаследованы буквально и легко «чинятся» по ошибке: печатаемая
`capitalisation` равна `NOCAPS`, когда `digit == ALLDIGIT` (`2017` — `ALLCAP`
по лестнице и `NOCAPS` в печати); `fontSize` — не сравнение со средним, а знак
изменения против последнего токена.

**Шкалы.** `NBBINS_POSITION = 12` — масштаб тела, и это **не**
citation-именованный аксессор из `native_feature_facts` (S04/D361): у семейств
раздельные шкалы, изменение одной не должно молча двигать другую.

**Отказ вместо аппроксимации.** `NativeFulltextFeatureError` типизирует
`EmptyInput | EmptyToken | TokenContainsWhitespace | NonFiniteFont |
NonFinitePageGeometry | NegativePosition | TooManyRows | WrongArity |
DeclaredColumnMismatch`. Пустой токен или токен с разделителем отвергается
(иначе разъезжается арность колонок), а модель, объявившая не 27 колонок,
получает `DeclaredColumnMismatch` — расширять строку под модель запрещено.

## Группировка меток: D359, равенство базовых имён

Словарь закрыт и берётся из DATA-ledger `data/parse/native_body_accounting.yaml`
(`fulltext_labels`, 12 баз): `paragraph`, `item`, `section`, `citation_marker`,
`figure_marker`, `table_marker`, `equation_marker`, `figure`, `table`,
`equation`, `equation_label`, `other`. В модели метки записаны в обоих
спеллингах (`<x>` и `I-<x>`), всего 24 допустимых строки.

Серия — **максимальный блок одинаковых базовых имён после снятия `I-`**: смена
базы закрывает серию и открывает следующую. Это правило (`base_name_equality`)
общее с bibliography-полосой и живёт в одном месте (`base_label_runs`,
`pub(crate)`). Зеркальное BIO-правило (bare открывает, `I-` продолжает) было
отвергнуто в S03: реальные последовательности GROBID не BIO-корректны, поэтому
`["I-<paragraph>", "<paragraph>", "<paragraph>"]` и
`["<paragraph>", "I-<paragraph>"]` — это одна серия `<paragraph>`, а не две.

Блок меток модели проверяется **fail-closed до построчной работы**:
`ModelLabelCount` (неверная кардинальность) и `ModelLabelMismatch` (расхождение
same-size блока) — разные типизированные отказы. `UnknownLabel` остаётся позади
этой проверки как defense-in-depth.

## Межстраничный reading order и continued-run

GROBID склеивает body-части документа в одну последовательность токенов через
страницы (`FullTextParser.getBodyTextFeatured`), поэтому reading order —
ответственность слоя склейки, а не CRF.

- Документный порядок выводится из **объявленных layout-фактов** (`page`,
  `block`, `within_block`), а не из порядка входного списка: перемешивание входа
  чтение не меняет.
- `relativeDocumentPosition` считается по длине всего тела, а
  `relativePagePosition` — по высоте текущей страницы; сброс внутристраничной
  позиции происходит при смене номера страницы.
- Вертикальный сдвиг блока (смена колонки) — типизированный факт
  `ColumnShift`, а не вес spacing.
- `CrossPageRun` — серия из sequence-слоя, пересекающая границу страницы: она
  остаётся одной серией с типизированным page-span (`first_page`, `last_page`,
  `continued`). Продолжение параграфа/секции через страницы — следствие
  склейки, а не отдельная модель.
- `NativeFulltextReadingOrder.digest` — identity чтения: он считается по
  **упорядоченным** строкам и их layout-фактам, поэтому равный bag меток при
  другом порядке даёт другой digest. Перестановка страниц меняет identity.
- Потолки: `MAX_FULLTEXT_PAGES = 4096`, `MAX_FULLTEXT_TOKENS = 32768` (совпадает
  с `MAX_NATIVE_FULLTEXT_FEATURE_ROWS`, чтобы склейка не могла подать в декод
  больше строк, чем принимает раннер). Превышение — типизированный отказ
  (`TooManyTokens` / `TooManyPages`), а также `EmptyInput`,
  `DuplicateOrdinal`, `OrdinalOutOfRange`, `IncompleteBlock`.

## Sentence/language: типизированные non-CRF шаги

У моделей GROBID **нет** отдельных sentence/language CRF: sentence detection
живёт в TEI-форматтере (`SentenceUtilities.runSentenceDetection`), а
`LanguageUtilities` из `FullTextParser` почти не вызывается. Порт «как GROBID»
без ресурса был бы silent-аппроксимацией, поэтому шаги смоделированы как
**разрыв (named gap)**, а не как детектор:

- `NativeBodyStepOutcome` имеет ровно три исхода: `NotRequested`,
  `MissingResource`, `Unsupported`. **Success-варианта нет**, `is_productive()`
  всегда `false` (запинено тестом), поэтому PASS без ресурса невозможен.
- Пустой или пробельный resource id трактуется как отсутствующий ресурс
  (fail-closed), а не как настроенный.
- Sentence-шаг напрямую завязан на wire-переключатель `segmentSentences`
  (`FulltextDialectV1::segment_sentences`), language-шаг — на caller intent
  (языкового свитча в V1-диалекте нет).
- Будущий реальный producer обязан **добавить** вариант исхода и поднять
  `NATIVE_FULLTEXT_SENTENCE_SCHEMA_VERSION`, а не расширять существующий.

## Версионирование тела и смежные поверхности

`schema_version` четырёх модулей полосы экспонируются как поверхность версии
(`native_body_version_surface()`), а смежные поверхности зафиксированы как
данные (`NATIVE_BODY_ADJACENT_SURFACES`), а не как проза:

| Поверхность | Владелец | Смысл |
|---|---|---|
| `abstract` | `header-label` | abstract — метка header-полосы S03, не метка тела; в словаре тела её нет |
| `pdf_version_vs_tei_body` | `parse-envelope` | различие PDF-version и TEI-body принадлежит parse-envelope, а не CRF-семье |

## Независимая приёмка same-n и знаменатели

Java-derived self oracle запрещён (H11-N1), а byte conservation не считается
доказательством корректности. Поэтому приёмка построена как SHA-256 пины
сохранённых TEI-фикстур под `crates/da-parse/tests/fixtures/preserved/` плюс
**собственный лексический сканер** в тесте, который не вызывает ни
`decode_fulltext_sequence`, ни `render_fulltext_features`, ни другие SUT-функции
полосы. Сравнивается структура чтения и группировка, а не совпадение байтов.

Знаменатели (три фикстуры, один population):

| Величина | Значение |
|---|---|
| блоков тела | 2375 |
| токенов тела | 75369 |
| максимальных серий по базам | 1313 |
| блоков по семействам (`body_blocks` / `body_markers` / `visual_content`) | 1678 / 475 / 222 |
| токенов по семействам | 67025 / 1525 / 6819 |
| body-scoped `<ref type="bibr">` маркеров | 376 |
| anchor-население bibliography-oracle (пересказано, другое население) | 380 |
| digest канонической квитанции | `285a5ddd…069ec9` |

376 и 380 — **разные населения** и никогда не пулятся; 380 пересказано здесь
именно чтобы это было машинно-видно. 376/2375/75369/1313 — lexical receipts,
а не записанные Java-goldens. Серии группируются дискриминирующим
D359-правилом, а отвергнутое mirror-BIO правило оставлено в oracle как
негативный контроль: оно пересплитовывает тот же корпус, поэтому регрессия
наблюдаема, а не эквивалентна.

## Opt-in harness реальной модели

`crates/da-adapters/tests/native_fulltext_real_model.rs` читает локальный
ресурс-рут из `DA_GROBID_MODEL_ROOT` плюс **относительный** путь
`fulltext/model.wapiti`, лимит 128 MiB, только относительные пути, без
абсолютных дефолтов. Без env тест печатает `MISSING` и выходит — он никогда не
рапортует PASS без модели. При заданном корне проверяются **структурные
инварианты**: число колонок объявленного наблюдения совпадает с
`NATIVE_FULLTEXT_FEATURE_COLUMNS`, а словарь меток модели не расходится с
нативным набором баз; расхождение — паника с диагностикой, а не
проглатывание. Ни один модельный байт в tree не попадает (D358).

## Регистрация и карантин (governor)

`data/parse/grobid_compatibility.yaml` → `defaults.fulltext_sequence_quarantine`:
36 атомов полосы и **четыре** production-границы
(`crates/da-cli/src/source_cli.rs`, `crates/da-ports/src/parser.rs`,
`crates/da-parse/src/parser.rs`, `crates/da-parse/src/projector.rs`), замки
`production_authority: java_http`, `observe_mode: shadow_receipt`,
`production_route`/`graph_write: forbidden`, `import_eligible: false`,
`auto_promote: forbidden`, `deployed: false`, `decision_point: 6`,
`independently_selectable: [header, references]`, `proof_level: contract`.

**Смежность имён карантинов намеренна (D362).** Рядом живут
`defaults.fulltext_blocks_quarantine` (P2 region accounting),
`defaults.body_accounting_quarantine` (M496 P5 accountant) и этот
`fulltext_sequence_quarantine` (S05 body reading order). Это **три разные
полосы**, и первые две здесь не трогаются. То же разделение продублировано на
Rust-стороне: `FULLTEXT_SEQUENCE_QUARANTINE_TOKENS` — отдельный массив, он не
сливается с `S05_BODY_QUARANTINE_ATOMS` / `S05_BODY_ACCOUNTING_DATA_TOKENS`.

Governor-пара `_fulltext_sequence_quarantine_spec` /
`_fulltext_sequence_quarantine_tokens_present` и
`_check_fulltext_sequence_quarantine` читают **только DATA**: при отсутствующем
или испорченном ключе они молчат (эту поверхность владеет SHA/evidence_missing),
но запрещённое множество и границы никогда не выдумываются чекером.
snake_case-атомы матчатся как call-site (пробелы перед `(` разрешены),
CamelCase — как имя типа по границе слова; комментарии `//` вырезаются, проза и
префикс-подстроки не считаются.

Rust-зеркало (`crates/da-parse/tests/architecture_contract.rs`) держит те же 36
атомов и те же 4 границы, сверяет их 1:1 с DATA-ключом и проверяет, что замки
`deployed`/`import_eligible`/`auto_promote`/`production_route`/`graph_write`/
`decision_point`/`independently_selectable` не разъехались.

Аддитивные receipts: полоса зарегистрирована как **observation stratum**
(`strata.lanes` += `body`, R125 — body никогда не `independently_selectable`) и
как вложенная под-квитанция в `component_receipts` существующего компонента в
`data/parse/native_fulltext_composition.yaml` и
`data/parse/comparison_contract.yaml`. Новый `component_id` не заводится,
`independently_selectable` и `independently_selectable_lanes` остаются ровно
`[header, references]`, `decision_point` остаётся 6. Вложенность выбрана
намеренно: квитанция полосы тела лежит под уже существующим структурным
компонентом, а не выглядит новым компонентом; `producer`/`disposition`/
`evidence_state` родителя не изменены — вложение ничего не «принимает».

## Офлайн-ограничения (что не доказано)

- Нет сохранённого layout/ALTO или геометрии страниц, поэтому цепочка
  **PDF → строки тела** не доказана; same-n-рамка — это TEI → атомы тела.
- Sentence detection и language identification — **named gaps**, а не
  воспроизведённые детекторы; PASS без ресурса невозможен по конструкции.
- `postProcess` fulltext (Java `FullTextParser.postProcess`) не воспроизведён.
- `abstract` — метка header-полосы S03, а различие PDF-version / TEI-body —
  parse-envelope, не CRF-семья.
- Знаменатели 376, 380, 1313, 1678/475/222, 67025/1525/6819 — lexical
  receipts независимого сканера, а не записанные Java-goldens.
- Наблюдения реальной модели — структурные инварианты (ширина наблюдения,
  словарь меток), а не записанные последовательности-эталоны.
- Лексикон-бандл инжектируется, а байты моделей/лексиконов GROBID никогда не
  распространяются.
- Уровень доказательства — `contract`, а не source-bound parity (D358).

## Операторы

```bash
# ABI + последовательность (T01/T02) и негативы
cargo test -p da-parse --test native_fulltext_contract
cargo test -p da-parse --lib native_fulltext

# межстраничный reading order и continued-run (T03)
cargo test -p da-parse --test native_fulltext_reading_order

# sentence/language как non-CRF шаги (T04)
cargo test -p da-parse --test native_fulltext_sentence_contract

# независимая приёмка: лексический сканер, SUT не вызывается (T05)
cargo test -p da-parse --test native_fulltext_same_n

# opt-in против локального grobid-home (модельные байты не копируются):
#   без DA_GROBID_MODEL_ROOT тест печатает MISSING и выходит, никогда не PASS
DA_GROBID_MODEL_ROOT=/path/to/grobid-home/models \
  cargo test -p da-adapters --test native_fulltext_real_model

# карантин и траектория (T06)
PYTHONPATH=scripts python3 -m pytest scripts/tests/test_scholarly_parse_governance.py -q
PYTHONPATH=scripts python3 -m governor --validate-yaml
PYTHONPATH=scripts python3 -m governor --phase preflight

# Rust-зеркало карантина + DATA-контракты
cargo test -p da-parse --test architecture_contract
cargo test -p da-parse --test data_contracts
```

Наблюдаемые сигналы полосы: набор зелёных/красных целей выше, печатаемая
квитанция oracle (канонизированный digest), типизированные отказы
(`NativeFulltextFeatureError`, `NativeFulltextSequenceError`,
`NativeFulltextReadingOrderError`, `CrfModelError::LimitsExceeded`) и
governor-флаги `policy_stamp` при протечке атома в production-границу.
Runtime-метрик у полосы нет и быть не может: `deployed=false`.
