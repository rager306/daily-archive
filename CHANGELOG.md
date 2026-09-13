# Changelog

## 2026-09-13 (4) — oracle-инфраструктура миграции: базовая линия и recorder стадий GROBID

Начата волна M506 (перенос каскада GROBID в самостоятельный Rust-parser). Перед
любой работой по переносу зафиксирована фактическая база.

Ключевое открытие: **моделей и лексиконов нет в исходниках GROBID**. На теге
0.9.1 каталог `grobid-core/src/main/resources` содержит только `xslt` и `xq`,
то есть модельная идентичность переносится **образом**, а не файлами репозитория,
и любой шаг миграции, читающий модели из vendor-дерева, строился бы на ложной
посылке. Артефакты измерены изнутри закреплённого образа и получили
воспроизводимые агрегатные отпечатки: models — 29 файлов, 14 семейств
(`ccb89327…`); lexicon — 50 файлов, 9 групп (`bcc1a00d…`); pdfalto — 257 файлов
(`abb64da7…`). Размеры реальные: `header` 70 МБ, `segmentation` 182 МБ.

Реализован съём debug-дампа GROBID (`debugMode`), сверенный по исходникам
сервиса, а не выведенный из общих соображений: `DEBUG_MODE = "debugMode"` как
`@FormDataParam` с `@DefaultValue("0")`. Ответ при включённом debug становится
`text/plain` вместо TEI, поэтому TEI и дамп — два отдельных запроса. Съём
переиспользует существующий транспорт (`input_part`, закрытые allowlist-формы,
общий `exchange_once` с таймаутами и лимитами байт), не дублирует его и не
входит в `ParserPort`: это offline-инструмент фиксации, а не production-маршрут.
Сырой TEI здесь сознательно не снимается — он уже есть в compatibility-обвязке.

Добавлен разбор дампа и учёт покрытия: понимаются обе формы заголовка
(`=== model: <name> ===` и `=== model: <name> (occurrence N of M) ===`), тело
секции берётся срезом без копирования, покрытие считается **по моделям, а не по
секциям**. Отсутствие секции трактуется как отсутствие доказательства, но не как
отсутствие вычисления.

Живая проверка на закреплённом PDF (2406.13163, релиз 0.9.1): полный дамп —
`captured`, 120 вызовов, 12 различных моделей, 3 187 215 байт, при этом `date`
вызывается 46 раз, `name-citation` — 45, `figure` — 14, а `segmentation` и
`header` — по одному разу; фильтр `--models segmentation` даёт `filtered` с одной
секцией; неизвестная модель даёт HTTP 400 → типизированный `invalid_request` и
ненулевой код возврата.

Инвариант непротечки доказан **на уровне wire**: по записанному телу реального
HTTP-запроса (детерминированный tokio-stub, тот же, что у остальных контрактов)
debug-запрос несёт `debugMode`, а без фильтра поле `models` не отправляется
вовсе. Проверка только по исходнику не поймала бы форму, собранную где-то ещё.

Тесты поймали реальный дефект до коммита: `=== model:  ===` принималось как
модель с пустым именем, что завышало бы покрытие анонимным вызовом.

Итог: da-parse `--tests` 765/0 (было 754), lib 92/0 (было 83), clippy
`-D warnings` и fmt чисто; четыре атомарных коммита.

## 2026-09-13 (3) — восстановлена и проверена способность откатить GROBID

При проверке фактического состояния M505 обнаружен разрыв между заявленным и
реальным: `provenance.json` утверждал `rollback_image_retained: true` и называл
digest `sha256:24ba90eb...`, `AGENTS.md` и `.docker/grobid-release.override.yml`
документировали команду отката по этому digest, но самого образа
`grobid/grobid:0.9.0-crf` в Docker **не было**. Документированный откат упал бы
с `No such image` — то есть режим восстановления существовал только на бумаге.

Восстановлено по digest (не по тегу, чтобы гарантировать точно тот же аудитнутый
release):
`docker pull grobid/grobid@sha256:24ba90eb1c959f65d812bcdb2cf79c677fa5fd7b95235de616b8bc9fa1317849`
— полученный digest совпал с задокументированным.

Способность отката проверена **end-to-end**, а не по факту наличия образа:
контейнер `daily-archive-grobid-rollback-check`, поднятый из образа отката на
свободном порту 18071, ответил `{"version":"0.9.0","revision":"0.9.0"}`;
после проверки контейнер удалён. Production при этом не затронут и продолжает
сообщать 0.9.1.

Состояние сделано явным и воспроизводимым, чтобы заявление не могло снова
устареть молча: в `provenance.json` добавлен блок `rollback` с каноническим
digest, явным признаком `local_presence_is_durable: false`, записью о
наблюдённой деградации (`decay_observed`), командой восстановления и
результатом end-to-end проверки; в override-файл добавлен шаг проверки и
восстановления образа по digest. Причина, по которой образ исчез (image GC или
prune), не установлена — важно, что локальный кэш образов не является durable
гарантией, и теперь это записано явно.

Проверено: `governor review` exit 0, `docs-drift` exit 0, governance pytest
438/0, JSON и YAML валидны, `docker compose config` собирается, production
отвечает 0.9.1.

## 2026-09-13 (2) — gate подтверждён целиком; устранён шторм WARN от сторонней PDF-библиотеки

`governor architecture` впервые завершился одним прогоном: **ARCHITECTURE GATE:
PASS**, exit 0, 16 шагов OK — включая два добавленных ранее feature-gated шага
(сборка примеров `--features pdf-layout --examples` и прогон `--tests`). Прежние
обрывы gate объяснялись конкуренцией за блокировку `target/`, а не дефектами.

Разбор логов приложения (регламент сессии) показал 0 ERROR, но ~2.5k WARN в
сутки, все от одного стороннего target `lopdf::object`: по одному предупреждению
на каждый нераскодированный глиф/различие кодировки шрифта в реальных PDF
(19 различных сообщений). Это заглушало собственный сигнал и обесценивало
документированный workflow «искать проблемы по уровню в логе».

Исправлено без хардкода и без нулевого варианта «выключить совсем»:
`crates/da-cli/src/logging.rs` получил композицию фильтра `base + LOG_SUPPRESS`,
где `LOG_SUPPRESS` — env-переменная со списком `target=level` директив (по
умолчанию `lopdf::object=error`). Выбран уровень `error`, а не `off`, поэтому
настоящие отказа разбора PDF остаются видимыми; директива называет только
сторонний target, поэтому ни один `da-*`/`kg-*` target не глушится. Значение
документировано в `.env.example` и `AGENTS.md`; композиция вынесена в чистую
функцию `compose_filter` с тремя unit-тестами (включая проверку, что директива
по умолчанию валидна и никогда не адресует собственные крейты).

Проверено: `cargo test -p da-cli --lib logging` 4/0; `cargo test -p da-cli
--lib` 219/0; fmt и clippy (`--lib --tests -D warnings`) OK. Реальный прогон
`native_visual_heads_shadow` с изолированным `LOG_DIR` и тем же манифестом, что
давал ~2500 предупреждений, теперь даёт **0 WARN**. Лимит: фильтр глобальный
(per-layer filtering требует nightly), то есть `LOG_SUPPRESS` действует на оба
приёмника — stderr и файл.

## 2026-09-13 — устранено скрытое предусловие и номинальное покрытие feature-gated тестов

`crates/da-cli/tests/layout_bakeoff_test.rs` требовал предварительно собранный
example `target/debug/examples/layout_bakeoff`, который объявлен с
`required-features = ["pdf-layout"]`, но секций `[[test]]` в `Cargo.toml` нет —
поэтому обычный `cargo test -p da-cli --tests` на чистом target падал с ложным
`prebuilt runner missing`. Проверка показала, что проблема системная: **девять**
файлов тестов da-cli объявлены как `#![cfg(feature = "pdf-layout")]` и потому в
дефолтном прогоне компилировались в ПУСТЫЕ цели — 29 тестов не исполнялись
вообще (418 против 447), то есть покрытие было номинальным, а не реальным.

Скрытые отказы: включение фичи вскрыло `fulltext_block_semantics_shadow_test`
(пример не был собран: `cargo test --test NAME` не собирает примеры) и реальный
семантический отказ `native_visual_heads_shadow_test` — тест требовал
`oracle_status == "not_observed"` от ВСЕХ heads, тогда как продукт намеренно
различает `unsupported` (пробел возможности/провижининга) и `not_observed`
(оценено и отсутствует; см. `NativeVisualAbsence` в примере). При этом тот же
тест в конце требует наличия `unsupported` outcome — ожидание было внутренне
противоречивым и слабее реального инварианта.

Исправлено: `layout_bakeoff_test.rs` получил `#![cfg(feature = "pdf-layout")]`
(следуя конвенции соседнего `fulltext_block_semantics_shadow_test.rs`), а
проверка в `native_visual_heads_shadow_test.rs` УЖЕСТОЧЕНА, а не ослаблена:
каждый head обязан иметь известный статус из закрытого словаря и связанный
знаменатель, а `unsupported` допустим ТОЛЬКО в outcome с
`failure_code == "unsupported"` — раньше такой кросс-инвариант не проверялся.

`governor architecture` больше не оставляет это покрытие номинальным: добавлены
финальные шаги `cargo build -p da-cli --features pdf-layout --examples` и
`cargo test -p da-cli --features pdf-layout --tests` (без хардкода списка целей,
поэтому будущие гейтнутые файлы попадают в область автоматически). Команда
задокументирована в `AGENTS.md`.

Проверено: обычный набор `cargo test -p da-cli --tests` — 418/0; набор с фичей
`--features pdf-layout --tests` — **447/0**; governor-тесты 438/0; синтаксис
`scripts/governor/__main__.py` корректен. Ограничение: шаги gate с включённой
фичей собирают крупные example-бинарники (≈283 МБ каждый); свободное место на
диске падало до 23 ГБ на пике.

## 2026-09-12 (2) — закрытие техдолга repo-gate, версионная идентичность GROBID 0.9.1, uv-проект

Восстановлен красный repo-gate, унаследованный от HEAD `3b327d7ce` (затронутые
файлы не изменялись в этой сессии). Архитектурный контракт: реестр `FamilyCatalog`
перенесён из production-дерева `da-application` в offline-обвязку
`crates/da-cli/examples/derived_maturity_rehearsal/` (main.rs + module), S04-тест
переписан без ослабления; маркер `NativeFulltext` уточнён до
`NativeFulltextComposition`, так как широкий маркер ловил законное семейство
`NativeFulltextBlocks` (отдельный контракт S16). DATA-контракты: объявлены
секции `population_contract`, `external_holdout_contract`,
`release_decision_package`, `rest_batch_asset_quarantine` и поле
`identity_sha256`; ожидания segmentation подняты к `document-segmentation.v2`
(`compatible_with v1` сохранён); `native_body_accounting` приведён к taxonomy v2;
перегенерирован каскад stale SHA-пинов; `deny_unknown_fields` сохранён, у части
полей `serde(default)` удалён (требования усилены), негативные `rejects_*` тесты
не ослаблены. Также исправлены 5 устаревших дайджестов
`governor/fixtures/scholarly_parse_plane/contract.positive.json` и привязка
мутации в `acceptance_evidence_test` (отступ 8/10 → фактические 6/8).
Итог: `cargo test -p da-parse --tests` 754/0 (было 63 падения),
`cargo test -p da-cli --tests` 424/0, `pytest scripts/tests` 438/0.

