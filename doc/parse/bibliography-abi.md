# Native bibliography + callouts: ABI, семейства и границы доказательства

Статус: **candidate/shadow-only; Java GROBID остаётся production authority**
Волна: M507 / S04 / T08
Дата: 2026-09-16

## Вывод

`da-parse` умеет нативно пройти bibliography-полосу и текстовую половину
callout-полосы: собрать точные feature-строки цитатной (30 колонок) и
reference-segmenter (28 колонок) моделей, декодировать их через scalar-CRF
(38 и 6 меток), сгруппировать серии по базовым именам в `<label>/<reference>/
<other>`-спаны и в 19 типизированных ролей цитат, прогнать **вложенные**
`name/citation` (18 колонок, 9 меток) и `date` (17 колонок, 7 меток) модели по
авторским/датовым кластерам, и разрешить in-text callout-маркеры
(`Fig. 3`, `Table 2`, `Eq. (4)`, `Ref. 12`, `[12]`) строго против инвентаря
меток тела.

Это **shadow-кандидат**: `deployed: false`, `import_eligible: false`,
`auto_promote: forbidden`, `decision_point: 6`, rollback-цель
`java-grobid-http@0.9.0-crf`. Никакой production route не изменён.

Документ намеренно называет и то, что offline **не** доказано (см. раздел
«Офлайн-ограничения»), а не только успехи.

## Компоненты полосы

| Модуль | Роль | Schema const |
|---|---|---|
| `crates/da-parse/src/native_feature_facts.rs` | общий per-token факт-слой (капитализация, цифры, пунктуация, single-char, year/month, http, lexical-лейны, обе scale-семьи) | `da-parse.native-feature-facts/v1` |
| `crates/da-parse/src/native_bibliography_gazetteer.rs` | восемь Java-лейнов как **позиционные спаны** + правило потребления `span_covers` | `da-parse.native-bibliography-gazetteer/v1` |
| `crates/da-parse/src/native_bibliography_features.rs` | рендерер 30- и 28-колоночного ABI (чистый: без I/O, env, time, tracing) | `da-parse.native-bibliography-features/v1` |
| `crates/da-parse/src/native_bibliography_sequence.rs` | bounded runner: rows → `map_input` → `viterbi_decode` → base-name серии и роли | `da-parse.native-bibliography-sequence/v1` |
| `crates/da-parse/src/native_bibliography_nested.rs` | вложенные `name/citation` и `date` семейства: строки, декод, типизированная проекция с байтовыми спанами | `da-parse.native-bibliography-nested/v1` |
| `crates/da-parse/src/callout_markers.rs` | грамматика in-text маркеров + инвентарное разрешение в тело | `da-parse.callout-markers/v1` |

Домен-типы (CRF reader/decoder, `LexiconBundle`) приходят в `da-parse` **только
через** `da-ports`: прямой импорт `da-domain` запрещён машинно
(`crates/da-parse/tests/architecture_contract.rs`), поэтому seam зафиксирован
реэкспортом `crates/da-ports/src/scalar_crf.rs` (D360).

## 30-колоночный citation ABI

Колонки 0..=28 — это то, что модель реально читает (`#rdr#81/28/0`), плюс
колонка 29 — training-label placeholder, который `printVector()` печатает
всегда. Итого ровно **30** полей, разделённых пробелом, одна строка на
допущенный токен.

```
0 token · 1 lowercase · 2-5 prefix(1..4) · 6-9 suffix(1..4) · 10 lineStatus
· 11 capitalisation · 12 digit · 13 singleChar · 14 properName · 15 commonName
· 16 firstName · 17 lastName · 18 isKnownLocation · 19 year · 20 month
· 21 http · 22 isKnownCollaboration · 23 journalOrAbbrevJournal
· 24 isKnownConferenceTitle · 25 isKnownPublisher · 26 isKnownIdentifier
· 27 punctType · 28 relativePosition(12 bins) · 29 label placeholder
```

Ловушки, зафиксированные кодом (а не комментариями класса):

