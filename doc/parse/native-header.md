# Native header: полная полоса заголовка и границы доказательства

Статус: **candidate/shadow-only; Java GROBID остаётся production authority**  
Волна: M507 / S03 / T07  
Дата: 2026-09-15

## Вывод

`da-parse` умеет нативно пройти header-полосу целиком: собрать 29-колоночный
ABI признаковых строк из layout-токенов, декодировать реальную scalar-CRF модель
`models/header/model.wapiti` (44 метки, 1 326 339 наблюдений, 82 паттерна) и
спроецировать серии меток в поля композиции (`title`, `authors`, `affiliation`,
`date`, `email`, `ids`). Это **shadow-кандидат**: `deployed: false`,
`import_eligible: false`, `auto_promote: forbidden`, `decision_point: 6`,
rollback-цель `java-grobid-http@0.9.0-crf`. Никакой production route не изменён.

Документ намеренно называет и то, что offline **не** доказано (см. раздел
«Офлайн-ограничения»), а не только успехи.

## Компоненты полосы

| Модуль | Роль | Schema const |
|---|---|---|
| `crates/da-parse/src/native_header_features.rs` | рендерер 29-колоночного ABI (чистый: без I/O, env, time, tracing) | `da-parse.native-header-features/v1` |
| `crates/da-parse/src/native_header_sequence.rs` | runner декодирования: rows → `map_input` → `viterbi_decode` → серии полей | `da-parse.native-header-sequence/v1` |
| `crates/da-parse/src/native_header_fields.rs` | проекция декодированных спанов в `title`/`email`/`ids` | `da-parse.native-header-fields/v1` |
| `crates/da-parse/src/native_composition.rs` | аддитивная проводка `compose_header_with_fields` / `run_header_sequence_with_fields` | `da-parse.native-composition.outcome/v1` |

Домен-типы (CRF reader/decoder, lexicon bundle, feature ABI) приходят в
`da-parse` **только через** `da-ports`: прямой импорт `da-domain` запрещён
машинно (`crates/da-parse/tests/architecture_contract.rs`), поэтому seam
зафиксирован реэкспортами в `crates/da-ports/src/{scalar_crf,lexicon,lib}.rs`.

## 29-колоночный ABI

Колонки декодированы из самой модели (`#rdr#82/28/0` ссылается только на
колонки 0..28) и совпадают позиция-в-позицию с
`FeaturesVectorHeader.printVector()`:

```
0 token · 1 lowercase · 2-5 prefix(1..4) · 6-9 suffix(1..4) · 10 blockStatus
· 11 lineStatus · 12 alignmentStatus · 13 fontStatus · 14 fontSize · 15 bold
· 16 italic · 17 capitalisation · 18 digit · 19 singleChar · 20 properName
· 21 commonName · 22 year · 23 month · 24 locationName · 25 email · 26 http
· 27 punctType · 28 largestFont
```

Колонки 29 (`smallestFont`) и 30 (`largerThanAverageFont`) существуют в
`printVector()` версии 0.9.0, но модель их не использует; `firstName`
закомментирован в Java. Рендерер воспроизводит **именно** то, что читает
модель.

Ловушки, зафиксированные кодом (а не комментариями класса):

- `digit`: `ALLDIGIT` / `CONTAINSDIGITS` (**plural**) / `NODIGIT`;
- `alignmentStatus`: `LINEINDENT` / `ALIGNEDLEFT` (комментарий класса говорит
  INDENTED/CENTERED — код побеждает);
- `fontStatus`: `NEWFONT` / `SAMEFONT`; `fontSize`: `HIGHERFONT` /
  `SAMEFONTSIZE` / `LOWERFONT`;
- `punctType`: `PUNCT` / `OPENBRACKET` / `ENDBRACKET` / `DOT` / `COMMA` /
  `HYPHEN` / `QUOTE` / `NOPUNCT`;
- block/line статусы: `BLOCKSTART|BLOCKIN|BLOCKEND`,
  `LINESTART|LINEIN|LINEEND`.