Версионная идентичность GROBID: probe `/api/version` fail-closed в batch и
load-путях, version-bound ключи стадий, `--allow-versionless-identity` только как
явный opt-out. Три offline-примера (`catalog_sweep_runner`,
`gold_corpus_canary`, `pipeline_figures`) больше не штампуют исторический
`0.9.0-crf`. Production переключён на 0.9.1 через
`.docker/grobid-release.override.yml` с сохранённым rollback-образом 0.9.0 и
неперезаписанными историческими пинами; `provenance.json` приведён к
фактическому состоянию (`production_switched_and_verified`, live 0.9.1).

Добавлен planning-реестр горизонта GROBID→Rust H01–H13
(`doc/review/grobid-rust-horizon-review.json` + governor DATA-контракт,
dispatcher и 17 тестов): валидность реестра не равна PASS реализации, закрытие
стадии требует machine-verifiable evidence. GAR-реестр сохранён без массового
закрытия.

Python переведён на uv: `pyproject.toml` (PyYAML, jsonschema, dev pytest) и
`uv.lock`; прямой `python3` не имел jsonschema/pytest и давал ложный INVALID
реестра. Проверки: `uv run python -m governor review`, `uv run python -m pytest
scripts/tests -q`.

Ограничения и честные оговорки: `layout_bakeoff_test` требовал example,
собранный с feature `pdf-layout`, — предусловие gate, а не дефект кода;
CBM-индекс `root-daily-archive` не обновился (count(n) = 490198 до и после двух
попыток, процессы остались) и не может служить доказательством по этим
изменениям — опора на GitNexus (82868/121150) и прямое чтение исходников.
Полный `governor architecture` одним прогоном в этом окружении не завершается;
все его шаги проверены по отдельности.

## 2026-09-12 — волна техдолга: holdout 5/5, reader bug, gold sources 200/200

GAR-015 external holdout закрыт: добавлен CLI флаг `parse-batch --timeout-secs`
(bound [1,3600], InvalidTimeout, `GrobidHttpBackend::with_timeout` wiring; timeout
исключён из content identity per ADR-075) — 5/5 внешних статей успешно распарсены
через production GROBID за 209s (было 2/5 при дефолтных 120s). Найден и исправлен
баг governor graph reader: Section text prop — `text` (ingest.rs:812), не
`raw_text`; ложный `graph_section_text_missing` на 28346 nodes устранён.
`gold_corpus_manifest` регенерирован с корнем `data/sources` (не `arxiv/`):
200/200 gold fixtures source-ready, `gold_unavailable_source_debt` устранён
(4 gnn-ch глав жили в `data/sources/textbook/` с chapter.html). M440 ASSESSMENT
получил post-seal note — reverse-ADR audit violations 1→0. `external_holdout_results`
конвертирован JSON→YAML (*.json forbidden в data/parse SCHEMA, ADR-075).
Governor preflight: 5 флагов → 1 (только информационный target/ bloat).
105/105 governor тестов PASS. Locks неизменны: D127, production authority,
три автомата, no auto-promote.

2026-09-08 — M498-0u7k1j/S05 T02: Java body observation closure sealed

Java P5 body observation remains the production stamp; the native accountant, offline join, cross-stage checker, and visual/reference shadow families remain quarantined. Rust architecture coverage now checks the four DATA body-accounting tokens in lockstep with the compatibility registry, while import, graph-write, and promotion remain frozen.

## 2026-09-07 — M496-27op9w/S05 T02: native body accounting quarantine sealed

Native P2 → body accounting remains a test-only shadow sequence: Rust architecture and Governor contracts now scan both `source_cli.rs` and `ParserPort` for body-accountant atoms, while preserving Java GROBID production authority. P5 remains `accountant_later`, DecisionPoint remains the six-wire closed set, and import, graph-write, auto-promotion, and deployment locks remain closed. Added data-driven negative governance coverage and refreshed the bound compatibility-ledger hash; no production route, graph write, import, or promotion changed.

## 2026-09-06 — M495-gnxzxu/S05 T01: reference sequence stays a test-only shadow runner

Reference sequence quarantine pinned in `crates/da-parse/tests/architecture_contract.rs`: `BIBLIOGRAPHY_NATIVE_PORT_TOKENS` 7 -> 8 with explicit `NativeCitationAnchorScanner` prohibition in `parser_port_carries_no_native_bibliography_methods_or_types` (R129). `run_reference_sequence` stays inside da-parse tests; source_cli.rs and parser.rs have zero `compose_references`/`run_reference_sequence` call sites and zero `NativeCitationAnchorScanner` mentions; parse-pdf stamps P4 from Java biblStruct citations (`stamp_p4_references_observe(&mut m, parsed.citations.len())`, production_authority java_http, citation_names_not_biblstruct true). Governor `_BIBLIOGRAPHY_QUARANTINE_TOKENS` untouched (still 2 snake_case call-site tokens). No SUT/runtime/YAML/graph/import change, no promotion.

## 2026-09-06 — M495-gnxzxu/S04 T01: native citation anchors same-n with fixture-bound malformed and oversized negatives

Native citation anchors same-n закреплён точным aggregate pin (380 total, 217 targeted, 163 targetless, 168 adjacent_fragment_pairs) и fixture-bound fail-closed негативами `fixture_bound_malformed_anchors_fail_closed_with_exact_reasons` и `fixture_bound_oversized_anchors_fail_closed_without_truncation` в `crates/da-parse/tests/native_citation_oracle.rs`: authority только test-local `FIXTURES`/`fixture_path`/`digest`/`scan_bibr_anchors`/`oracle_adjacent_pairs` (SHA-пины неизменны); незакрытый opening tag даёт Malformed MalformedOpeningTag, отсутствующий closing tag — Malformed MissingClosingTag, total cap ниже fixture length и anchor cap 16 ниже первого реального bibr — Oversized, never Recognized и never Truncated; empty остаётся Empty; мутации только in-memory копии, не входят в population 380; FIXTURES 3-tuple, тела S02/S03, scanner SUT, fixtures, YAML, native_composition/source_cli/parser не менялись. Shadow/same-n, no promotion, no import/graph change.

## 2026-09-06 — M495-gnxzxu/S03 T01: native reference segments same-n with fixture-bound empty and reorder negatives

Native reference segments same-n закреплён shadow-негативами `fixture_bound_mixed_empty_segments_fail_closed_as_empty_segment` и `checksum_bound_hub_reorder_preserves_n_but_breaks_ordered_identity` в `crates/da-parse/tests/native_citation_oracle.rs`: authority только test-local `FIXTURES`/`fixture_path`/`scan_fixture` (hub=80/reca=78/rlm=49/total=207, SHA-пины и ORDERED_RAW_DIGEST неизменны); смешанные `""` и `"  "` среди непустых fail closed как Malformed EmptySegment, all-empty/all-whitespace — отдельный outcome Empty, never Recognized; swap 0/1 на hub сохраняет n=80, но меняет source sequence и joined digest, пермутация ломает ORDERED_RAW_DIGEST. Добавлен только import уже экспортируемого `NativeReferenceListError`; тела S02/S04, fixtures/YAML, native_composition/source_cli/parser не менялись. Shadow/same-n, no promotion, no import/graph change.

## 2026-09-06 — M495-gnxzxu/S02 T01: invented citation DOI/title not admitted against preserved TEI oracle

Native citation fields same-n закреплён fail-closed негативом `shape_valid_invented_doi_and_title_are_not_admitted_as_oracle_evidence` в `crates/da-parse/tests/native_citation_oracle.rs`: authority только test-local `FIXTURES`/`fixture_path`/`scan_fixture` (hub=80/reca=78/rlm=49/total=207, title denominator=200, SHA-пины неизменны); shape-valid синтетика `doi:10.9999/not-in-corpus` и `title: Completely Fake Paper` распознаётся парсером как синтаксические кандидаты (Doi/ArticleTitle), но отсутствует в SHA-bound declared fields и ни в одном preserved raw_reference; моделируемое добавление даёт 207 -> 208 и нарушает exact same-n/zero-invention admission. Существующий substring-in-raw invented counter сохранён отдельно; production TEI lifter/composition/domain expected builders не импортируются; тела тестов S03 segmenter и S04 anchors, YAML/SHA-байты, parser/runtime/CLI wiring, graph/import/promotion/selectable locks не менялись. No promotion, no import/graph change.

## 2026-09-06 — M495-gnxzxu/S01 T01: citation names 2198 pinned as a separate DATA flag

Governor `scholarly_parse_plane` держит citation_names_total=2198 и references_total=207 раздельными именованными знаменателями на обеих DATA-поверхностях: `data/parse/native_fulltext_composition.yaml` и `native_fulltext_composition_shadow` из `comparison_contract.yaml` (включая comparison-shadow `java_lift_same_n`: citation_names_not_biblstruct=true, pooling_citation_names_with_references=forbidden, java_biblstruct_equals=references_total). Подмена 2198 на 207 и снятие запрета pooling дают существующий флаг `scholarly_parse_plane_acceptance_same_n_drift` с evidence по конкретной DATA-поверхности; нового violation label нет. Ключи selectable разные по имени (`independently_selectable` в composition, `independently_selectable_lanes` в comparison), оба ровно [header, references]. Scratch-root pytest мутации покрывают citation_names_total: 207 (обе поверхности) и pooling: allowed. YAML-байты и SHA-пины не менялись (DATA уже корректны). No promotion, no import/graph change.

## 2026-09-06 — M494-a1i14t/S04 T01: native affiliation same-n with document_id on preserved TEI identities

NativeAffiliation recognition and linking proven same-n on the three hub/rlm/reca SHA identities with explicit document_id on every call and every occurrence (MEM990). Recognition: test-local per-fixture expected (hub 21/0, rlm 4/0, reca 11/13; recognized 13/13) bound to the preserved TEI checksums; inputs are lexical raw_affiliation note texts scanned from pinned bytes, never from compose_header or TeiLifter. Linking: reca 13 occurrences across 11 authors link with document_id reca_2605.26525v1 on call and rows; hub 21 + rlm 4 zero-link authors stay visible as successful empty link sets (25 total). Missing document_id on call or occurrence and foreign-key mismatch fail closed with UnknownKey; invented organisation/key matches no preserved identity (aff0-aff3 / four oracle organisations). Linker takes a literal accepted name, never reparses personal names. No NativeAffiliation calls from source_cli.rs or parser.rs (already zero; architecture_contract quarantine holds). independently_selectable stays [header, references] (untouched). No graph write.

Green: da-parse native_affiliation_contract 9/9, native_affiliation_link_contract 10/10, architecture_contract 57/57.

## 2026-09-06 — M494-a1i14t/S03 T01: native personal-name same-n on preserved TEI identities

NativePersonalNameParser proven same-n on the three hub/rlm/reca SHA identities: test-local per-fixture expected (hub 20/665, rlm 4/973, reca 11/560; header 35, citation_names 2198) bound to the preserved TEI checksums, one candidate evaluation per oracle occurrence, same-n drift names fixture identity. Expected builder imports no TeiLifter/HeaderAuthor and calls no compose_header/compose_references/run_header_sequence. Empty author is Empty; invented single-token is Ambiguous; invented et-al is Unsupported; a shape-valid invented surname matches no preserved oracle identity. Hub native accepted 20 vs Java header_raw 21 stays a named DATA gap in `data/parse/native_fulltext_composition.yaml` `source_evidence.s12_named_denominators.java_lift_same_n`, never closed by compose; citation_names 2198 is never pooled with header 35 or biblStruct 207. parse-pdf still stamps P3 from Java header_authors (pinned by architecture_contract). No DATA edit, no splice, no graph write.