- **`capitalisation` печатается, а не вычисляется «по лестнице»**: при
  `digit == ALLDIGIT` печатается `NOCAPS`, поэтому `2017` — `ALLCAP` по
  лестнице и `NOCAPS` в строке;
- **`http` (21) берётся из gazetteer URL-лейна**, а не из `FeatureFactory.http`
  паттерна; у reference-segmenter gazetteer-прохода нет и он использует именно
  паттерн — это два разных механизма с одним именем;
- **`lineStatus`** — статус строки, а `alignmentStatus` в цитатной строке
  отсутствует вовсе (он есть только у reference-segmenter).

## 28-колоночный reference-segmenter ABI

Модель читает колонки 0..=25 (`#rdr#70/25/0`); `email` закомментирован
upstream, поэтому строка **28**, а не 29 колонок. Декод идёт **по строке**:
`alignmentStatus` относится ко всей строке, `relativePosition` — позиция токена
в строке, пунктуационный профиль — профиль строки; `max_line_length` —
блочный максимум, против которого строка масштабируется.

Две колонки (`firstName`, `locationName`) upstream несут
`/* TODO: to review, never set! */` и никогда не присваиваются: печатается `0`
даже там, где лексикон сказал бы иначе. Рендерер воспроизводит **напечатанное**
значение, а не намерение.

## Вложенные семейства: 18 и 17 колонок

`name/citation` и `date` — **другие** модели с **другой** feature-лестницей, и
30-колоночная цитатная строка к ним неприменима (это был риск 1 в
`507-04-RESEARCH.md`).

| Семейство | Колонки | Что читает модель | Метки |
|---|---|---|---|
| `name/citation` | 18 (`string` … `isKnownTitle`) | `0..=17` (`#rdr#63/17/0`) | 9 |
| `date` | 17 (`string` … `punctType`) | `0..=16` (`#rdr#50/16/0`) | 7 |

`FeaturesVectorName.printVector()` печатает 21 поле, `FeaturesVectorDate` — 18;
эмитится только **референсируемое подмножество**. Колонка, которую модель не
может сослаться, не может изменить декод, а её эмиссия заявила бы паритет,
которого волна не устанавливала. `isKnownSuffix` (18), `punctType` (19) и
training-label placeholders намеренно не эмитятся.

Семантическая разница, которую легко проглядеть: колонка 14 у `name/citation`
— это `commonName`, а не цитатный `properName`. Лексические блоки двух лестниц
различны и не взаимозаменяемы.

`isKnownTitle` (кол. 17) — **фразовый** лейн
(`Lexicon.tokenPositionsPersonTitle`, многословные записи вроде
`Professor Emeritus`), которого gazetteer не публикует. Он приходит как
типизированный `NativePersonTitleCoverage` (инклюзивное `start <= n <= end`);
блок без спанов даёт all-`0` колонку — типизированную missingness, не
фабрикацию.

## Label-наборы

| Модель | Метки | Порядок |
|---|---|---|
| `citation/model.wapiti` | **38** (19 ролей × {bare, `I-`}) | verbatim из label-блока модели: не сгруппирован по ролям, не отсортирован, не спарен как `<x>, I-<x>` |
| `reference-segmenter/model.wapiti` | **6** | `I-<other>`, `I-<label>`, `<label>`, `I-<reference>`, `<reference>`, `<other>` |
| `name/citation/model.wapiti` | **9** | модель-порядок |
| `date/model.wapiti` | **7** | модель-порядок |

Порядок таблиц — не косметика: label id индексирует весовые блоки модели
(`uoff[o] + y`, `boff[o] + yp*Y + y`), поэтому пересортированная таблица молча
неверна.

## Gazetteer: позиционные спаны, не membership

Восемь лейнов воспроизводят `FastMatcher.matchLayoutToken`: многословные
фразовые окна (до `GAZETTEER_MAX_PHRASE_TOKENS = 24`), разделители прозрачны и
сохраняют индексы внутри фразы, whitespace-нормализующие токены разрывают
открытую фразу. Потребление — **`span_covers(start <= n <= end)`**, это и есть
предикат `addFeaturesCitation`; single-token membership — *другой* предикат и
здесь не используется (D361).