Lexicon-backed колонки (`properName`, `commonName`, `year`, `month`,
`locationName`, `email`, `http`) принимают `LexiconBundle` как **инжектируемый
чистый вход**. Модельные и lexicon-байты GROBID в репозитории не хранятся
(`data/parse/grobid_model_rights.yaml`, D358): `redistributable: false`,
`runtime_resource_root: grobid-home/`.

## Группировка меток: семантика `writeField`, не BIO

Наивная посылка «непрефиксованная метка открывает серию, `I-` продолжает» в
GROBID **отсутствует**. Первоисточник — `HeaderParser.writeField`
(`grobid-core/.../engines/HeaderParser.java`): решение «продолжать или открыть»
принимается сравнением **базовых** имён полей уже очищенных от `I-`; префикс
`I-` сам по себе не открывает и не продолжает серию.

Правильное правило, реализованное раннером и закреплённое тестами:

1. снять префикс `I-` → `base`;
2. если `base` совпадает с `base` предыдущего токена — токен продолжает текущую
   серию;
3. если отличается — текущая серия закрывается, открывается новая с этим `base`.

Обе формы одного поля (`<field>` и `I-<field>`) считаются **одним** полем и не
открывают новую серию друг относительно друга. Типизированные отказы
(`EmptySequence`, `WrongArity`, `UnknownLabel`, `CountMismatch`) сохранены.

## Проекция полей

`project_native_header_fields` переносит декодированные спаны в
`NativeHeaderFields`: `title`, `email`, `ids`. Отображение зафиксировано явно:

- `<title>` → `title`;
- `<email>` → `email`;
- `<pubnum>` → `ids` (в наборе меток нет `<ids>`/`<idno>`; идентификаторы
  заголовка в GROBID приходят из `<pubnum>` и header post-processing, поэтому
  отображение документируется, а не изобретается).

Текст поля реконструируется как тексты токенов (первая колонка ABI),
соединённые одним пробелом (`HeaderFeatureRow::as_text`); оригинальный
PDF-микроспейсинг намеренно не угадывается. Ёмкости ограничены:
`MAX_NATIVE_HEADER_PROJECTION_ROWS`, `MAX_NATIVE_HEADER_FIELD_VALUES`,
`MAX_NATIVE_HEADER_FIELD_CHARS`.

Существующие сигнатуры `compose_header` / `run_header_sequence` не изменены:
поля проносятся аддитивно через `compose_header_with_fields` /
`run_header_sequence_with_fields`, от которых зависят quarantine, каталог
семейств и три существующих набора тестов.

## Независимая приёмка same-n

`crates/da-parse/tests/native_header_same_n.rs` — независимый oracle:
SHA-256 пины трёх сохранённых TEI, тест-локальные знаменатели и лексический
сканер `teiHeader`, который **не** вызывает `TeiLifter`, `compose_header`,
`run_header_sequence` или `project_native_header_fields`. Отсутствие SUT в
области построения ожиданий проверяется структурно.

| Знаменатель | Значение |
|---|---|
| Java raw authors | 36 |
| Native accepted authors | 35 |
| `<titleStmt>` titles | 3 |
| `<title` в `teiHeader` | 6 |
| emails | 3 |
| `<idno>` (всего / checksum) | 6 / 3 |
| `<pubnum>` | 3 |
| `<date` в `teiHeader` | 6 |
| `<date` по всему документу | 173 |

`hub` 21 (Java raw) vs 20 (native accepted) остаётся **названным разрывом**:
он не закрывается изобретением имени и не пулится
(`pooling_accepted_with_raw: forbidden`).

## Opt-in harness реальной модели

`crates/da-adapters/tests/native_header_real_model.rs` владеет I/O. Без
переменной окружения тест печатает `MISSING` и выходит — **никогда** не PASS.
С заданным ресурсом проверяются структурные инварианты: 44 метки, длина
последовательности равна числу токенов, все метки ⊆ объявленного набора.
Абсолютные пути в коде не зашиты; модельные байты не копируются и не
коммитятся (D358).