Green: da-parse native_personal_name_oracle 9/9, native_personal_name_contract 6/6, java_lift_same_n_oracle 2/2, architecture_contract 57/57.

## 2026-09-06 — M494-a1i14t/S01 T01: observation lanes pinned off independently selectable

Governor `scholarly_parse_plane` fails closed when `document_segmentation`, `figure`, `table`, `formula`, or `learning_policy` enters `independently_selectable` on the native composition authority, the header/reference shadow, or the fulltext composition shadow in `comparison_contract.yaml`. `strata.lanes` may stay a superset. Selectable stays exactly `[header, references]` on both YAML files (SHA-bound). Negative fixture already mutates selectable with `document_segmentation`; Rust `data_contracts` adds the same negatives. No `strata.lanes` shrink, no `document_segmentation` pass, no `this_wave_promotes` change, no splice, no graph write.


## 2026-09-05 — M493-jzmctv/S12 T01: candidate admission and receipt completeness pinned

Independent Rust DATA contracts pin S12 candidate admission over three YAML
ledgers and receipt completeness. Unified validator; ablation path is the real
nested DATA shape. No family promote. No splice. No graph. D127 false. P5 later.
Hub 20 vs 21 stays DATA. `visual_bundle` stays later.

Green: da-parse data_contracts S12 tests 2/2 plus S06–S11 regression.

## 2026-09-05 — M493-jzmctv/S11 T01: PARSE_BARE fail-closed for parser lifetime

Governor classifies `GrobidParser::from_env()` by binding use in the enclosing
fn, not by a nearby `is_alive` token. Health then `parse_pdf` on the same
binding is `PARSE_BARE`. Pure health-only, `with_profile`, helper, tests and
comments stay allowed. Clean-root allows only DATA-rooted constructor hits
under declared `scan_roots`; remaining corpus examples stay T02 inventory.
Filenames are not hardcoded. No splice. No graph. D127 false. P5 later.
Hub 20 vs 21 stays DATA.

Green: scholarly parse governance 222.

## 2026-09-05 — M493-jzmctv/S10 T01: six DecisionPoint variants and exact later set pinned

Independent Rust DATA+architecture contracts pin six DecisionPoint wires and the
exact later set. New local `s10_*` helpers only; production symbols untouched.
`visual_bundle` stays later, not a DecisionPoint variant. P5 stays accountant_later.
Hub 20 vs 21 stays DATA. No splice. No graph. D127 false.

Green: da-parse architecture 52; data contracts 86; governor 57; source_cli 21.

## 2026-09-05 — M493-jzmctv/S09 T01: P5 accountant later pinned in independent Rust contracts

P5 stays `accountant_later`, not a live stamp. `stamp_p5` stays absent. Independent
Rust DATA+architecture contracts pin the later row without touching production
source. Hub 20 vs 21 stays DATA. No splice. No graph. D127 false.

Green: da-parse architecture + data contracts for P5 later.

## 2026-09-05 — M493-jzmctv/S08 T02: Rust/DATA Java P6 contracts, visual_bundle stays later

P6 stamps Java figure/table/formula counts. Visual atoms stay shadow. `visual_bundle`
stays later, not a DecisionPoint variant. Hub 20 vs 21 stays DATA. No splice.
No graph. D127 false.

Green: da-parse architecture + data contracts for P6 policy and visual-bundle later.

## 2026-09-05 — M493-jzmctv/S07 T02: Rust/DATA Java P4 contracts, bibliography stays shadow

P4 stamps Java `ParsedArticle.citations` (biblStruct). Bibliography atoms stay
shadow. `java_biblstruct_equals` is `references_total`; citation-names stay a
boolean flag, never pooled with biblStruct. Hub 20 vs 21 stays DATA. No splice.
No graph. D127 false.

Green: da-parse architecture 45; data contracts 78; scholarly governance 170.

## 2026-09-05 — M493-jzmctv/S06 T02: Rust/DATA Java P3 contracts, hub 20 vs 21 stays DATA

Header atoms stay shadow: `compose_header`, `run_header_sequence` and
`NativeDateParser` stay out of parse-pdf and ParserPort. P3 stamps Java
`ParsedArticle.header_authors`. Hub native 20 vs Java 21 is a named
`java_lift_same_n` DATA gap, not compose. No splice. No graph. D127 false.

Green: da-parse architecture + data contracts; Governor quarantine 151.

## 2026-09-05 — M492-k0s52t/S01: P3 observe must not hardcode hub native 20

stamp_p3 writes named_native_accepted_gap as a boolean flag, not count 20.
java_header_n still comes from Java lift. DATA still names hub 20 vs Java 21.
Governor flags a stamp that writes named_native_accepted_gap 20. No splice.
No graph. D127 false.

Green: da-cli lib source_cli 21; scholarly_parse_governance 105.

## 2026-09-05 — M491-lj9ybx/S01: Hub real_tei must not pool sections with biblStruct

Hub Java sections are 69: body heads minus figure caption heads, not
biblStruct 80. real_tei pins sections at 69 and keeps citations >=80.
Governor flags pooling those two pins. extract_sections is unchanged.
No splice. No graph. D127 false.

Green: da-parse lib real_tei 15; scholarly_parse_governance 104.

## 2026-09-05 — M490-3jfk24/S01: Remove dead extract_head_level from TeiLifter

extract_head_level is gone. Live lift reads head n through
extract_head_n_string and derives level from path components. A live
extract_sections test covers n=3, missing n, and n=abc. No splice.
No graph. D127 false.

Green: da-parse lib tests:: 67 (skip real_tei). real_tei hub 69 vs
≥80 sections is pre-existing on HEAD, not this slice.

## 2026-09-05 — M489-rj64mf/S01: P5 accountant later must not look like a live consult

YAML comment no longer lists later as fsm_consult. P5 observe_mode stays
accountant_later. Governor flags stamp_p5 and a live P5 mode.
grobid_compatibility.yaml SHA rebound. No stamp_p5 added. No splice.
No graph. D127 false.

Green: scholarly_parse_governance 103.

## 2026-09-05 — M488-isklf9/S01: parse-pdf stamps P9 conservation observe without calling the engine

parse-pdf merges cascade_observe.P9 as candidate_lift. production_authority
is tei_lift. engine_called and computed_this_run are false. tracked_verdict
is conserved. The comparator stays in da-parse. Governor flags a parse-pdf
without the P9 stamp. No splice. No graph. D127 false.

Green: da-cli lib source_cli 21; scholarly_parse_governance 101.

## 2026-09-05 — M487-mr0jkj/S01: parse-pdf stamps P8 consolidation observe from live profile

parse-pdf merges cascade_observe.P8 from the live GrobidProfile.
observe_mode is production_stamp. production_authority is java_profile.
P8 is not pooled with P0 admit. ConservationEngine is not called.
Governor flags a parse-pdf without the P8 stamp. No splice. No graph.
D127 false.

Green: da-cli lib source_cli 20; scholarly_parse_governance 100.

## 2026-09-05 — M486-5cggw1/S01: parse-pdf stamps P7 callout observe from Java lift

parse-pdf merges cascade_observe.P7 from Java visual_callouts.
observe_mode is production_stamp. native_surface is none.
empty_count_not_success stays true. Callouts are not pooled with P6
figures. Governor flags a parse-pdf without the P7 stamp. No splice.
No graph. D127 false.

Green: da-cli lib source_cli 19; scholarly_parse_governance 99.

## 2026-09-05 — M485-jgybkl/S01: parse-pdf stamps P1 layout observe beside Java

parse-pdf merges cascade_observe.P1 from Java layout_json presence.
None is typed absence, not an empty lattice. Native layout is not called.
P0 P2 P3 P4 P6 stay. Governor flags a parse-pdf without the P1 stamp or
with LayoutProvider names. No splice. No graph. D127 false.

Green: da-cli lib source_cli 18; scholarly_parse_governance 98.

## 2026-09-05 — M484-ra6fpq/S01: parse-pdf stamps P0 admit observe from live consults

parse-pdf merges cascade_observe.P0 from live grammar_route and
profile_route consults. observe_mode is production_stamp. native_surface
is none. P2 P3 P4 P6 stay. Governor flags a parse-pdf without the P0
stamp. No splice. No graph. D127 false.

Green: da-cli lib source_cli 17; scholarly_parse_governance 97.

## 2026-09-05 — M483-x7e137/S01: P6 cascade map must not list later as fsm consult

cascade_stages.P6.fsm_consult is none. visual_bundle stays later, not a
DecisionPoint. Governor flags later names in fsm_consult. grobid_compatibility.yaml
SHA rebound. No splice. No graph. D127 false.

Green: scholarly_parse_governance 96.

## 2026-09-05 — M482-k8go0z/S01: parse-pdf stamps P6 visual observe beside Java

parse-pdf merges cascade_observe.P6 from Java figures/tables/formulas.
P2 P3 P4 are kept. empty_count_not_success stays true. Native visual
heads are not called. Governor flags a parse-pdf without the P6 stamp
or with native visual names. No splice. No graph. D127 false.

Green: da-cli lib source_cli 16; architecture_contract native_visual 2;
scholarly_parse_governance 95.

## 2026-09-05 — M481-x8opx5/S01: parse-pdf stamps P2 segmentation observe beside Java

parse-pdf merges cascade_observe.P2 from Java sections.len().
P3 and P4 are kept. tei_divs_not_native_regions stays true.
NativeSegmenter is not called. Governor flags a parse-pdf without the
P2 stamp or with native segmentation. No splice. No graph. D127 false.

Green: da-cli lib source_cli 15; architecture_contract segmentation 2;
scholarly_parse_governance 94.

## 2026-09-05 — M480-ccwqii/S01: parse-pdf stamps P4 references observe beside Java

parse-pdf merges cascade_observe.P4 from Java citations.len().
P3 is kept. citation_names_not_biblstruct stays true. compose_references
is not called. Governor flags a parse-pdf without the P4 stamp or with
native compose. No splice. No graph. D127 false.

Green: da-cli lib source_cli 14; scholarly_parse_governance 93.

## 2026-09-05 — M479-xin07g/S01: parse-pdf stamps P3 header observe beside Java

parse-pdf stamps cascade_observe.P3 from Java header_authors.len().
production_authority java_http, observe_mode shadow_receipt,
named_native_accepted_gap 20. run_header_sequence is not called.
Governor flags a parse-pdf without the stamp or with native compose.
No splice. No graph. D127 false.

Green: da-cli lib source_cli 13; scholarly_parse_governance 92.

## 2026-09-05 — M478-k37wo4/S01: DATA maps P0-P9 cascade stages onto Java native and FSM

cascade_stages lists ten parse stages. P0 consults grammar_route and
profile_route. P3 header native is shadow_receipt, not production.
Governor flags native production_authority or empty-native fsm_consult.
SHA rebound in the same change. da-parse modules unchanged. No splice.
No graph. D127 false.

Green: scholarly_parse_governance 91.

## 2026-09-05 — M477-m1qp4n/S01: demo sona-policy stamps consult_status and refuses empty-native

run_sona_policy reads consult_status. Empty-native and later are refused
before consult. grammar_route prints demo=true parse_pdf_consult=true.
batch_admission prints demo=true parse_pdf_consult=false. Governor
flags a demo CLI that omits the stamp. DecisionPoint enum unchanged.
No splice. No graph. D127 false.

Green: da-cli lib background_tasks 4; scholarly_parse_governance 90.