Лейны: `JournalNames`, `AbbrevJournalNames`, `ConferenceNames`,
`PublisherNames`, `LocationNames`, `CollaborationNames`, `IdentifierPattern`
(DOI/arXiv shape), `UrlPattern` (`urlPattern1` shape). Бюджеты:
1 MiB текста, 16 384 токена, 65 536 записей на лейн, 65 536 спанов на вызов.

`IdentifierPattern` и `UrlPattern` реализованы как bounded non-backtracking
**shape-правила** и явно документированы как НЕ source-bound воспроизведение
паттерн-набора GROBID. Реальные определения живут в lexicon-конфигурации
локального `grobid-home`, который D358 запрещает коммитить.

## Группировка меток: равенство базовых имён

Серия — максимальный блок одинаковых **базовых** имён: снять префикс `I-`,
сравнить базы. Префикс `I-` сам по себе ни открывает, ни продолжает серию
(правило D359, исправленное на header-полосе real-model харнессом). Одна
реализация (`bibliography_label_runs`) обслуживает оба семейства: две
реализации позволили бы reference- и citation-полосам разойтись ровно на том
правиле, которое уже пришлось исправлять.

Проверка ширины строки выполняется по **двум** представлениям одной строки
(`split(' ')` и `split_whitespace()`): ABI-разделитель — пробел, а разделитель
декодера — любой whitespace, поэтому колонка, протащившая таб, меняет сетку,
не меняя число строк; именно двойной вид позволяет `WrongArity` это поймать.

## Callouts: грамматика маркера и разрешение в тело

Текстовая половина — `callout_markers.rs` — отвечает на два вопроса одной
грамматикой: какие маркеры печатает текст и в какой элемент тела ведёт каждый
маркер. Грамматика закрытая и детерминированная (без regex-зависимости):

- ключевые слова по классам: `Figure`/`Fig.`/`Figs.`, `Table`/`Tab.`,
  `Equation`/`Eq.`, `Section`/`Sec.`/`§`, `Reference`/`Ref.`, плюс русские
  написания (`Рис.`, `Табл.`, `Уравнение`, `Раздел`); побеждает длиннейшее
  совпадение, и ключевое слово обязано быть **сопровождено** адресуемой меткой
  (`Figure of speech` не маркер);
- формы меток: arabic (`3`, `2.1`), suffixed (`3b`), roman (только верхний
  регистр), alpha (`B`), диапазоны (`2-4`), скобочные (`(4)`, `[12]`);
- приклеенные формы (`Fig.4`, `§2`) читаются из остатка ключевого токена, но
  только arabic-метки: `Figure's` не должно давать маркер из хвостового `s`;
- скобочные номера (`[12]`) классифицируются как bibliography-маркеры без
  ключевого слова; голое `12` — маркером не является.

Разрешение строго инвентарное: `CalloutBodyInventory` публикует только те
метки, которые элемент тела реально объявляет, и каждая цель несёт
`CalloutLabelBasis` (печатный текст против ordinal-конвенции `[12] → bibl #12`).
Метка, которую заявляют два элемента, — `Ambiguous`, никогда не «первый
победил»: молчаливый выбор цели и есть фабрикация. Ненайденное остаётся
видимым как типизированное `Unresolved`
(`NoTarget` | `TargetNotInInventory` | `Ambiguous` | `RangeTooWide`).

Единственный вектор амплификации — диапазон: `MAX_CALLOUT_RANGE_SPAN = 64`,
шире — не расширяется и **не сужается** молча. `MAX_CALLOUT_TEXT_BYTES = 4 MiB`,
`MAX_CALLOUT_MARKERS = 100_000`.

Этот модуль **не** слит с соседними плоскостями: `native_callout_classification`
— форматная (токен-шейпы Java `CalloutAnalyzer`, ordinal-карта `[12]`),
`native_callout_matching` — digest-матчер по финализированным квитанциям. Это
разные вопросы, и слияние заставило бы один план отвечать за чужую семантику.

## Фикстуры: рукописные синтетические Wapiti-модели

