# Аудит абсорбции GROBID в Rust

Исходная ревизия: `bcdd0cfc5e34f64821798463fe23b2f3845efe56`, ветка `feat/da-parse`. Дата проверки окружения: 2026-09-21. Исправления runtime не выполнялись; этот документ описывает измерения и следующий объём работ, а не разрешение на выпуск.

## Вывод

Rust-реализация существенно продвинулась, но полнота перевода Java и семантическая эквивалентность не доказаны. Закрытие milestone, целостность реестра и зелёные тесты — три разных свойства. Ни одно не заменяет проверки выходного TEI. Формулировки предыдущего отчёта «плоскость доказательств завершена» и «миграция выполнена на 0%» отозваны как необоснованные.

| Измерение | Факт | Ограничение |
|---|---|---|
| Capability ledger | 38: proxy 7, shadow 12, candidate 2, fallback 1, absent 16 | Смешивает REST, стадии и learning; не является знаменателем процента перевода Java |
| Native Rust | 32 native-модуля, 25 452 строки из 43 671 строк da-parse по исходному замеру | LOC не измеряет полноту поведения |
| Production | В реестре нет строк production; Java остаётся authority/fallback | Это статус допуска, не нулевой объём реализации |
| Horizon | 12 complete / 1 in_progress (H05) | Governor проверяет целостность записей, не повторяет все семантические проверки |
| GAR | 14 resolved / 1 superseded / 1 open (GAR-015) | Новые находки настоящего аудита не входят в исторические 16 записей |
| Лицензии | 38 непустых ссылок license; в license_inventory 22 компонента: 7 verified / 15 blocked | Наличие ссылки не означает разрешения на распространение; ограничения inventory сохраняются |
| Absent scope | 7 supporting / 5 not_compatibility_requirement / 4 deferred | Нельзя объявлять все 16 дефектами обязательного продукта |

## Планировали, сделали, получили

| Контур | План | Реализация и результат | Остаток и причина |
|---|---|---|---|
| HTTP и TEI | Стабильный ParseBackend/ParserPort и сохранение консервации | Java HTTP, TEI envelope/lift, lineage и profile identity | Это обёртка и инфраструктура, не перевод Java-алгоритмов |
| Feature ABI и CRF | Native feature/sequence-family pipeline | Реализованы Rust ABI, scalar CRF и семейства с contract/oracle/same-n тестами | Нужен проверяемый Java method → Rust symbol → независимый oracle реестр; общего процента parity нет |
| Header/reference composition | Независимые native полосы с Java fallback | Композитор существует и проходит штатные тесты | Воспроизведены повреждение XML и нарушение ссылочной целостности; синтетических фикстур оказалось недостаточно |
| Body/visual | Консервация текста, порядка и структур | Есть shadow-family модули, manifests и ограничения допуска | Потенциальная потеря типизированных причин absence; требуется отдельный runtime regression |
| Ресурсы | Global budgets и bounded reorder | Есть process-local ограничения, publication window | H05 явно не закрыт; это не отсутствие всех budget-механизмов |
| Внешняя оценка | Независимый frozen probe и release decision | Есть пятистатейный пилот и package contract | Требуемый isolated probe not_executed, reviewer=null, decision=pending; пилот не заменяет этот допуск |
| Governor | Исполнимая проверка правил и честность статусов | 368 тестов проходят; оба исторических review-реестра валидируются | Это не доказательство корректности XML или качества парсинга |

## Подтверждённые дефекты

Серьёзность ниже относится к shadow-композитору. Доказательств воздействия на боевой Java-маршрут нет.

| ID | Приоритет | Дефект и место | Доказательство | Почему текущие тесты пропускают |
|---|---|---|---|---|
| A-XML | P0 | `native_fulltext_composition.rs:402–434,602–613`: сохранение диапазонов по имени смешивает открывающие/закрывающие теги, создаёт пересечения; output не перепарсивается | Настоящий Rust `compose_native_fulltext` возвращает Accepted для трёх валидных сохранённых TEI; независимый ElementTree отвергает все три результата | Проверяется наличие строк в простом синтетическом заголовке, не well-formedness реального результата |
| A-REF | P0 | `native_fulltext_composition.rs:493–520` (ревизия `bcdd0cfc5`): проверяются только переданные anchors, а не фактические body targets; emit меняет id на bibl-ordinal. Текущее состояние — раздел S02 ниже | При исходном body target=#b0 и пустых supplied anchors Rust возвращает Accepted; b0 отсутствует в выходном множестве xml:id | Есть тест чужого переданного anchor, но нет проверки полноты anchors относительно исходного body |
| A-FIELDS | P1 | `native_fulltext_composition.rs:460–520` (ревизия `bcdd0cfc5`): header emitter читает authors/date, references emitter только source. Текущее состояние и снятие RED/GREEN — раздел S03 ниже | Статическое чтение: структурированные header fields, author affiliations и citation fields не сериализуются этими функциями | Сохранение строки не проверяло семантическую эквивалентность title/email/id/affiliation/citation roles |