## 2026-09-05 — M476-33ifna/S01: DATA names live vs empty-native DecisionPoints

consult_status: parse-pdf consults grammar_route and profile_route.
empty_native_unknown_point is concurrency_tune, retry_policy,
summary_depth. batch_admission has native rules but is not consulted.
later stays head_fallback, visual_bundle, critic_sample. Governor flags
empty-native listed as a parse-pdf consult. SHA rebound in the same
change. DecisionPoint enum unchanged. No splice. No graph. D127 false.

Green: scholarly_parse_governance 89.

## 2026-09-05 — M475-d1npw9/S01: DATA names Java header raw vs native accepted gap

java_lift_same_n maps Java header_authors to header_raw_total 36, not
accepted 35. Java biblStruct equals references_total 207, not
citation_names 2198. Hub native accepted 20 vs Java 21 stays named.
Governor flags pooling those pairs. SHA rebound in the same change.
No splice. No graph. D127 false.

Green: native_fulltext_composition_data_contract 2; scholarly_parse_governance 88.

## 2026-09-05 — M474-90q2i6/S01: shadow report declares synthetic_clone identity

Native fulltext composition shadow stamps identity_kind=synthetic_clone
and unique_source_sha256=1. documents=3 is three clones of one hybrid
fixture, not hub/rlm/reca. Governor flags undeclared SHA clones and
preserved names without preserved_three_sha. SHA rebound in the same
change. No splice. No graph. D127 false.

Green: scholarly_parse_governance 87.

## 2026-09-05 — M473-9h6jhe/S01: Java lift same-n vs independent three-SHA oracle

TeiLifter::lift_tei on hub/rlm/reca matches independent header_raw
21/4/11 and biblStruct 80/49/78. Header and citation populations stay
separate. Native compose is not the expected builder. Hub native
accepted=20 stays a named gap, not Java truth. No splice. No graph.
D127 false.

Green: java_lift_same_n_oracle 2.

## 2026-09-05 — M472-nxpy4n/S01: governor flags bare from_env PDF constructors

DATA lists allowed GrobidParser::from_env roles. Governor scans declared
da-cli roots and flags a bare from_env used as a PDF parse constructor
(policy_stamp). Health probes, with_profile, consult helper and tests stay
allowed. Facade default stays rich. SHA of grobid_compatibility.yaml rebound
in the same change. No graph. D127 false.

Green: scholarly_parse_governance 85.

## 2026-09-05 — M471-oi80qz/S02: production PDF constructors consult ProfileRoute

ingest, batch-ingest, extract, summarize, scheduler, da load journey,
multimodal production, eval_batch, eval_extract, figure_context,
gepa_hub_research, pipeline_figures, gold_corpus_canary and catalog_sweep
PDF parsers use corpus_grobid_parser() (DATA structural default).
Health is_alive probes keep from_env. Facade default stays rich.
No graph. D127 false.

Green: parse_profile 6; load 20; production 11; load_journey 2; bins+examples check.

## 2026-09-05 — M471-oi80qz/S01: shared ProfileRoute consult helper

consult_corpus_grobid_profile fail-opens to DATA structural (D204).
da load PDF path uses the consult instead of from_env().profile().
GrobidParser::from_env stays historical rich. HTML path unchanged.
No graph. D127 false.

Green: parse_profile 6; grobid_profile_from_route 2; stamp_parse_grammar.

## 2026-09-05 — M470-485ukv/S02: parse-pdf consults ProfileRoute

parse-pdf maps consulted ProfileRoute to GrobidProfile instead of hardcoding
structural(). Receipt grobid_profile is stamped only on the Java parse-pdf
path. grammar stamp no longer claims a Java profile. DATA default stays
structural (D204). Rich is a closed consult variant, not corpus default.
No WASM wat on this point. No graph. D127 false.

Green: resolve_parse_profile 4; parse_grammar 12; stamp + grobid_profile CLI 4.

## 2026-09-05 — M470-485ukv/S01: FSM ProfileRoute native default structural

DecisionPoint now parses profile_route as its own point, not an alias of
grammar_route. Closed ProfileRoute {structural, rich}. Native fallback
consults DATA default=structural (D204). Rich is named, never implied.
No WASM wat. Governor SHA-binds data/policies/registry.yaml and flags
non-structural default via policy_stamp. No graph. D127 false.

Green: da-domain fsm_policy 15; da-application fsm_policy_fallback 5;
parse_grammar 8; wasm_policy 3; scholarly_parse_governance 83.

## 2026-09-05 — M469-j23uk8/S01: stamp grobid_profile separate from grammar_profile

parse-pdf receipts now stamp grobid_profile=structural next to FSM grammar_profile.
Stdout names both fields so they cannot be mixed. Java GROBID call stays
GrobidProfile::structural() (D204 corpus default). No rich. No fingerprint
rotation. No graph. D127 false.

Green: stamp_parse_grammar unit; choose_parsed_action unit.

## 2026-09-04 — M468-4k3p7s/S01: ledger and leftover pins stop claiming loss_detected

`defaults.promotion.reason` no longer says tracked conservation is still
loss_detected. Pristine cache_identity and parse_compat_baseline pins are
Conserved. Frozen lift hashes rotated after caption-head skip (sections
264→183). Mutated citation deletion still LossDetected. SHA of
grobid_compatibility.yaml bound in scholarly_parse_plane in the same change.
this_wave_promotes stays none. Java production_route. No graph. D127 false.

Green: cache_identity 23; parse_compat_baseline 16; data_contracts 74;
test_scholarly_parse_governance 83.

## 2026-09-04 — M467-ase1bu/S03: Conserved acceptance evidence after region honesty

Regenerated artifacts/parse_compat/acceptance_evidence.json from ConservationEngine.
Three preserved TEI documents are Conserved; no_silent_loss is true; verdict PASS.
SHA bound in scholarly_parse_plane in the same change. Java production_route.
No graph. D127 false.

Green: acceptance_evidence offline; test_scholarly_parse_governance 83.

## 2026-09-04 — M467-ase1bu/S02: flat lifter skips caption heads; anchors bag-aligned

TeiLifter extract_sections_flat no longer treats figure/table caption `<head>` as a
section. Four flat walkers share next_section_head so tables, formulas and
anchors stay on the same windows. Citation anchors compare as a digest bag:
bibliography grouping is not Reordered. Pristine conservation on the three
preserved TEI files is Conserved. Hub body-head floor is 69 (was 80 while
captions inflated the IR). Java production_route. No graph. D127 false.

Green: extract_ 45; conservation lib 9; conservation_contract 14; conservation_properties 6.

## 2026-09-04 — M467-ase1bu/S01: three-region scanner honesty for back annex

ConservationEngine now has three TEI regions: header, body, back. Annex `<head>`
in `<back>` is not a Section. A targeted `<ref type="bibr">` in `<back>` is a
typed CrossReference remainder, not CitationAnchor linkage_loss. Back
`biblStruct` stays CitationEntry (bibliography lives there). Pins:
section_missing 0/0/0, linkage_loss 0/0/0, rlm cross_refs 67. Remaining
LossDetected is CountMismatch: the flat GROBID head scan still emits figure
caption heads as sections (S02). Java production_route. No graph. D127 false.

Green: conservation lib 9; conservation_contract 14; conservation_properties 6.

## 2026-09-04 — M466-xxitm7/S03: honest acceptance evidence after scanner rewrite

Regenerated artifacts/parse_compat/acceptance_evidence.json from ConservationEngine.
Verdict stays FAIL: remaining extra body heads (hub 1, rlm 6) and rlm
linkage_loss=1 are still typed losses. no_silent_loss stays false. SHA bound
in scholarly_parse_plane in the same change. Java production_route. No graph.
D127 false.

Green: acceptance_evidence offline; test_scholarly_parse_governance 83.

## 2026-09-04 — M466-xxitm7/S02: unresolved bibr is not linkage loss

A GROBID `<ref type="bibr">` without `target` is an unresolved marker, not a
lost citation link. ConservationEngine classifies it as a typed CrossReference
remainder. linkage_loss is now 0/1/0 on hub/rlm/reca (was 36/29/99). Remaining
rlm=1 is a targeted bibr the nested-div lifter still does not attach. Java
production_route. No graph. D127 false.

Green: conservation lib 7; conservation_contract 14.

## 2026-09-04 — M466-xxitm7/S01: scanner honesty for header biblStruct and caption heads

ConservationEngine no longer counts the paper's own header sourceDesc/biblStruct as a
cited work, and no longer counts figure caption heads as sections. That was a
scanner lie, not a missing bibliography. Remaining pinned section_missing is
1 (hub) and 6 (rlm) extra body heads the nested-div lifter does not emit.
citation_missing is 0. linkage_loss stays typed. Java production_route.
No graph. D127 false.

Green: conservation lib 6; conservation_contract 14.

## 2026-09-04 — M465-sqt7w0/S07 W6: identity ignores workers, no family promote

Reuse stamp_parse_grammar and layout_smp receipts. TeiIdentity and D204 fingerprints
do not contain worker/smp/concurrency. SMP semantic receipts stay equal for
workers 1..=cap. grobid defaults.promotion: this_wave_promotes none, deployed false,
max_non_candidate_families 1. Conservation is still loss_detected so no family
is candidate-promoted. Governor adds family_promote_cap and worker_identity_leak.
Java production_route. Graph frozen. D127 false.

Green: identity_and_fingerprint_ignore_worker_count; layout_smp every_worker_count;
architecture_contract 38; data_contracts 74; test_scholarly_parse_governance 83.

## 2026-09-04 — M465-sqt7w0/S06 W5: missing Image is typed absence not zero figures

Reuse detect_figure_heads/table/formula. An observation without Image/Graphic
returns Terminal NotObserved, not Figure with empty receipts. Table and Formula
stay independent on the same observation. Ledger stage.figure_table_formula is
shadow with native_visual.rs. Governor adds typed_visual_absence and
zero_count_from_absence. Fulltext composition stays forbidden. Java
production_route. No graph. D127 false.

Green: native_visual_contract 6; architecture_contract 38;
test_scholarly_parse_governance 79.

## 2026-09-04 — M465-sqt7w0/S05 W4: Java body bytes survive splice, overlap is hard fail

Reuse compose_native_fulltext. Only admitted header/reference subtrees splice.
Exact <body> bytes after splice equal Java. listBibl inside body is
ChangedReadingOrder and returns the original envelope. DATA independently_selectable
stays header,references; segmentation/visual stay fallback. Governor adds
body_java_fallback and reading_order_change. Java production_route. No graph.
D127 false.

Green: native_fulltext_composition_contract 6; native_fulltext_composition_data_contract 2;
architecture_contract 38; test_scholarly_parse_governance 75.

## 2026-09-04 — M465-sqt7w0/S04 W3: bibliography segment then fields then compose

Reuse NativeReferenceSegmenter, NativeCitationParser, and compose_references.
Ledger citation names/citation/list/reference_segmentation are shadow with crate
surfaces (were absent or a CLI example). run_reference_sequence is segment then
fields then compose; empty list or empty middle segment cannot invent a
ReferenceCandidate. Structural profile still has consolidate_citations=false.
Governor adds bibliography_sequence and lost_references. Java production_route.
No graph. D127 false. Patent citation endpoints stay absent.

Green: native_composition_contract 10; architecture_contract 38; data_contracts 74;
test_scholarly_parse_governance 71.

## 2026-09-04 — M465-sqt7w0/S03 W2: header sequence date then name then affiliation