```bash
DA_GROBID_CRF_MODEL_PATH=/root/vendor-source/grobid/grobid-home/models/header/model.wapiti \
  cargo test -p da-adapters --test native_header_real_model -- --nocapture
```

## Governance-замки

- `data/parse/grobid_compatibility.yaml` `defaults.header_sequence_quarantine`
  — DATA-источник запрещённого набора header-атомов и production-границ
  (`crates/da-cli/src/source_cli.rs`, `crates/da-ports/src/parser.rs`).
  Governor читает этот блок; мутация `capabilities[].state` не может убрать
  токен из проверяемого набора.
- `crates/da-cli/examples/derived_maturity_rehearsal/derived_family_adapters.rs`
  `HEADER.allowed_files` / `candidate_symbols` регистрируют новые модули и
  символы в закрытом каталоге семейств.
- `data/parse/comparison_contract.yaml`
  `native_header_reference_composition_shadow` и
  `data/parse/native_fulltext_composition.yaml` `component_receipts.header`
  несут receipts header-полосы (schema versions, знаменатели, offline-лимиты).
- `data/parse/feature_tagger_training_parity.yaml` остаётся закрытой
  11-колоночной advisory-поверхностью; `native_feature_tagger.rs` **не**
  является входом CRF и в этой полосе не расширяется.

## Офлайн-ограничения (что не доказано)

1. **Нет сохранённого ALTO/layout для трёх фикстур.** В
   `crates/da-parse/tests/fixtures/preserved/` лежат только `*.tei.xml`;
   единственные ALTO в репозитории — синтетические структурные фикстуры
   `da-adapters`. Поэтому лозунг «от PDF через layout и сегментацию к
   заголовку» **не** проверяется end-to-end на сохранённых фикстурах: для
   этого нужен PDF→layout runtime (pdfalto), который вне области этой волны.
   Честный уровень доказательства — **контракт**: ABI рендерера на
   hand-authored layout-фикстуре, проекция полей и same-n на сохранённом TEI,
   декодирование реальной модели — в opt-in harness.
2. **Рамка same-n — TEI→атомы, а не PDF→атомы.** Существующие oracle-хелперы
   выводят входы из TEI (`header_authors(xml)`, `date_input(xml)`); волна
   расширяет **полевое** покрытие этой рамки (title/email/ids), а не
   притворяется PDF-путём.
3. **`hub` 20 vs 21** — названный DATA-разрыв, не пул (см. выше).
4. **Последовательности реальной модели не являются независимым oracle.**
   Записанный вывод модели — oracle только если он пересчитывается из
   независимых входов. В CI используются hand-authored синтетические Wapiti
   модели с посчитанными вручную ожиданиями; для реальной модели проверяются
   только структурные инварианты.
5. **Lexicon-ресурсы не в репозитории.** Колонки properName/commonName/
   locationName/email/http/year/month требуют GROBID lexicon, который не
   распространяется (D358). Рендерер принимает `LexiconBundle` как чистый
   вход; runtime-харнесс строит его из сконфигурированного локального корня,
   CI — из крошечного проектного бандла.
6. **Производительность не гейтится.** `map_input` строит
   `BTreeMap<&str, usize>` по 1 326 339 наблюдениям на каждый вызов; для
   заголовка в несколько тысяч токенов это разовая стоимость. Она измеряется
   в harness, но `da-domain` в этой волне не оптимизируется без отдельного
   доказательства.

## Операторы

```bash
cargo test -p da-parse --lib native_header_features
cargo test -p da-parse --lib native_header_sequence
cargo test -p da-parse --test native_header_same_n
cargo test -p da-parse --test native_composition_contract
cargo test -p da-parse --test data_contracts
cargo test -p da-parse --test architecture_contract
DA_GROBID_CRF_MODEL_PATH=<...>/models/header/model.wapiti \
  cargo test -p da-adapters --test native_header_real_model -- --nocapture
```

Никогда не запускать `cargo test --workspace` (rebuild librocksdb, 5–10 мин).