Статус после корректирующей волны: A-XML закрыт на замеренной области — S01/T02
починил эмиттер (RED→GREEN), S01/T03 закрыл найденный остаточный
namespace-случай, S01/T04 развёл WF-проход и сканер и сделал нелегальную
эмиссию типизированной, S01/T05 консервирует неэлементных детей и вложенные
обёртки, S01/T06 закрыл два остаточных класса (правописание `xml:id` и
`<!DOCTYPE`-подстрока в комментарии/CDATA) и запинил лимиты 4 MiB/128 как уже
корректные. Подробности и актуальные счётчики — в разделе T04–T06 ниже. H14 при
этом НЕ закрывается: planning-запись с `implementation_pass=false` сохраняется.
A-REF закрыт на замеренной области S02 — композитор публикует исходные
published keys и отказывает `Accepted`, если обязательная локальная ссылка не
резолвится в id-пространстве эмиссии; измерение и снятие RED — в разделе S02
ниже. A-FIELDS закрыт на замеренной области S03 — admitted title/email/ids,
author→affiliation и citation role/value сериализуются, а независимый
post-serialization оракул и production-гейт отказывают типизированно при потере;
измерение и RED→GREEN — в разделе S03 ниже. H16 при этом НЕ помечается PASS:
planning-запись сохраняет `implementation_pass=false`.

Воспроизведение: `doc/review/grobid-composition-repro.rs`. Оно использует те же синтетические admissions/candidates, что штатные контракты, но реальный Java TEI как документ. Это тест публичного композитора, НЕ end-to-end доказательство достижимости через production admission.

```bash
cargo build -p da-parse --lib
LIB=$(ls -t target/debug/deps/libda_parse-*.rlib | head -1)
rustc --edition 2021 doc/review/grobid-composition-repro.rs \
  -L dependency=target/debug/deps --extern da_parse="$LIB" -o /tmp/grobid_composition_repro
/tmp/grobid_composition_repro
python3 - <<'PY'
import glob, xml.etree.ElementTree as E
for p in glob.glob('/tmp/audit-*.tei.xml'):
    try:
        E.parse(p)
        print(p, 'VALID')
    except E.ParseError as error:
        print(p, 'INVALID', error)
r = E.parse('/tmp/audit-dangling.xml')
ids = {n.get('{http://www.w3.org/XML/1998/namespace}id') for n in r.iter()}
print('dangling', {n.get('target')[1:] for n in r.iter('ref')} - ids)
PY
```

Это диагностический probe: exit=0 означает, что измерение выполнено, НЕ что SUT корректен. Измеренный результат — INVALID для hub_2507.19457, reca_2605.26525v1, rlm_2512.24601 и dangling={'b0'}.

### Регрессия M511-j7wz2f/S01/T01 (измерение, не исправление)

Тот же замер теперь исполняется в тестовой поверхности библиотеки, без rustc вне cargo:
`crates/da-parse/tests/native_fulltext_composition_contract.rs`.

- Независимый парсер — Rust `roxmltree` 0.21.1, добавленный **только** в
  `[dev-dependencies]` da-parse (квар­антин import-root продакшена `{crate, serde, sha2, std}`
  не изменён; `Cargo.lock` правился точечно: один пакет).
- Каждый прогон читает три сохранённых TEI из `tests/fixtures/preserved/` и
  проверяет их sha256 по `provenance.yaml` до измерения (fail-visible при дрейфе).
- Дефолтный прогон зелёный и пинит замеренный RED
  (`a_xml_red_preserved_tei_emissions_are_rejected_by_an_independent_parser`):
  `Accepted` на всех трёх, `</title></title>` ровно один раз, рост числа
  закрывающих `</title>`, потеря `xml:lang` у синтезированного `<teiHeader>`, и
  точный диагностический текст `expected 'titleStmt' tag, not 'title' at 6:<col>`.