Reuse NativeDateParser, NativePersonalNameParser, NativeAffiliationParser,
and compose_header. Ledger surfaces that hid none behind prose now point at
native_date.rs, native_personal_name.rs, native_affiliation.rs, and
native_composition.rs (shadow). run_header_sequence stops on empty date and
rejects empty or foreign authors. Governor ledger_surface treats none-prefix
prose as missing; header_sequence and invented_author gates added.
Architecture proxy set includes W0 health/version. Java production_route.
No graph. D127 false.

Green: native_composition_contract 8; architecture_contract 38; data_contracts 74;
test_scholarly_parse_governance 67.

## 2026-09-04 — M465-sqt7w0/S02 W1: lexical lattice honesty, one region per token

Reuse, not a new parser. LayoutTokenKind::from_lexical_text never returns
Image or Formula. NativeSegmenter assigns exactly one closed region per
admitted token including opaque visuals. Ledger stages pdfalto_layout and
document_segmentation are shadow with real surfaces (were fallback/none).
Governor adds lexical_visual_mix and region_coverage. Existing 9-lane
segmentation report stays mixed pass/unsupported; unsupported is typed
absence, not a fake 202-page green. Java production_route. No graph.

Green: layout_contract 8; native_segmentation_contract 5; data_contracts 74;
test_scholarly_parse_governance 63.

## 2026-09-04 — M465-sqt7w0/S01 W0: honest GROBID probes, measured conservation, policy stamp

ADR-076 / D321: three automata, graph frozen, M464 was platform-only.
GROBID GET /api/health and /api/version are fail-closed probes on
GrobidHttpBackend (not ParseBackend). TeiIdentity.backend_version is
additive; historical cache keys unchanged until a version is stamped.
Acceptance evidence measures ConservationEngine on three preserved TEI
files instead of hardcoding no_silent_loss. Current lift is loss_detected
on all three, so verdict is FAIL (honest). parse-pdf stamps policy_source
wasm|native_fallback|none and policy_hash or none. Governor scholarly_parse_plane
gains ledger_surface, backend_identity, policy_stamp, silent_loss.
No graph writes. D127 false.

Green: da-parse backend_contract+data_contracts+cache_identity+conservation;
da-cli source_cli; test_scholarly_parse_governance 59; clippy da-parse/da-cli.

## 2026-08-25 — M462-ieh2tt: load network is on by default

--allow-network was an extra step. AcquisitionPolicy.allow_network now
defaults true. da load / acquire-arxiv fetch missing catalog assets unless
--disallow-network or DA_LOAD_DISALLOW_NETWORK=1. Hidden --allow-network
is a no-op so old scripts keep parsing. ensure_only() stays offline for
tests. Catalog hits still work with allow_network false (ADR-056).
D127 false. D290.

Green: policy_network; resolve_network; mock_miss; ensure_miss;
acquisition_batch 2; arxiv_acquisition_contract 4; clippy
da-domain/cli/ports/application --lib; cargo build -p da-cli.

## 2026-08-25 — M461-6665iq: URL locator is network intent for load FSM

Pasting an arXiv URL is now a first-class path. locator_implies_network is
true only for http(s); bare ids stay fail-closed (ADR-056). da load on a
URL allows network unless DA_LOAD_ALLOW_NETWORK is 0/false and defaults
stages to ingest+extract. da source run canonicalizes the URL to the
arXiv id, ensures the catalog, then walks the existing FSM. Parser
try_parse_arxiv_locator is unchanged (GitNexus CRITICAL). D127 false. D289.

Green: locator_implies 1; resolve_network 1; url_locator_defaults 1;
normalize_source_target 1; clippy da-domain/cli --lib; cargo build -p da-cli.

## 2026-08-25 — M460-7tqron: operator graph-query honours exhausted negatives

M459 gated agent-run; da graph-query and da query still hit the store without
admit_retrieval. admit_operator_read maps paper-scoped admit_retrieval onto
GraphQueryCliError::Runtime containing negative_blocked. Empty paper_id
fail-opens. graph_query::run consults before opening the store. da query
by-arxiv/by-vid uses the same helper; count/orphans/graph-stats stay ungated.
CLI reads DA_RETRIEVAL_PAPER_ID and DA_NEGATIVE_MEMORY_DIR inline (dual bin/lib).
Not Samyama. D127 false. D288.

Green: graph_query 9 including admit_operator_read + operator_query_needs_admit;
clippy -D warnings da-cli --lib; cargo build -p da-cli.

## 2026-08-25 — M459-sfjk31: agent retrieval honours exhausted negatives

Vasiliev gap after M456-M458: LLM stages and Layer H freeze existed, but
AgentReadPlane.query could still return evidence for a paper whose critic
repair exhausted. admit_retrieval is paper-scoped: any exhausted negative
for that paper blocks. Empty paper_id fail-opens. Blocked is typed
AgentFailureCode::NegativeBlocked (FailedVisible), never rows=0.

AgentRunUseCase::with_negatives consults after plan validation and before
identity/query. CLI reads DA_RETRIEVAL_PAPER_ID and opens
JsonlNegativeMemory from DA_NEGATIVE_MEMORY_DIR (inline env; dual bin/lib).
Not Samyama. D127 false. D287.

Green: admit_retrieval 4; all_failure_codes; agent_run 26 including Spy
plane query_calls=0; da-cli agent_run 22; clippy -D warnings
da-domain/application/cli --lib.

## 2026-08-25 — M458-qxcimz: ContextBundle freezes as_of policy_hash and MSC

ADR-053 Layer H was identity + evidence + claims only. ContextBundle now
carries optional as_of, policy_hash, scope, task_kind, compiled_at
(serde default empty so legacy JSON still loads). freeze_id hashes
identity + as_of + policy_hash + scope + task_kind; compiled_at is
observational and excluded. AgentRunRequest.freeze is stamped onto the
bundle at construction. CLI da agent-run reads DA_CONTEXT_AS_OF,
DA_CONTEXT_POLICY_HASH, DA_CONTEXT_SCOPE, DA_CONTEXT_TASK_KIND; compiled_at
from SystemClock. Domain stays clock-free. Not Samyama. D127 false.

Green: freeze_id 2 + legacy JSON 1 + context_bundle 2; agent_run 24
including run_stamps_request_freeze; da-cli agent_run 22 including env
freeze; clippy -D warnings da-domain/application/cli --lib.

## 2026-08-25 — M457-jj82w7: figure_analysis_stage admits before multimodal LLM

Same Vasiliev/M455 gap as summaries: figure_analysis_stage called the
vision model without consulting negative memory. execute() now admits
canonical_id+figure_analysis (pattern=None) before analyze(). Missing
canonical_id fail-opens (legacy unbound contexts). Blocked maps to
Terminal error_class=negative_blocked; SpyModel is never called.

CLI production.rs and load_journey.rs open JsonlNegativeMemory from
DA_NEGATIVE_MEMORY_DIR (inline env; bin/lib dual compile). Open failure
fail-opens the stage without a store.

Green: figure_analysis_stage 11 (blocked skips LLM + missing id
fail-open), clippy -D warnings da-application/cli --lib.

## 2026-08-25 — M456-g1nv83: summaries_agent_stage admits before LLM

M455 CLI gate was bypassed by the pipeline. Now run_summaries_agent
consults NegativeMemoryPort (stage=summaries, pattern=None) before the
LLM call. BlockedExhausted is a typed error; SpyLlm is never invoked.
Admit without a pattern fail-closes on any exhausted record for that
stage (pipeline reuse). CLI build-summary and eval-frozen open
JsonlNegativeMemory from DA_NEGATIVE_MEMORY_DIR.

Live: critic-repair exhausted on p-block/summaries → build-summary
prints blocked_exhausted, no model call. Not Samyama. D127 false.

Green: domain negative_memory 7, summaries_agent_stage 2, clippy
-D warnings da-domain/application/cli --lib.

## 2026-08-25 — M455-5jxhte: negative semantic knowledge blocks reuse

Vasiliev gap: critic Reject/repair_exhausted was logged but retrieval
could reuse the same claim. Now:

- Domain: NegativeMemoryRecord + admit_claim (key = paper_id+stage+pattern).
  Exhausted reject denied; other paper/stage/resolved allowed.
- Port NegativeMemoryPort separate from CVF AgentMemory.
- Jsonl adapter `{dir}/{paper_id}.jsonl`. Path: DA_NEGATIVE_MEMORY_DIR.
- persist_if_exhausted only on exhausted outcomes.
- CLI: critic-repair writes negative; `da source memory-admit` denies.
  Live: 2507.19457 figure_analysis schema violation → blocked;
  other paper/stage → allowed. Not Samyama. D127 false.

Green: domain negative_memory 6, application critic_repair_stage 5,
adapters jsonl 1, clippy -D warnings touched crates --lib.

## 2026-08-25 — source capture: Vasiliev 2026 three graph roles

Added Substack article as a blog source (not graph truth, D127):
`data/sources/blog/vasiliev2026knowledge-graph-memory-graph/`.
HTML captured locally (gitignored `data/**/*.html`, sha256 in identity.json);
tracked: identity.json, citation.bib, article.md sidecar, NOTES.md, loader.
Cited from ADR-040 (tier = roles), ADR-048, ADR-053 Layer H, ADR-074.
Research note: `doc/research/vasiliev-2026-three-graph-roles.md`.

## 2026-08-25 — M454-c3yx94: critic repair process + Governor current-milestone

Two process gaps closed:

1. Critic repair was DATA + a domain library with zero application callers.
   AgentManifest now parses optional `repair.mechanism.reject_triggers`.
   `run_critic_repair()` calls `route_repair` and returns typed RepairOutcome.
   CLI: `da source critic-repair` (DA_CRITIC_* env). Live: reject →
   figure_analysis/reanalyze_with_diagnostics; confirm → no action;
   attempts=2 → repair_exhausted.

2. Governor flagged every ROADMAP 🔄 empty PLAN (M447/M449/M450 ghosts).
   Empty-PLAN diagnostics now apply only to STATE-active (or preferred
   ROADMAP-active if STATE missing). Live: only M446-rw9a2p/S02 remains.

Green: domain agent_manifest 4, application critic_repair_stage 4,
pytest test_gsd_planning_reader 15, clippy -D warnings da-domain/
application/cli --lib.

## 2026-08-25 — M453-aym18d: parse-pdf consults grammar_route

Process gap from M452: WASM/native policy host existed, parse-pdf still
hardcoded grammar_profile=arxiv_pdf_dot.

- DATA: grammar_route.profile_map + default_profile in
  data/policies/registry.yaml. xy_cut stamps intent, keeps arxiv_pdf_dot
  parse path (no XY-Cut CLI yet).
- Application: resolve_parse_grammar() — consult + map + fail-open (8 tests).
- CLI: run_parse_pdf stamps grammar_profile, grammar_route, policy_hash.
  FSM parsed-stage: page_outline + TOC cache → parse-pdf-toc, else parse-pdf.
  Features from clap / DA_PARSE_* env / TOC observation cache.
- ADR-074 accepted (W1–W3 infra; LoRA still D136).

Green: domain fsm_policy 11, application parse_grammar 8, cli stamp+choose 2,
clippy -D warnings da-domain/application/cli --lib.

## 2026-08-25 — M452-mhx138 S02: WASM FSM policy host (ADR-074 W3 infra)

Hot-swappable policy modules at FSM decision points. No LoRA (D136).

- Domain: `ArticleFeatures`, `GrammarRoute`, `Admission`, `PolicyRegistry`
  (YAML-driven native thresholds). 10 tests.