| Фикстура | Семейство |
|---|---|
| `crates/da-parse/tests/fixtures/native_bibliography_citation_synthetic.wapiti` | citation (30 колонок, 38 меток) |
| `crates/da-parse/tests/fixtures/native_bibliography_reference_synthetic.wapiti` | reference-segmenter (28, 6) |
| `crates/da-parse/tests/fixtures/native_bibliography_name_synthetic.wapiti` | `name/citation` (18, 9) |
| `crates/da-parse/tests/fixtures/native_bibliography_date_synthetic.wapiti` | `date` (17, 7) |

Ожидания в CI посчитаны вручную против этих рукописных моделей. Реальные
модели GROBID в репозитории не хранятся (D358, `redistributable: false`), и
записанный вывод реальной модели **не** является независимым oracle:
наблюдаемые последовательности печатаются и никогда не пишутся как goldens.

## Знаменатели (receipts)

Независимый same-n oracle — `crates/da-parse/tests/native_bibliography_same_n.rs`
— собирает ожидания лексическим сканом по SHA-привязанным TEI
(`crates/da-parse/tests/fixtures/preserved/`: `hub_2507.19457`,
`rlm_2512.24601`, `reca_2605.26525v1`), структурно отрезав SUT-полосу:

| Знаменатель | Значение | Канонизация |
|---|---|---|
| `biblStruct` в теле | **207** = 80 / 49 / 78 | аналитический `biblStruct` в `teiHeader` исключён (иначе 210) |
| `<ref type="bibr">`анкеры | **380** = 123 / 99 / 158 | — |
| … targeted | **217** = 87 / 71 / 59 | анкер объявляет `target` |
| … targetless | **163** = 36 / 28 / 99 | анкер `target` не объявляет |
| смежные фрагментированные пары | **168** = 43 / 32 / 93 | — |
| `<author>` в блоках | **2185** = 665 / 947 / 573 | — |
| header-авторы | **36** = 21 / 4 / 11 | знаменатель header-полосы |
| строки / токены библиографии | **3135** / **15415** | тот же лексический реконструктор |

Targeted и targetless остаются **двумя** знаменателями: единое пулированное
число скрыло бы эрозию любой из сторон (MEM992).

SUT-сторона привязана к **токен-знаменателю** 15415, а не к собственному
выводу билдеров: ровно 15415 citation-row × 30 колонок и 15415
reference-segmenter-row × 28.

`citation_names` = **2198** — **названный гэп**: это Java-популяция имён,
которая не равна ни 207, ни 2185, ни 36/35, и не выводится никакой
пулинг-комбинацией этих полос. Отдельный тест фиксирует `assert_ne` по всем
суммам. Закрывать гэп фабрикацией или подменой знаменателя запрещено
(MEM1049/MEM1104/MEM1112).

## Остаток версии 0.9.1 против 0.9.0-crf

ABI транскрибирован из вендоренного дерева **0.9.1**, а pin'овая rollback-цель
— `java-grobid-http@0.9.0-crf`. Причина в том, что байты четырёх моделей
(`reference-segmenter`, `citation`, `name/citation`, `date`) между двумя
версиями **байт-идентичны**: изменился Java-код вокруг моделей, не сами веса и
не feature-контракт. Опциональный прогон harness против локального
`grobid-home` (версия runtime — `/api/version` → `0.9.1`) подтверждает
структурные инварианты моделей.

Это остаток, а не эквивалентность: если будущий upstream сменит веса или
список колонок, транскрибированные таблицы и ширины должны быть перевыверены
против новых байтов, а не «подогнаны» под наблюдаемые последовательности.

## Границы доказательства

Уровень доказательства — **contract**, и это НЕ source-bound parity:

1. **Нет сохранённого ALTO/layout.** Рядом с тремя фикстурами лежат только
   `*.tei.xml`. Поэтому рамка same-n — **TEI→атомы**, а не PDF→атомы, и лозунг
   «от PDF через layout к ссылкам» offline не проверяется.
2. **Паттерн-лейны — shape-правила.** `IdentifierPattern`/`UrlPattern` не
   воспроизводят паттерн-набор GROBID источник-в-источник.