- Целевой гейт `preserved_tei_composition_is_independently_well_formed` остаётся
  `#[ignore]` и падает при `--ignored` (rc=101): дефект RED, починка — S01/T02.

Доказательства gsd_exec: `6a10f8d4-24ac-4c84-b6b0-afb3defd27f8` (три фикстуры,
диагностика независимого парсера), `33c46aa7-0fcf-4421-9223-2e851556cee2`
(дефолтный прогон rc=0 и ignored-gate rc=101). Замки не менялись: D127/D321
закрыты, H14 остаётся незакрытым.

### Регрессия M511-j7wz2f/S01/T02 (исправление: RED→GREEN)

Эмиттер исправлен структурно, а не косметически:

- `XmlDocument` спаривает открывающие и закрывающие теги одним стеком, поэтому
  диапазон всегда `(start открывающего тега, конец закрывающего)` и не может
  начинаться с `</…>`;
- содержимое заголовка переразмещается по родителю: дети исходного `titleStmt` —
  внутрь синтезированного `titleStmt`; дети исходного `fileDesc` без `titleStmt` —
  в синтезированный `fileDesc`; остальные дети `teiHeader` — после него; три
  множества попарно непересекающиеся и вместе покрывают каждого потомка исходного
  `teiHeader`, кроме двух синтезируемых обёрток;
- атрибуты исходного `<teiHeader>` воспроизводятся дословно;
- эмиссия перечитывается структурным валидатором до `Accepted`
  (`ComposedOutputNotWellFormed`), иначе возвращается байт-идентичный
  Java-envelope.

Гейт `preserved_tei_composition_is_independently_well_formed` больше не
`#[ignore]`: три сохранённых TEI (перепроверяемые по sha256 при каждом прогоне)
проходят независимый `roxmltree`. RED-пин T01 заменён независимой консервацией
(`preserved_tei_emissions_conserve_every_source_header_subtree`).

### Остаточный namespace-дефект, найденный и закрытый в M511-j7wz2f/S01/T03

Обязательная для T03 проверка namespace-негативов воспроизвела ещё один случай
того же класса: объявление `xmlns:ext` на **синтезируемой** обёртке `fileDesc`
или `titleStmt` не воспроизводилось, поэтому композитор возвращал `Accepted` при
эмиссии, которую независимый парсер отвергал —
`an unknown namespace prefix 'ext' at 1:202` (объявление на `fileDesc`) и
`1:153` (на `titleStmt`); gsd_exec
`c9cbe8d7-b4dc-492a-aa7d-ab7134a5335b`. Дефект закрыт узко: с этих двух обёрток
переносятся только `xmlns`/`xmlns:*`. Прочие их атрибуты (например собственный
`xml:lang`) по-прежнему не копируются — это семантический остаток поля (H16), а
не well-formedness. Регрессия:
`wrapper_namespace_declarations_are_carried_so_no_prefix_is_unbound`.

### Свидетельства среза M511-j7wz2f/S01/T03 (интеграция и границы)

| Проверка | Свежий результат |
|---|---|
| `cargo test -p da-parse --tests` | 100 suites, 1359 passed, 0 failed, 0 ignored |
| `cargo test -p da-parse --test native_fulltext_composition_contract` | 17 passed, 0 ignored |
| `cargo clippy -p da-parse --all-targets --no-deps -- -D warnings` | exit=0 |
| `cargo fmt -p da-parse -p da-cli -- --check` | exit=0 |
| shadow-пример | план-команда без фичи даёт rc=101 (`target … requires the features: pdf-layout`); с `--features pdf-layout` check exit=0 и реальный прогон exit=0 (`replay.semantic_equal=true`, `conservation.no_silent_loss=true`, 9 terminal из 9 admitted, три лейна) |
| `governor review --registry doc/review/grobid-absorption-roadmap-2026-09-21.yaml` | exit=0 |

Счётчики этой таблицы — снимок ревизии S01/T03 (`native_fulltext_composition_contract`
17 passed, `da-parse --tests` 1359 passed). Они не переписываются; актуальный
замер — в разделе T04–T06 ниже.