- Port: `FsmPolicyPort` in da-ports. Native fallback in da-application
  (fail-open on Load/Instantiate/Io). 4 tests.
- Adapter: `SharedWasmPolicyHost` (wasmi 0.51) in da-adapters only.
  WAT fixtures under `data/policies/`. Hot-swap test: hash + decision
  change without process restart. 3 tests.
- DATA: `data/policies/SCHEMA.yaml` + `registry.yaml`; `policies` added
  to `data/SCHEMA.yaml` and governor structural_directories.
- CLI: `da source sona-policy <point> [--pages --bookmarks --quality]`
  Live: grammar_route typical → xy_cut (wasm sha256 stamped);
  monograph 700p+TOC → page_outline; scan → grobid_structural;
  batch_admission pages=0 → skip (native).
- D285: wasmi not wasmtime; onion boundary held.

Green: domain fsm_policy 10, application 4, adapters wasm_policy 3,
clippy -D warnings on da-domain/ports/application/adapters/cli --lib.

### M446 W-A1: словари сделали живыми — кэш инвалидируется, rebuild 6 минут

- **Найдены и закрыты два системных разрыва** (DATA-first нарушался):
  1) Раннеры (sweep, production journey) строили экстрактор из
  **bundled-снимка в бинарнике** — правки data/extraction_patterns.yaml
  не меняли рантайм. Теперь: from_yaml_file за env
  DA_EXTRACTION_PATTERNS (default data/…), честный fallback на bundled.
  2) Stage-контракты несли **статический** configuration_fingerprint —
  смена словаря не инвалидировала кэш extraction. Порт Extractor
  получил cache_fingerprint() (default None), RuleBasedExtractor
  отдаёт sha256 значений конфига, RuleExtractionStage компонует
  `;rules_sha256=` в ключ. Тест: разные словари → разные ключи.
- **methods.phrases +18 фраз** (math-oc алгоритмы, SE-эконометрика,
  robotics diffusion): 3 zero-mention статьи извлекают 2/1/5 сущностей.
- **Полная пересборка: 14с sweep (604, кэш) + 6.3 мин материализация
  (589/589, 0 failed, 0 replay-noop)** — было ~2ч; индексы M443 дали
  ~19x на полном корпусе. Regression-check против предыдущего отчёта:
  0 findings. Drift: **0 zero-mention алармов**, mentions 9.84→11.08.
- Лоадер материализации различает старые/новые extraction-записи по
  живому fingerprint словаря (история не удаляется).
- Новый инструмент extract_probe (не-gold аналог eval_extract).
- **Отложено честно**: gold F1 spot-check заблокирован pre-existing
  аномалией — 19 статей лежат в 2+ категориях каталога
  (байт-идентичные PDF; чинится lookup-унификацией, не удалением).

### M445 pipeline stage audit: все 20 этапов против ADR

- Артефакт doc/review/pipeline-stage-audit-2026-08-22.md: поэтапная
  таблица (ADR/слой ACAP/логика/пробелы), планировщик и FSM-адмиссия,
  мета-промпт готовность + хантинг хардкодов (найден один: OpenAlex
  arXiv-DOI фильтр), инвентарь 29 YAML-словарей + приоритетные
  дополнения (citation_styles, venue_registry, author_identity,
  doi_prefix_map, method_taxonomy), научный контекст (якоря есть;
  DOI/Crossref/цитатные сущности — нет), реестр аномалий.
- Вердикт: линейный хребет acquisition→checkpoint ГОТОВ сквозно на
  корпусе arXiv-каталога; фронтир — мультисорс, словари цитирования,
  постраничный AST, агентный мозг.

### M444 corpus regression harness: регрессии ловятся на всех статьях по пути

- Направление пользователя: работать со ВСЕМИ статьями и ловить регрессию
  инлайн, не золотыми замерами. Gold остаётся только эталоном F1.
- **Regression diff engine** (da-cli/catalog_regression): типизированные
  находки между двумя run-отчётами — PapersRemoved/Added, RevisionDrift,
  StructuralDrop (с относительным допуском DA_REGRESSION_TOLERANCE),
  ReplayHonestyLost, FailedPapers; exit 1 при любой находке. 6 юнит-тестов.
- **corpus_regression_check** example: на идентичных реальных отчётах
  589/589 — 0 findings (exit 0); на тампернутом — поймал все 3 внесённые
  регрессии (удалённая статья, дрейф ревизии 2→3, entities 12→4), exit 1.
- **Drift-статистика по всем статьям** (catalog_drift_stats.json):
  9.84 mentions/статья в среднем; нулевых — ровно 3 (те самые low-yield
  не-CS статьи из бэклога). Семантика исправлена по первому применению:
  entities_created — глобальный dedup-счётчик (0 на статью = пере-
  использование vid, норма), alarm-сигнал — нулевые MENTIONS.
- **Попутно найден и исправлен баг артефактов**: PaperRunRecord писал
  `revision` дважды (дубликат JSON-ключа из flattened FSM-состояния);
  строгие парсеры отклоняли отчёты. Writer исправлен, закоммиченный
  full-run отчёт починен (round-trip dedup, значения идентичны).

### M443 indexed hot path: материализация 50–143x быстрее на полном сторе

- Диагноз: 2-часовой full-run M442 — каждый identity-lookup (Entity vid,
  Paper arxiv_id, Section/Reference/revision vids) без property-index
  линейно сканировал стор, растущий до 66k узлов; replay удваивает.
- Инфраструктура индексов в SamyamaGraphStore уже была (backfill+трекинг,
  протестирована) — composition root их просто не создавал.
- Новый контракт da-application/materialization_indexes: 9 уникальных
  identity-ключей + ensure_materialization_indexes; **Citation.arxiv_id
  сознательно исключён** (дубликаты между статьями: min-id vs scan-order
  семантика) — задокументировано.
- Runner обеспечивает индексы после open/preload. Бенчмарк на полном
  снапшоте (examples/materialize_index_bench): **Entity.vid 50x,
  Paper.arxiv_id 143x** на miss-пробах; сборка 9 индексов по 66k узлов —
  389 мс one-time.
- Регрессия A/B: 30-статейная канарка с индексами — **структурно
  идентичный** граф (3341/6990 узлов/рёбер), 0 replay-noop нарушений.
- Governor-пин logging поймал новый bench-пример до коммита (M440-урок
  работает). 5 новых тестов (2 lib + 3 integration).

### M442 full run: 589 статей каталога → единый семантический граф

- **589/589 completed, 0 failed, 0 replay-noop violations** (wall-clock
  ~2 ч, оффлайн, workers=1). Снапшот catalog_materialized.sgsnap:
  **66299 узлов / 169916 рёбер**; graph-stats disk→query same-process OK.
- Покрытие материализации каталога: **589/604 = 97.5%** (523 TEI + 66
  pdf-stamp recovery + 15 честных no_stamp).
- Сравнение с gold-графом (173 статьи, 21868/51518): плотность на статью
  сопоставима (catalog 112.6/288.5 vs gold 126.4/297.8 узлов/рёбер);
  наборы статей не пересекаются by construction — отчёт фиксирует
  honest non-equivalence (artifacts/catalog_sweep/
  catalog_vs_gold_comparison.json).
- Все 589 статей несут revision-owned identity (v1..v11).

### M442 feature-matrix gate + crash-safe resume материализации

- **Пин feature-matrix**: verify_rust_architecture.sh теперь компилирует
  `da-cli --features pdf-classifier --examples`; Governor-политика
  `feature_matrix_gate` (DATA YAML + Python-чек + 3 юнит-теста)
  флагает исчезновение пина. Урок M441 закрыт двумя слоями.
- **Crash-safe resume** (--resume / DA_MATERIALIZE_RESUME=1): прерванный
  full-run продолжает с места остановки — preload снапшота, skip
  завершённых статей по prior-отчёту, merge записей и totals.
  E2E: 5 → resume → 10 total, граф растёт ровно на новые статьи
  (380→680 узлов), 0 дубликаций, 0 replay-нарушений.
- Хелпер prior_completed_ids (flattened-state парсинг) + 3 юнит-теста
  (17 в модуле). Баг nested-парсинга пойман E2E-канаркой и исправлен.

### M441 version recovery: PDF-штампы закрывают 66/81 no_tei_revision

- Разведка: 9/12 пропущенных PDF несут штамп arXiv:<id>vN на первой
  странице — источник версии именно этого ассета (без сети и без
  угадывания latest).
- `arxiv_stamp_revision` (da-application, чистая логика): таблица
  форматов (vN, суффикс категории, перенос строки, регистр), отказ для
  чужих id/v0/без-версии; RevisionRecoveryFsm (TeiChecked → StampChecked
  → Recovered/StillUnbound, terminal-absorb) + D127-canary. 6 новых
  тестов (всего 15 в модуле).
- Runner: режим `--recover` за feature `pdf-classifier` (pdf-inspector,
  первые 2 страницы, оффлайн): **66 recovered / 15 still_unbound /
  0 pdf errors** по реальным 81; merge в --run через
  DA_MATERIALIZE_RECOVERY (+66 materializable).
- Merged-канарка 30 статей: 30/30 completed, 0 failed, 0 replay-noop;
  восстановленные статьи прошли до Completed со своими версиями
  (1611.01796→v2, 1905.08085→v5, 2005.14165→v4).
- Попутно исправлен pre-existing баг feature-сборки: load_journey звал
  trait-метод RegionTextPort без импорта трейта (ломал
  `cargo check -p da-cli --features pdf-classifier` со времён M422;
  полный гейт эту комбинацию не собирал). Ре-экспорт
  `da_adapters::pdf_inspector` для feature-пути.
- Env: DA_MATERIALIZE_RECOVERY_REPORT, DA_MATERIALIZE_RECOVERY.

### M440 sweep materialization: кэш свипа → семантический граф (S01-S03)

- Coverage-audit 604 ok-статей свипа по ADR-065 tei_declared_revision
  fallback: **523 materializable (86.6%), 81 typed-skip (no_tei_revision)**,
  reconcile без пропусков; артефакт materialization_coverage.json (D283).
- Явный FSM материализации (da-application/catalog_materialization.rs):
  scan→bind→materialize→verify(replay-noop), терминальные состояния
  поглощают события, illegal-переходы типизированы, D127-canary —
  import/promote/unlock событий нет by design; proptest на absorb.
- Runner (examples/catalog_materialize_runner.rs): полный оффлайн-replay
  кэша (parse+extraction+embedding) через materializer-only use cases,
  revision-owned identity, manifest-дайджест из parsed.pdf_hash с
  cross-check против sweep-отчёта (drift → typed Fail), экспорт .sgsnap.
  Env-ручки: DA_MATERIALIZE_{LIMIT,SNAPSHOT,RUN_REPORT} (.env.example).
- Канарка 25 статей: **25/25 completed, 0 failed, 0 replay-noop
  violations**; повторный прогон структурно идентичен (2626 nodes / 5484
  edges; байтовые отличия — только Clock-метки по ADR M330/M398);
  graph-stats по цепочке disk→query one-process OK.
- Тесты: 25 новых (14 da-cli audit+loader, 9 FSM incl. proptest,
  2 integration MockGraphStore); clippy чист.

### M439 catalog-wide sweep: 606 статей обработаны, отклонения типизированы

- Полный обход каталога вне gold-200: **604/606 ok (99.7%), 0 failed**,
  9 типизированных source-аномалий, wall-clock 12.3 мин (workers=4).