3. **Реальные модели — только opt-in.** В CI — рукописные синтетические
   Wapiti-модели; для реальных проверяются только структурные инварианты
   (объявленные числа меток и арности, `validate_loaded_crf_model`,
   двустороннее согласие словаря) плюс декод-пробник.
4. **Lexicon-ресурсы не в репозитории.** Лексические колонки не являются
   утверждением о лексиконах GROBID; bundle инжектируется.
5. **Позиционная ось имеет ограниченный остаток.** Java накапливает позицию по
   *всем* layout-токенам (включая пробельные); ABI отказывает whitespace-токену,
   потому что протащенный разделитель испортил бы арность строки. Реальный
   layout-производитель обязан передать список, где пробелы уже учтены, иначе
   колонки 23/28 несут ограниченный остаток.
6. **Callout-цели — ordinal/печатные метки, не Java-координаты.**

## Регистрация и карантин (governor)

- `data/parse/grobid_compatibility.yaml`
  `defaults.bibliography_sequence_quarantine` — **единственный** источник
  запрещённого набора и production-границ для этой полосы (мутация
  `capabilities[].state` не может убрать токен или границу из проверяемого
  набора). `independently_selectable` остаётся ровно `[header, references]`
  (MEM1216).
- Границы: `crates/da-cli/src/source_cli.rs`, `crates/da-ports/src/parser.rs`,
  `crates/da-parse/src/parser.rs`, `crates/da-parse/src/projector.rs`.
- Существующий карантин S07/T01 (`compose_references(` /
  `run_reference_sequence(`) **не тронут**: governor держит только snake_case
  call-site атомы, а PascalCase-имена типов — на Rust-стороне в
  `crates/da-parse/tests/architecture_contract.rs` (MEM1115, D362).
- `crates/da-cli/examples/derived_maturity_rehearsal/derived_family_adapters.rs`
  `BIBLIOGRAPHY.allowed_files` / `candidate_symbols` регистрируют новые модули и
  символы в закрытом каталоге семейств.
- `data/parse/comparison_contract.yaml`
  (`native_header_reference_composition_shadow.lanes.references` и
  `component_receipts.references`) и `data/parse/native_fulltext_composition.yaml`
  (`component_receipts.references`, `source_evidence.s12_named_denominators`)
  несут receipts полосы: schema versions, раздельные знаменатели, named gap и
  offline-лимиты. Никакого пулинга с header-знаменателями.

## Офлайн-ограничения (что не доказано)

1. Нет сохранённого ALTO → **PDF→ABI недоказуем** offline.
2. Рамка same-n — TEI→атомы, не PDF→атомы.
3. 3135/15415 — лексические receipts, а не записанные Java-goldens.
4. `citation_names` 2198 — названный гэп, никогда не пулится с 207.
5. Паттерн-лейны — shape-правила, не source-bound паттерны.
6. Callout-разрешение — ordinal/печатные метки, не Java-координаты.
7. Последовательности реальной модели — структурные инварианты, не oracle.
8. Lexicon/modeл-байты инжектируются и никогда не распространяются (D358).

## Операторы

```bash
cargo test -p da-parse --lib native_feature_facts
cargo test -p da-parse --lib native_bibliography_features
cargo test -p da-parse --lib native_bibliography_gazetteer
cargo test -p da-parse --lib native_bibliography_sequence
cargo test -p da-parse --lib native_bibliography_nested
cargo test -p da-parse --test native_bibliography_contract
cargo test -p da-parse --test native_bibliography_same_n
cargo test -p da-parse --test callout_markers
cargo test -p da-parse --test architecture_contract
cargo test -p da-parse --test data_contracts
# opt-in против локального grobid-home (модельные байты не копируются):
DA_GROBID_MODEL_ROOT=<grobid-home>/models \
  cargo test -p da-adapters --test native_bibliography_real_model -- --nocapture
```

Env не задан → harness печатает `MISSING: DA_GROBID_MODEL_ROOT is not
configured` и возвращается: **никогда не PASS**. Отказы load/validate
санитизированы (без абсолютного пути) — искать по относительному
`<family>/model.wapiti`.

Никогда не запускать `cargo test --workspace` (rebuild librocksdb, 5–10 мин).