Негативы проверены явно: malformed (усечённый документ, лишний закрывающий тег,
незавершённый комментарий, незавершённая CDATA), внешняя сущность
(`<!DOCTYPE … <!ENTITY …>`), дублированный `xml:id` во входе и в эмиссии,
namespace (объявление на `teiHeader`, внутри переносимого поддерева и на
синтезируемых обёртках). H14 не закрывается этой записью: `implementation_pass=false`
сохраняется, а закрытие этапа — решение вне planning-реестра. Замки не менялись:
`deployed=false`, `import_eligible=false`, `auto_promote=forbidden`,
`graph_write=forbidden`, `this_wave_promotes=none`, D127/D321 закрыты. H14 не
означает полной миграции: три сохранённых TEI и shadow-пример не доказывают
production-достижимость.

### Остаточные классы M511-j7wz2f/S01/T04–T06 (актуальный GREEN)

Таблица T03 выше — снимок той ревизии; она не переписывается. Ниже — что
изменилось дальше и что измерено сейчас.

- **S01/T04 (WF-проход отдельно от сканера).** `validate_well_formed` стал
  отдельным проходом по конкретным WFC-классам (ровно один корневой элемент;
  легальность `Name`/`QName`; список атрибутов в форме `Name S? Eq S? AttValue`
  с кавычками и без дублей; ссылки `&`/`&#…;`; `]]>` в CharData; `--` в
  комментарии; CDATA только внутри документа; лимит глубины; легальные
  XML-символы; привязка префиксов), а инвариант «`Accepted` ⇒ эмиссия
  разбирается независимым парсером» закреплён property-тестом. Исторический RED
  этого класса — потеря объявления `xmlns:ext`, записанного как
  `xmlns:ext = "urn:x"` — снят в T04.
- **S01/T05 (структурная консервация).** Обёртки консервируются байтовыми
  интервалами (`content_segments` по `open_end..close_start` минус исключённые
  ПРЯМЫЕ дети), поэтому комментарии, processing instruction и текст/whitespace
  между детьми не теряются, а поиск обёртки ограничен прямыми детьми — вложенный
  `titleStmt` больше не дублируется. Три новые регрессии RED→GREEN.
- **S01/T06 (два измеренных остатка закрыты здесь, лимиты — запиннены).**
  1. `xml:id` с одиночными кавычками или с пробелами вокруг `=` обходил
     uniqueness-гейт: сканер резал открывающий тег по некавычечному whitespace и
     узнавал только компактное `xml:id="…"`, поэтому эмиссия публиковалась как
     `Accepted` с двумя `xml:id="author-0"` (gsd_exec
     `b9a3be53-e685-41c7-ba48-f66674baad41`: `single quotes: an emission with a
     duplicate xml:id must not be published: …<author xml:id="author-0">…<note
     xml:id='author-0'>…`). Закрыто: атрибуты читаются XML-грамматикой
     (`parse_attributes`), нечитаемый список — `MalformedXml`, то есть fail-closed.
  2. `<!DOCTYPE`/`<!ENTITY` искались подстрокой по всему документу, поэтому
     сохранённый TEI, который лишь упоминает строку в комментарии или CDATA,
     отвергался как `ForbiddenExternalEntity` (там же: `Rejected { … error:
     ForbiddenExternalEntity }`). Закрыто: декларация распознаётся там, где
     начинается разметка. Гейт не ослаблен — реальный `<!DOCTYPE` остаётся
     `ForbiddenExternalEntity`, `<!doctype` отклоняется как нелегальная разметка,
     а `<!DOCTYPE` внутри значения атрибута отклоняется по правилу `AttValue`.
  3. Лимиты 4 MiB (фрагмент) и 128 (глубина XML) замерены как **уже корректные**:
     отказ типизирован (`Oversized { component: "native" }` и `MalformedXml`),
     возвращается байт-идентичный Java-envelope, частичного вывода нет. Это
     pinned regression по замеру, а не «исправление»: дефект не воспроизведён.

| Проверка (актуальная ревизия) | Результат |
|---|---|
| `cargo test -p da-parse --test native_fulltext_composition_contract` | 28 passed, 0 failed, 0 ignored (было 24 до T06) |
| `cargo test -p da-parse --test native_fulltext_composition_properties` | 8 passed, 0 failed (было 7) |
| `cargo test -p da-parse --tests` | 100 suites, 1374 passed, 0 failed, 0 ignored (было 1369) |
| `cargo clippy -p da-parse --all-targets --no-deps -- -D warnings` | exit=0 |
| `cargo fmt -p da-parse -- --check` | exit=0 |
| `governor review --registry doc/review/grobid-absorption-roadmap-2026-09-21.yaml` | exit=0 (stages=8 unresolved=8 claims=0) |