- Найден и исправлен продуктовый баг: rule_extractor резал текст по
  байтовым индексам из lowercase-копии — паника на мультибайтных символах
  роняла весь прогон. Char-boundary snapping + red-first регресс-тест с
  мутационной проверкой (panic воспроизводится двойной мутацией).
- lib `da-cli/catalog_sweep.rs`: DATA-пороги, типизированный
  классификатор отклонений, инвентарь; example-runner с resume.
- Отклонения: 3 статьи с 0 entities (не-CS лексика — расширение DATA
  словаря), 9 source-аномалий (8 no_pdf → HTML-путь, 2 инфраструктурные).
- Метрики и рекомендации оптимизации: artifacts/catalog_sweep/
  {sweep_report,aggregate}.json + analysis.md.


### M438 architecture closure: orchestrator defer + drift-gate coverage

- ADR-066 / D280: PipelineOrchestrator stays product-side — engine
  migration (ADR-064 §5 follow-up) closed as deferred-by-design. Recon
  showed the overlap with kg-pipeline's ExecutionEngine is ~30 lines of
  step-walking; the orchestrator owns durable cross-process retry
  (RetryableFailure persisted to cache, RetrySuppressed on next run)
  while the engine owns bounded in-process retry with an injected
  Sleeper — complementary semantics, not duplication. ACAP admission,
  cache identity, GraphMaterialize rerun and the StageRunRecord surface
  must not enter a universal crate.
- docs_drift_check.sh now covers all five doc-rendering commands: the
  three original ones plus `audit-fields` and `schema-check` (the
  ADR-045 Wave E schema drift gate). GRAPH-SCHEMA.md gained a canonical
  "Schema drift gate" section carrying both commands' current output and
  the same-commit update rule. Negative fixture proven: hiding either
  command marker from both docs makes the detector report it. The hook
  stays advisory-only by design (FAST-ONLY pre-commit policy).


### M437 corpus-wide semantic graph rebuild (MEM679)

- Full gold-corpus semantic graph materialized for the first time:
  **173 papers -> artifacts/gold_corpus/corpus_graph.sgsnap**
  (21868 nodes / 51518 edges), sha256-bound AllWavesReport with
  `replay_zero_writes` on every wave and a continuous graph chain —
  the ADR-051 §8.1 disk-to-query contract now proven at corpus scale.
- Process gap fixed (ADR-065): catalog is 100% versionless, so exact
  revision binding blocked the whole corpus. Added honest
  `tei_declared_revision` binding from the GROBID idno already stored in
  the parse cache (`arXiv:{id}v{N}`); fail-closed preserved when the
  document declares no version. 27 papers without any declared version
  are excluded via filtered DATA manifest (no silent denominator loss).
- Canaries: canary verification stage-id drift fixed (report-driven
  `extraction_compute` instead of hardcoded `extract` — red-first test);
  20 embedding-enabled waves over the shared cache root,
  `replay_all_cache_hit` everywhere, import_eligible=false throughout.


### M436 kg-pipeline honest execution engine

- Recon finding: `kg-pipeline::ExecutionEngine` was a fictitious contract —
  the doc-comment promised retry/failure policy/status tracking while the
  loop aborted on first error; `RetryPolicyRef` and `FailurePolicy` were
  never consumed; 4/6 `PipelineStatus` variants were unreachable. The
  product path never noticed because execution lives in
  `PipelineOrchestrator` (ADR-049) and kg-pipeline was used only for
  topology.
- Fixed (ADR-064): per-step retry from a policy registry with an injected
  `Sleeper` port (ADR-051 determinism infra — no real sleeps in tests);
  `FailurePolicy::Abort`/`Continue`/`RetryThenAbort` semantics with
  transitive skip of dependents; `attempts_per_step`/`skipped_steps` in
  `ExecutionResult`; observable FSM — `StepEvent` stream (Started/Retried/
  Failed/Skipped/Completed + StatusChanged Pending→Running→terminal) with
  all six statuses reachable; cooperative `EngineControl` pause/stop
  checked between steps.
- Red-first + mutation-checked: retry success on attempt 3, sleeper call
  count, Continue→abort mutation and pause-check mutation both caught by
  tests. `execute()`/`execute_with_policies()` keep historical single-
  attempt abort behavior (no consumer change; da-application
  acap_control_plane_test green).


### M435 Governor pins the audit gate

- M433/M434 made the audit-fields drift gate structurally correct
  (multi-line `extract_set_fields`, both product surfaces), but it only
  ran under CI's `schema_check_test` — in the dev loop a regression to
  the line-based scan or a single-surface scope would pass every commit
  and silently re-blind the ADR-045 gate.
- Two Governor pins (`audit_field_scanner` requiring `extract_set_fields`;
  `audit_both_surfaces` requiring `src_dirs` and forbidding the restored
  line-by-line `for bline in block.lines()` blind spot) now enforce it at
  every commit in ~1s with no build — the first line of defense.
- The pin construction itself exposed the scanner's doc-comment
  false-positive class once more: a naive `forbidden: line-by-line`
  tripped on the module doc's own description. The forbidden pattern is
  the exact regressed code shape, not the English phrase. D127 locked.

### M434 Audit both surfaces and declare sha fields

- The audit-fields drift gate scanned only da-application/src — blind across
  the crate boundary. da-cli load_journey stamps `embedding_sha256` on Paper
  (the first-attach path beside add_vector) and the shared embedding_write
  helper stamps it on PaperRevision (the revision content node); neither
  schema declared it, and M432 declared it only on Entity. The extended
  audit (90 files across da-application + da-cli) immediately reported the
  Paper drift — red run closed by declarations on both schemas.
- The scanner also learned to ignore its own doc-comment examples (a
  docstring `create_node("Label")` / `set_node_property_*(…, "field", …)`
  was being reported as drift). Binary audit now reports OK scanning both
  surfaces; schema unit tests pin the two new declarations. D127 locked.

### M433 Multi-line field scanner

- The audit-fields drift gate was line-based: cargo fmt routinely splits
  `set_node_property_*` calls across lines, and the scanner missed exactly
  those fields (M432: 2 of 4 drifted fields invisible — a blind gate is a
  fake gate). The extraction now lives in a pure tested lib function
  `da_cli::audit_fields::extract_set_fields` scanning the block from each
  occurrence to the matching closing paren.
- The fixed scanner immediately exposed its own false-positive class: a
  dynamic property loop (field passed as a variable) has no string literal
  and an unbounded scan misattributed the next literal —
  `create_edge(…, "hasSummary")` was reported as a Summary property.
  Paren-bounding closes it: the field must be a string literal inside the
  call's argument list. Five fixtures (single-line, multi-line, chained,
  no-string, dynamic-loop regression); binary audit OK. D127 locked.

### M432 CI matches the swept test surface; schema drift closed

- CI listed a dead `-p da-graph` (clippy fails with unmatched package ID
  on a fresh runner), omitted da-mcp entirely (46 real-engine tests never
  ran in CI), and skipped da-cli/da-adapters integration suites incl.
  the ADR-061 cross-process contract. Fixed and synced with the verify
  script. Running the suites locally exposed real schema drift: 4 fields
  written by M418-M419 but never declared (Paper triage trio, Entity
  embedding_sha256) — declared, audit-fields OK. The figure-journey test
  pins DA_FIGURE_QUOTE_POLICY absolutely. D127 locked.

### M431 Honest direct property write on the adapter
## 2026-08-25 — toolchain 1.94.1 → 1.98.0 (Rust 2025–2026 wave study)

Изучена сводка компиляторных изменений 2025–2026; применено:

- **rustc 1.94.1 → 1.98.0** (rustup update stable): v0 symbol mangling
  на stable (читаемые backtrace с generic-типами — проверено живым
  тестом), 4 минорных релиза компиляторного перфа (~−5% wall по
  nnethercote survey), LLVM-апгрейды.
- **line-tables-only**: подтверждён уже установленным (profile.dev
  debug = 1) — изменений не требуется.
- **LLD**: rust-lld присутствует в sysroot 1.98; дефолт линковки
  x86_64-linux проверен рабочей сборкой; отдельный override не внесён.
- **НЕ принято** (nightly-only, продакшен-дисциплина): next-gen trait
  solver (nightly с 2026-08-21), Polonius Alpha (2026-08-04),
  -Z annotate-moves. Переоценить после стабилизации; trait-solver
  интересен для TAIT в гексагональных трейтах.

Верификация на 1.98: domain 705 ✓ adapters 246 ✓ cli 176 ✓,
clippy -D warnings clean, RocksDB system-link работает, sccache
перепрогрет (однократно). cargo check -p da-domain 40s → 22s после
прогрева.

## 2026-08-25 — ADR-073 citracer trio in Rust (upstream resilience)

Deep vendor-source research wave 2 (13 repos via codebase-memory; ADR-073).
Implemented for OpenAlex — all in Rust per product direction:

- `da source fetch-openalex <ids|pending> [--no-purge]`:
  negative-cache purge (stale 404/422 self-heal; citation-salvage makes
  old negatives wrong) → bounded-concurrent path-form fetch (8 parallel;
  probe proved doi-filter pipe-OR is HTTP 400 — batch rides concurrency,
  not unsupported API syntax) → per-pid slim cache files (enrich-layer
  consumes unchanged).
- CircuitBreaker in da-adapters (threshold 5, cooldown 120s, success
  resets) — complements action-level retry from the audit wave.
- Toolchain registry: enriched = [fetch-openalex, enrich-layer]; python
  pre-hook removed from the runtime path.

Live: 560×200 (+1 recovered), 85 honest 404 (textbook chapters with
arXiv-shaped DOIs + not-yet-indexed young papers — purge retries them
on every run). Clippy 1.98 new lint fixed in load.rs (as_chunks).

## 2026-08-25 — python-crutch purge wave (runtime path is Rust-only)

Audit of ALL python-in-runtime-path crutches (m451_enrich_openalex.py class).
Classification + action:

REPLACED BY RUST:
- m451_doi_import.py (hand-tuned one-paper DOI mirror) → `da source acquire
  <doi>` handler (route doi_mirror → OpenAlex oa_location → PDF download →
  identity.json). Live: routes + finds PDF; ACM returns honest 403 paywall.
- m451_background_runner.py (per-section LLM summaries) → `da source
  background-run [task|pid]` (da-cli/background_tasks.rs): registry-driven
  policy (background_tasks.yaml), injection scrub (da-domain), models.yaml
  binding, batches ≤16, fingerprint cache, rate limit + daily budget,
  honest failed markers, idempotent rerun ({"cache":1} on retry). LLM
  output parser handles the three 9router shapes: array, json_object
  wrapper, concatenated per-section object stream.

DEAD PORTS REMOVED (functionality already in Rust):
- m451_navigation_build.py (ported: da-domain/navigation.rs)
- m451_summaries_build.py (ported: build-summary action)
- scripts/tests/test_navigation.py (covered by Rust tests)

KEPT (sanctioned patterns):
- m451_pdf_observations.py / m451_pdf_toc_observations.py — reference
  extractors (PyMuPDF observations; Rust stages consume; ADR phase-2)
- m451_textbook_import.py / m451_monograph_import.py — one-time manual
  acquisition tools (4 books imported; Rust textbook-acquire is backlog)
- m451_migrate_article_manifests.py / m451_reference_layers.py /
  m451_summary_eval.py — one-time migration/pilot/eval session tools

Registry honest: doi_mirror_fetch impl=rust, per_section_lazy impl=rust.

## 2026-08-25 — book boundary-tail recovery + margin captions (manual audit)