RED→GREEN T06 измерен двумя отдельными прогонами до правки — `25 passed;
3 failed` (gsd_exec `b9a3be53-e685-41c7-ba48-f66674baad41`) и `7 passed;
1 failed` (`1646c34e-5ac1-4476-8254-9bd62c5a41b2`) — и двумя после неё:
`28 passed` (`340ff4f1-c4d7-4255-879b-2dc2b9e005a5`) и `8 passed`
(`5f5e83ef-881d-434f-b366-3b6363d0119f`). Полный набор —
`19803fcd-9b09-4b7e-ae0f-a1d4eda0ed60`, clippy —
`2d7bdcc9-dc10-47e9-a154-0cf743c8cc05`, fmt —
`da2c8832-5a9f-4c95-8377-0265625dcf98`. Замки не менялись.

### Свидетельства среза M511-j7wz2f/S02 (целостность ссылок: исторический RED и текущий GREEN)

A-REF закрыт на замеренной области, и это измерено независимым оракулом на тех
же трёх sha256-пиннутых сохранённых TEI (`hub_2507.19457` `98546123…`,
`rlm_2512.24601` `e6aa41f5…`, `reca_2605.26525v1` `dac18882…`), а не пересказом.

Оракул независим от тестируемого кода: только `roxmltree` (**dev-dependency**, в
`src/` недостижим) читает исходный XML; source published keys считаются как
прямые дети уникального `listBibl` (declared `xml:id`, иначе позиционный
`b{ordinal}`), обязательные targets — из `ref type="bibr"` с `target="#…"` по
XML-грамматике атрибутов. Каждый прогон сверяет sha256 фикстур до измерения,
поэтому знаменатель не может тихо измениться.

- **Исторический RED (S02/T01, до исправления).** `references_xml` эмитировал
  `<listBibl>` с `xml:id="bibl-{ordinal}"`, тогда как сохранённое тело
  ссылалось на исходные ключи `b0..`; гард резолва был ограничен
  `candidate.anchors`, поэтому при пустом списке anchors вход принимался
  (`Accepted`), а висячие ссылки оставались. Измерено: 207 записей (hub 80 /
  rlm 49 / reca 78), 217 вхождений `ref type="bibr"` (87 / 71 / 59) и **120
  уникальных обязательных целей (55 / 41 / 24), и все 120 висят** (знаменатель —
  сумма пер-фикстурных значений: имена `b0..` повторяются между документами).
  RED зафиксирован явным пином без `#[ignore]`, который падает при отклонении
  от замера в любую сторону.
- **Текущий GREEN (S02/T02–T05).** Композитор публикует исходные ключи
  (declared `xml:id`, иначе `b{ordinal}` по `da_ports::citation_link`) и
  отвергает `Accepted`, если после splice хоть одна обязательная локальная
  ссылка не резолвится в id-пространстве эмиссии: обязательные targets выводятся
  из сохранённого документа, а не из `candidate.anchors`, поэтому пустой
  anchor-поток больше не делает гейт вакуумным. Типизированные классы отказа —
  `BibliographyDomainMismatch`, `BibliographyKeyCollision`,
  `UnresolvedBibliographyTarget`, `InconservableBibliographyChild` — идут с
  байт-идентичным Java-envelope. На тех же трёх фикстурах: те же 207 записей
  `biblStruct` (80 / 49 / 78), 217 вхождений, 55 / 41 / 24 обязательных целей и
  **0 висячих**; id-домен эмиссии равен исходному. `NativeFulltextManifest`
  несёт блок `reference_links` (`ReferenceLinkLedger`, `#[serde(default)]`):
  `source_entries`, `admitted_references`, `target_occurrences`,
  `required_targets`, `resolved_targets`, `id_space`.
- **Невакуумное доказательство на реальном прогоне (S02/T05).** Shadow-пример на
  фикстуре с одной body-ссылкой печатает блок `reference_links`
  (`required_targets=1`, `resolved_targets=1`, `id_space=3`) и падает, если
  `required_targets == 0`, поэтому пустой блок больше не выглядит как
  доказательство. Замороженный артефакт
  `artifacts/parse_compat/native_fulltext_composition_shadow.json`
  перегенерирован и запиннен
  (`5b125dcb60fc9f203675257b2ffbbea6f2b44ffceb4e12a597f7a3a8baf1feca`), заодно
  сняв его до-T02 расхождение с композитором (в артефакте оставался
  синтетический `bibl-0`). Контракт
  `data/parse/native_fulltext_composition.yaml#reference_link_integrity`
  машинно сверяется с константами композитора
  (`NATIVE_FULLTEXT_REFERENCE_LINK_*`) и с сериализованной формой ledger'а, а не
  дублирует словарь прозой.

| Проверка (актуальная ревизия) | Результат |
|---|---|
| `cargo test -p da-parse --test native_fulltext_composition_reference_integrity` | 19 passed, 0 failed, 0 ignored |
| `cargo test -p da-parse --test native_fulltext_composition_data_contract` | 5 passed, 0 failed, 0 ignored |
| `cargo test -p da-parse --test native_fulltext_composition_oracle` | 2 passed, 0 failed, 0 ignored |
| `cargo test -p da-parse --tests` | 101 suites, 1396 passed, 0 failed, 0 ignored |
| shadow-пример (`--features pdf-layout`) | exit=0; `reference_links.required_targets=1`, `resolved_targets=1`, `id_space=3` |
| `PYTHONPATH=scripts python3 -m governor review --registry doc/review/grobid-absorption-roadmap-2026-09-21.yaml` | exit=0 (stages=8 unresolved=8 claims=0) |

H15 при этом **не помечается PASS**: planning-запись сохраняет
`implementation_pass=false`, `verification_pass=false`, `status: pending`, и
закрытие этапа требует независимой корпусной регрессии S05 — трёх сохранённых
TEI для этого недостаточно. Замеренная область — публичный shadow-композитор;
production-достижимость не доказана. Замки не менялись: `deployed=false`,
`import_eligible=false`, `auto_promote=forbidden`, `graph_write=forbidden`,
`this_wave_promotes=none`, D127/D321 закрыты.

### Свидетельства среза M511-j7wz2f/S03 (семантические поля: исторический RED и текущий GREEN)

A-FIELDS закрыт на замеренной области, и это измерено независимым оракулом
(`roxmltree`, **dev-dependency**, в `src/` недостижим), который читает эмиссию
ПОСЛЕ сериализации, а не candidate-структуру. Оракул ищет sentinel-значения
admitted-полосы, а не наличие элементов: источник консервирует собственные
`<title>`/`<email>`/`<affiliation>`, поэтому проверка по виду элемента объявила
бы conserved-текст источника admitted-полем.

- **Исторический RED (S03/T01).** Измерено на трёх sha256-пиннутых сохранённых
  TEI (`hub_2507.19457`, `rlm_2512.24601`, `reca_2605.26525v1`): из admitted
  header/reference полей в эмиссии не присутствовало НИ ОДНО — счёт admit →
  conserve был `0` по каждому классу, тогда как сам источник несёт
  `title` (2/2/2 элемента), `email` (0/3/0) и `affiliation` (0/0/13). RED
  запиннен как данные (точное равенство, exit 0), а не как `#[ignore]`-тест.
- **Текущий GREEN (S03/T02).** Эмиттер сериализует admitted header
  title/email/ids, author→affiliation bindings и bibliography citation
  roles; production post-serialization semantic gate перечитывает выданные
  байты своим структурным сканером и отказывает типизированно
  (`SemanticFieldLoss`) с байт-идентичным Java-envelope при любом классе,
  который emission не консервирует. `NativeFulltextManifest` несёт
  `semantic_fields` (`SemanticFieldLedger`, `#[serde(default)]`): admitted и
  conserved счётчики по пяти классам. Словарь отказов — `SemanticFieldLoss`,
  и его `token()`/`citation_field_role_token` сверяются с словарём оракула и
  serde-wire-формой отдельными drift-тестами.
- **Политика lane fallback (S03/T03).** Машинно-проверяемый sibling-блок
  `lane_fallback_policy` живёт внутри уже санкционированного
  `data/parse/native_fulltext_composition.yaml` (D377, нового файла нет):
  скаляры связаны с константами композера `NATIVE_FULLTEXT_LANE_FALLBACK_*`,
  классы отказов — с `SemanticFieldLoss::ALL`, а декларации полос сверены
  DATA↔DATA по всему `data/parse`; расхождение/подмена/отмена права полосы на
  Java-fallback падают fail-closed по имени.