Manual page-by-page comparison of extracted algorithm sections vs the
original PDF (user: «срафнить с оригиналом — что теряем») surfaced two
loss classes:

1. BOUNDARY TAILS (103/267 section boundaries, CRITICAL): content on the
   next section's first page ABOVE its heading (continuation paragraphs,
   equations — e.g. the core Q-learning update eq 17.10 with ←) was
   dropped by both ranges: anchor-splitting removed it from the next
   section, and page_end=next_page-1 excluded it from the previous one.
   The earlier anchor fix had silently converted misattribution into
   DELETION. Fix: build_pdf_toc_sections reassigns the shared-page
   prefix (up to the next heading anchor, fail-open) to the previous
   section. Live: lost boundaries 103 → 22 (residual = anchors not
   found on imprecise TOC pages); Q-Learning 1449→2454 chars with eq
   17.10 + ← recovered.

2. MARGIN CAPTIONS (173 Algorithm N.N blocks): Tufte-layout algorithm
   captions/footnotes live in the margin zone and never reached parsed
   sections. Now attached as a structured `margin_notes` field per
   section (pages range joined) — 146 sections carry Algorithm captions;
   body reading order stays clean (never merged).

Tests: boundary_tail_reassigned_to_previous_section (eq 17.10 fixture),
margin_notes_attached_structured_not_merged. Clippy 1.98 new lints
fixed en route (Option::zip in ingest.rs).

## 2026-08-25 — ADR-074 WASM MicroLoRA policy modules + roadmap update

ADR-074 proposed: WASM MicroLoRA as swappable FSM policy modules.
Three-layer architecture: FSM core (Rust) → decision points (trait
objects) → policy artifacts (.wasm, sha256-versioned). Five decision
points: grammar_route, batch_admission, concurrency_tune,
retry_policy, summary_depth. Staged W1→W4 behind D136.

Requirements registered: R097-R104 (evidence gate, multi-domain
routing, survey spine, boundary-tail recovery, margin notes,
negative-cache purge, circuit breaker, WASM policy modules).

ROADMAP: M451 (current) + M452-M457 future milestones added:
M452 SONA ReasoningBank extraction, M453 WASM hot-swap infra,
M454 figure-repair loop, M455 temporal query filters,
M456 RRF reranker composition, M457 content_ref vector layer.

continue.md updated with full session state and pending backlog.

## 2026-08-25 — Wave 1: citation materialization + domain fix + manifests

Technical debt wave 1 (audit items D1+D2+D6):

1. D2 DOMAIN FIX: trench-real-analysis identity gained
   coarse_topic_code=math-ho → routing now resolves to [mathematics]
   (was computer_science via default fallback). ads-discrete-structures
   already had math-discrete. algorithms-for-decision-making stays cs-ai.

2. D1 CITATION SALVAGE MATERIALIZED: re-extracted 28 articles that
   carry GROBID-mangled citation IDs. The try_parse_arxiv_locator
   salvage (committed earlier) now has data flowing through it —
   ~1000 citation edges recovered on next graph wave.

3. D6 DOMAIN MANIFEST REFRESH: biology domain gained 3 papers (was 0
   — stale manifest from before multi-domain routing landed; the
   q-bio articles' routing entries existed but no build-extracted
   had run on a biology article since). All five domains now show
   non-zero processed counts.

4. FSM boundary-agent type unification: find_heading_anchor now
   returns Option<(usize, AnchorVerdict)> — Accepted vs Rejected
   verdict alongside the offset. Callers destructure. Dead code
   (unused number-line swallow after FSM classify) removed.
   AnchorVerdict enum lives in pdf_toc (not boundary_agent).

Green: domain 713, adapters 249, cli 176, pytest 142,
validate 645x0, clippy clean.

## 2026-08-25 — Wave 2: figure-repair loop (D3)

D3 from audit: 4 articles had terminal figure_analysis failures
(schema violations, JSON parse, checksum mismatch). The critic
repair contract covered enriched/citations/summaries but NOT
figures — asymmetric gap.

Fix: map_error now classifies LLM output errors as Retryable
(not Terminal): StructuredOutput (schema violation → truncated
quote retry), Json (parse → simplified prompt retry),
ImageContextMismatch (checksum → rebuilt context retry).
Critic manifest reject_triggers gains figure_analysis →
reanalyze_with_diagnostics.

TDD: test written first (red), then map_error extended (green).
The pipeline's existing retry infrastructure (StageExecutionError::
Retryable → RetrySuppressed on cache) handles the rest — no new
pipeline code needed, just honest error classification.

Next graph wave: the 4 failed articles will retry automatically
when their cache entries expire.

## 2026-08-25 — Wave 3: audit-fidelity false-positive fix + .env completeness

1. AUDIT-FIDELITY GATE FIX: 42 boundary-tail "losses" were false
   positives from the Rust gate. The FSM boundary-agent returns
   Rejected verdict for ambiguous anchors (table cells like "Hex
   world" inside Table F.1) — the split doesn't happen, so the tail
   content stays in BOTH sections. The gate was checking only the
   current section for the tail probe → 42 false "lost" tails.
   Python probe (without FSM) confirmed 0 actual losses.

   Fix: the gate now checks BOTH the current AND next section
   (Rejected = content in both). Result: 231/231 recovered ✓.

   Also fixed: accidental `continue` inside a nested `if let` was
   skipping coverage and margin checks (305→117 sections).

2. ENV COMPLETENESS: 9 env vars referenced in Rust code but missing
   from .env.example: DA_DOI_PREFIX_MAP, DA_EXTRACTION_PATTERNS,
   DA_FIGURE_MODEL_BINDING, DA_FIGURE_QUOTE_POLICY, DA_GRAPH_DATA_PATH,
   DA_MULTIMODAL_CONTEXT_SCHEMA, DA_REGRESSION_TOLERANCE,
   NINE_ROUTER_API_KEY, OPENALEX_BASE_URL. All added with defaults.

Green: domain 713, cli 176, application 337, pytest 142,
validate 645x0, clippy clean. Audit-fidelity: 231/231 tails,
91.4% coverage (NEEDS-ATTENTION from <98% threshold — honest,
glossary/table sections are the worst performers).

## 2026-08-25 — Wave 4: Governor test_fidelity_drift + raw backfill + registry honesty

1. GOVERNOR DIMENSION: test_fidelity_drift — ловит класс регрессий
   «код зелёный, но гейт врёт» (wolves 1-3 lessons: 42 false positives,
   coverage drop 305→117, boundary deletion — ни один юнит-тест не
   поймал). YAML contract: watched_sources, fixture_requirements
   (trench: 48-52 sections, 100% tails, ≥85% coverage), 4 checks
   (audit-fidelity pass, sections stable, no-python-in-runtime,
   domain manifests fresh).

2. D4 LEGACY BACKFILL: 603 manifests gained synthetic raw events
   (actor: legacy-implication-backfill, note: parsed proves raw).
   FSM sequences now complete — no gaps in lifecycle_events.

3. D5 TEXTBOOK PLACEMENT: registry impl → rust. Manual workflow
   documented (identity.json + PDF → da source acquire → run).
   Zero reference tools remain in the FSM runtime path.

Governor pre-existing BLOCKED (1764 violations): reverse_adr_audit
(41 subdir patterns + GSD planning projections) + target bloat
(42GB) + graph section stubs — all pre-existing, not from waves 1-4.

## 2026-08-25 — Wave 5: Governor schema fixes + hardcoded URL + textbook manifest

Governor BLOCKED causes addressed:

1. SCHEMA.YAML COMPLETENESS (41→0 subdir violations):
   - data/processed: subdir_pattern += trench-*, ads-*, gnn-ch-*,
     algorithms-* (textbooks don't have dots in names)
   - data/: valid_subdirs += agent_memory, agents, domains, prompts
   - data/sources: valid += acm, doi
   - data/sources/arxiv: valid += article_catalog
   - doc/: valid += research

2. HARDCODED URL: revision_query.rs 'https://example/...' →
   DA_REVISION_ASSET_BASE_URL env var (added to .env.example)

3. TEXTBOOK MANIFEST: algorithms-for-decision-making/textbook_manifest.json
   created (format, parsing approach, TOC bookmarks, grammar profile)

Remaining Governor BLOCKED causes (graph materialization backlog, NOT
from waves 1-5):
- graph_section_text_incomplete: avg 364 chars <500 (M439/M440 backlog)
- graph_enrichment_missing: 0% coverage (enrichment wave needed)
- graph_revision_binding_low: 0/295 papers (revision binding wave needed)
- reverse_adr_audit: 1764 violations ≈ 295 stubs + 885 enrichment +
  295 revision (sum of the three graph gaps above)
- resource_target_bloat: 42GB target/ (cargo clean advisory)
- gsd_planning_projection: 4 old milestones without PLAN tasks

These are structural corpus work items (graph materialization),
not code bugs. They map to existing roadmap M439/M440 backlog.

Green: domain 713, cli 176, app 337, pytest 142, validate 645x0.

## 2026-08-25 — Wave 5 continued: Governor planning projection fixed (6→4 flags)

GSD planning projection flags eliminated:
- STATE.md Active Milestone → M451 (was stale M446)
- M446-M450 marked complete in ROADMAP/STATE/QUEUE
- .gsd/phases/451/ created with S01-PLAN.md (12 tasks in Governor
  _TASK_ROW format: - [x] **TNN**: description)
- PLAN file placed at phase_dir root level (not nested S01/) —
  Governor's glob("*-PLAN.md") is non-recursive

Governor flags: 6→4. Remaining 4 = graph materialization backlog:
- reverse_adr_audit_violations [high]: 1764 (sum of below)
- graph_section_text_incomplete [high]: avg 364 chars <500 threshold
- graph_enrichment_missing [high]: 0% Author/Venue/Topic coverage
- graph_revision_binding_low [high]: 0/295 revision bindings

These are structural corpus work items (M439/M440 backlog), not code bugs.

## 2026-08-25 — Wave 6: pipeline completion + char-boundary fix

1. CHAR-BOUNDARY FIX: enriched_authors.rs f[..1] panicked on
   math italic glyphs (𝒚 = 4 bytes). Fixed with chars().next().
   This blocked enrich-layer for 4 articles (2504.16084 etc).

2. PIPELINE COMPLETION: run-batch on all remaining incomplete
   articles. Result: 0 articles without enriched (was 7),
   0 without summarized, 0 without extracted.

3. Governor: target_bloat flag eliminated (40GB → 13GB after
   incremental cleanup + cargo clean -p stale artifacts).
   resource_target_bloat no longer appears in flags.

Green: domain 713, cli 176, app 337, adapters 249, pytest 142,
validate 645x0, clippy clean.

## 2026-08-25 — M452-mhx138 S01: SONA bridge + sona-analyze CLI

SONA ReasoningBank bridge implemented in Rust:
- features_to_embedding(): CVF trajectory JSON → SONA embedding vector
  [grounding, latency_norm, scrubbed, tokens_norm, inverse_quality]
- build_reasoning_bank(): K-means++ clustering via ruvector-sona
  PatternConfig {max_trajectories: 10000, quality_threshold: 0.3}
- summarize_patterns(): human-readable output

CLI: da source sona-analyze <agent_id>
Live proof on summarizer agent: 55 trajectories → 7 patterns extracted
(avg_quality range 0.50–0.90). Two unit tests pass.

This is ADR-074 Phase W2 Step 1: pattern extraction without LoRA
training. D136 gate still applies to actual weight adaptation.