- **Невакуумное доказательство на реальном прогоне (S03/T04).** Shadow-пример
  наполняет header-candidate `NativeHeaderFields{title,email,ids}` и
  `NativeAffiliationLink`, а reference band — `CitationField` с ролью;
  отчёт печатает блок `semantic_fields` на верхнем уровне и в каждой строке
  исхода. На замороженной фикстуре: admitted=conserved=`1` по каждому из пяти
  классов, независимое наблюдение примера (подстрокный скан собственных
  sentinel-значений в эмитированных байтах, не по ledger композитора) —
  `observed_fields=5`, все пять классов `true`, и пример падает на любом нулевом
  admitted-счётчике. Ограничение названо честно: подстрокный скан не видит
  XML-структуру, поэтому структурный оракул — `native_fulltext_composition_semantics.rs`,
  а пример доказывает, что admitted-семантика невакуумна и присутствует в байтах.
  Замороженный артефакт
  `artifacts/parse_compat/native_fulltext_composition_shadow.json` перегенерирован
  и запиннен (`3a58566a26410331768327a7f90db8dd484bc2f4f7c247dcc7067d71b7ba256b`)
  в обеих governor-поверхностях (`bindings.canonical_artifacts` и
  `governor/fixtures/scholarly_parse_plane/contract.positive.json`); DATA-дайджест
  `data/parse/native_fulltext_composition.yaml` — `266b68b4…`, уже перепинен T03.

| Проверка (актуальная ревизия) | Результат |
|---|---|
| `cargo test -p da-parse --tests` | 102 suites, 1413 passed, 0 failed, 0 ignored |
| `cargo test -p da-cli --tests` | 37 suites, 584 passed, 0 failed, 0 ignored |
| `cargo test -p da-cli --features pdf-layout --tests` | 37 suites, 612 passed, 0 failed, 0 ignored |
| `cargo clippy -p da-parse --all-targets --no-deps -- -D warnings` | exit=0 |
| `cargo fmt -p da-parse -- --check` / `cargo fmt -p da-cli -- --check` | exit=0 / exit=0 |
| shadow-пример (`--features pdf-layout`) | exit=0; `semantic_fields.ledger` admitted=conserved=1 по 5 классам, `observed_fields=5` |
| `PYTHONPATH=scripts python3 -m pytest scripts/tests/test_governor.py -q` | 368 passed |
| `PYTHONPATH=scripts python3 -m pytest scripts/tests/test_scholarly_parse_governance.py -q` | 437 passed |
| `governor review --registry doc/review/grobid-absorption-roadmap-2026-09-21.yaml` | exit=0 (stages=8 unresolved=8 complete=0 claims=0) |
| `PYTHONPATH=scripts python3 -m governor architecture` (полный гейт, внешний прогон вне in-band `verification_timeout_ms=600000`) | `GATE_RC=0`, `ARCHITECTURE GATE: PASS` (fmt + clippy lib/tests + hexagonal + per-crate tests + feature-gated example runners/contracts; ~1329s) |

H16 при этом **не помечается PASS**: planning-запись сохраняет
`implementation_pass=false`, `verification_pass=false`, `status: pending`, и
закрытие этапа требует независимой корпусной регрессии S05. Измеренный остаток
H16-N3 назван, а не замалчивается: не-namespace атрибуты синтезируемых обёрток
`fileDesc`/`titleStmt` (например собственный `xml:lang`) по-прежнему не
копируются — это остаток поля, а не well-formedness. Замеренная область —
публичный shadow-композитор; production-достижимость не доказана. Замки не
менялись: `deployed=false`, `import_eligible=false`, `auto_promote=forbidden`,
`graph_write=forbidden`, `this_wave_promotes=none`, D127/D321 закрыты.

## Риски, не выдаваемые за воспроизведённые дефекты

| Область | Наблюдение | Требуемая проверка |
|---|---|---|
| Изоляция полос | Любая ошибка compose_inner возвращает общий Java fallback; независимость допуска не равна независимости восстановления после ошибки | Согласовать fail-closed policy и тест header-success/reference-failure |
| Visual absence | `native_visual.rs` пропускает primitives без bbox/digest; пустой результат становится NotObserved | Runtime тест malformed primitive: сохранение причины и знаменателя |
| Byte accounting | fallback-компонентам приписывается размер всего Java-документа | Явно указать scope счётчика; не суммировать как разбиение байтов |
| budget_waits | available_permits проверяется до acquire | Зафиксировать семантику observation, либо измерять действительное ожидание |
| Java parity | Большое число oracle-тестов не задаёт полного множества исходных Java-методов | Инвентарь источников, ветвей и исключений, независимые gold/holdout и tolerance |

## Проверки и границы покрытия

| Запуск | Свежий результат |
|---|---|
| cargo test -p da-parse --tests | 1351 passed, 100 suites; включая lib и 99 интеграционных targets |
| cargo test -p da-domain --lib | 771 passed |
| cargo test -p da-cli --lib | 295 passed |
| PYTHONPATH=scripts python3 -m pytest scripts/tests/test_governor.py -q | 368 passed |
| governor review: horizon, GAR | exit=0; H05/GAR-015 остаются открытыми |
| Публичный Rust composition probe + независимый XML parser | 2 класса дефектов воспроизведены |

99 файлов `da-parse/tests/*.rs`; непересекающееся разбиение по именам не строилось. Подстроки встречаются: contract 55, properties 16, oracle 15, data_contract 13, same_n 8, metamorphic 1. Категории перекрываются. Процент line/branch/mutation coverage не измерялся. Полная архитектурная проверка, все adapter/application suites и живой Java-vs-Rust corpus-run в этом аудите не выполнялись. Утверждать «всё покрыто» нельзя. Строка `1351 passed` в таблице — замер ревизии аудита `bcdd0cfc5`; после S01/T06 тот же набор даёт 100 suites / 1374 passed / 0 failed / 0 ignored (раздел T04–T06), а после S02/T01–T05 — 101 suites / 1396 passed / 0 failed / 0 ignored (раздел S02),
а после S03/T01–T04 — 102 suites / 1413 passed / 0 failed / 0 ignored (раздел S03).

Доказательства GSD exec: `6c9cbd4d-e9eb-4281-8da8-1fdc7d724f1f` (Rust probe), `73a4222b-035b-4439-955e-96d36c068534` (test totals и валидность исходного XML), `c0d5f8f1-490c-4fe0-87c3-4f6e495c9a5b` (scope и test tags), `98f91300-d06b-4d67-8917-19d64e7b8d18` (реестры).

## Индексация

CBM переиндексирован через CLI; живые MCP list_projects, trace_path и check_index_coverage выполнены. Проект root-daily-archive: 499260 узлов, 644211 рёбер. Для трёх проверенных runtime-файлов metadata_match/no_recorded_issue; это best-effort, не полнота call graph. Входящие связи compose_native_fulltext: 12 тестовых вызывающих функций и один shadow-example.

GitNexus MCP context завершился Connection closed. Первая индексация провалилась из-за исчерпания buffer pool 827 MiB; повтор с 4 GiB успешно завершён за 152.1 с: 81431 узел, 130142 ребра, 639 flows. CLI context нашёл compose_native_fulltext и исходящее ребро compose_inner, но incoming пуст даже при 13 callers в CBM и явных вызовах в исходниках. Это измеренное расхождение анализаторов: даже пометка GitNexus epistemic=exact здесь не доказывает отсутствие вызывающих функций. MCP-мост остался недоступен; использован CLI fallback.

## Остаток и политика

Исторический аудит задач: терминальные 1179/1179; это не доказательство полного перевода. Отложенный незапланированный объём вообще не входит в этот знаменатель. Пять parked milestone имеют 13 skipped срезов без задач. H05 и GAR-015 не закрыты. A-XML закрыт на замеренной области S01, A-REF — на
трёх пиннутых сохранённых TEI в S02; A-FIELDS закрыт на замеренной области S03
(невакуумный runtime-прогон shadow-примера). Исторические PASS задним числом не
переписываются.

Следующий план: `doc/review/grobid-absorption-roadmap-2026-09-21.yaml` и `doc/review/grobid-absorption-followup.json`. Проверять командой `PYTHONPATH=scripts python3 -m governor review --registry doc/review/grobid-absorption-roadmap-2026-09-21.yaml`. Прохождение означает целостность planning registry, не выполнение будущих стадий. Новая волна auto не запускается; production promotion, import и graph writes не разрешены.
