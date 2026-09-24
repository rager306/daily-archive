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
| Ресурсы | Global budgets и bounded reorder | Измерено 2026-09-24 (S06): два конкурентных батча с медленным ordinal-0 на локальном детерминированном loopback-стабе — RSS 21114880 B, хвосты p50 5 / p95 93 / p99 93 мс, bounded reorder 2, пик удержанных байт 61146 B внутри введённого потолка 262144 B, timeout ≠ закрытый peer по типу (`BackendTerminationClass`), `budget_waits` observed=2 | Java-CPU остаётся типизированным `not_observed` (субстрат без Java-процесса); H05 поэтому `in_progress` с суженным open item, а не закрыт |
| Внешняя оценка | Независимый frozen probe и release decision | Есть пятистатейный пилот и package contract | Требуемый isolated probe not_executed, reviewer=null, decision=pending; пилот не заменяет этот допуск |
| Governor | Исполнимая проверка правил и честность статусов | 368 тестов проходят; оба исторических review-реестра валидируются | Это не доказательство корректности XML или качества парсинга |

## Подтверждённые дефекты

Серьёзность ниже относится к shadow-композитору. Доказательств воздействия на боевой Java-маршрут нет.

| ID | Приоритет | Дефект и место | Доказательство | Почему текущие тесты пропускают |
|---|---|---|---|---|
| A-XML | P0 | `native_fulltext_composition.rs:402–434,602–613`: сохранение диапазонов по имени смешивает открывающие/закрывающие теги, создаёт пересечения; output не перепарсивается | Настоящий Rust `compose_native_fulltext` возвращает Accepted для трёх валидных сохранённых TEI; независимый ElementTree отвергает все три результата | Проверяется наличие строк в простом синтетическом заголовке, не well-formedness реального результата |
| A-REF | P0 | `native_fulltext_composition.rs:493–520` (ревизия `bcdd0cfc5`): проверяются только переданные anchors, а не фактические body targets; emit меняет id на bibl-ordinal. Текущее состояние — раздел S02 ниже | При исходном body target=#b0 и пустых supplied anchors Rust возвращает Accepted; b0 отсутствует в выходном множестве xml:id | Есть тест чужого переданного anchor, но нет проверки полноты anchors относительно исходного body |
| A-FIELDS | P1 | `native_fulltext_composition.rs:460–520` (ревизия `bcdd0cfc5`): header emitter читает authors/date, references emitter только source. Текущее состояние и снятие RED/GREEN — раздел S03 ниже | Статическое чтение: структурированные header fields, author affiliations и citation fields не сериализуются этими функциями | Сохранение строки не проверяло семантическую эквивалентность title/email/id/affiliation/citation roles |
| A-ABSENCE | P1 | Визуальные head-детекторы (`native_visual.rs`) пропускали primitives без bbox/digest и теряли уже объявленную адаптером типизированную причину: объявленное отсутствие уплощалось до `NotObserved`, знаменатель — до 1. Текущее состояние и снятие RED/GREEN — раздел S04 ниже | Измеренный RED-пин: malformed primitive с `NativeVisualAbsence::MalformedBbox` давал `Terminal{reason: NotObserved, terminal: 1}` при N объявленных | `malformed_observation_inputs_fail_closed_before_detection` проверял только `matches!(outcome, Terminal{..})`, не причину и не знаменатель |
| A-BYTES | P1 | `native_fulltext_composition.rs`: `fallback_entry` присваивал каждому fallback-компоненту `byte_count = java.len()` и `sha256` всего Java-документа без scope, поэтому сумма `byte_count` учитывала документ `fallback_components` раз, а не разбивала байты. Текущее состояние и снятие RED/GREEN — раздел S04 ниже | Измеренный RED-пин: при документе 307B шесть fallback несли `byte_count=307` (сумма 1842), `sum_all=2129` при `native_sum=287` | Не было поля, различающего длину native-фрагмента и размер всего документа как ссылки на сохранённое Java-доказательство |

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
planning-запись сохраняет `implementation_pass=false`. A-ABSENCE и A-BYTES закрыты на замеренной области S04 — malformed primitive сохраняет типизированную причину и полный знаменатель, а байтовые счётчики компонентов несут явный scope; измерение и RED→GREEN — в разделе S04 ниже. H17 при этом не помечается PASS: planning-запись сохраняет `implementation_pass=false`.

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

### Свидетельства среза M511-j7wz2f/S04 (visual absence и byte accounting: исторический RED и текущий GREEN)

A-ABSENCE и A-BYTES закрыты на замеренной области. Оба дефекта воспроизведены
как данные до правки — тот же паттерн, что S01/T01, S02/T01 и S03/T01: RED-пин
падает при отклонении в любую сторону и печатал измеренный дефектный профиль,
а не желаемое поведение.

- **A-ABSENCE, исторический RED (S04/T01).** Malformed primitive, который
  адаптер `crates/da-cli/src/visual_adapter.rs` уже несёт с типизированной
  причиной (`NativeVisualAbsence::MalformedBbox`), терял её в детекторах
  `detect_figure_heads`/`detect_table_heads`/`detect_formula_heads`: результат
  уплощался до `Terminal { reason: NotObserved }` со знаменателем `terminal=1`
  при одном объявленном элементе и при N объявленных на разных страницах.
  Существующий `malformed_observation_inputs_fail_closed_before_detection`
  маскировал дефект: проверял только `matches!(outcome, Terminal{..})`, не
  причину и не знаменатель.
- **A-ABSENCE, текущий GREEN (S04/T02).** Детекторы отделяют валидные
  primitives (bbox+digest) от объявленно-отсутствующих и возвращают
  `Terminal { reason, metrics.terminal = items.len() }`: `reason` —
  детерминированно первая объявленная причина в порядке (page_ordinal,
  source_ordinal) (популяция уже отсортирована), `terminal` — знаменатель
  объявленных, но не допущенных элементов, не константа 1. Истинное пустое
  множество того же kind по-прежнему даёт законный `NotObserved`, а нелегальная
  форма (bbox/digest/absence все пусты) отвергается fail-closed
  `NativeVisualError::InvalidGeometry` — «0 без типа» невозможно. Новых
  вариантов enum не добавлено, serde-контракт потребителей не ломается.
  DATA-блок `typed_absence_semantics` в `data/parse/native_visual_heads.yaml`
  связывает словарь отсутствий с Rust-константами: `pooling`,
  `missing_head_denominator` и `unity_denominator` запрещены,
  `terminal_is_pass=false`, отсутствие блока докладывается как
  `typed-absence-semantics:missing`, а не как drift.
- **A-BYTES, исторический RED (S04/T03).** `fallback_entry` в
  `crates/da-parse/src/native_fulltext_composition.rs` присваивал каждому
  fallback-компоненту `byte_count = java.len()` и `sha256`/`evidence_digest`
  всего Java-документа без scope, поэтому наивная сумма `byte_count` учитывала
  документ `fallback_components` раз. Измерено в фикстуре теста: при документе
  307B шесть fallback несли `byte_count=307` (сумма 1842), и `sum_all=2129`
  при `native_sum=287` — не разбиение байтов.
- **A-BYTES, текущий GREEN (S04/T04).** `ByteCountScope { NativeFragment,
  WholeDocumentReference }` на `FulltextComponentManifest` (`#[serde(default)]`,
  по умолчанию `NativeFragment` сохраняет декодируемость старых манифестов),
  манифестный `ByteAccountingLedger` (`native_fragment_bytes`,
  `java_document_bytes`, `fallback_is_reference_not_partition`) и
  `native_byte_partition()`, пере-выводящий разбиение из компонентов: только
  native-фрагменты разбивают эмиссию, whole-document fallback — ссылка на
  сохранённое Java-доказательство. `byte_accounting_violations()` ловит
  `NativeFragmentWithoutAdmittedSplice`, `ReferenceDoesNotRepeatDocument`,
  `NativePartitionDisagreesWithLedger`,
  `JavaDocumentLengthDisagreesWithManifest` и `FallbackDeclaredAsPartition`.
  `byte_count`/`java_fallback_bytes` не переименованы (R123, обратная
  совместимость).
- **Невакуумное runtime-доказательство (S04/T05).** Shadow-пример печатает
  `byte_scope` в каждом компоненте и верхнеуровневый `byte_accounting`; на
  замороженной фикстуре: `native_fragment_bytes=517` (header 370 + references
  147), `java_document_bytes=490`, `whole_document_reference_components=5`,
  `naive_byte_count_sum=2967` = 517 + 5×490, то есть наивная сумма не разбиение.
  Пример падает, если scope не сериализуется или native-разбиение пусто (тот же
  урок `required_targets == 0` из S02/T05 и `admitted == 0` из S03/T04).
  Визуальный runtime печатает
  `malformed_probe … mapped_lane_absence=malformed_bbox is_not_observed=false`.
  Замороженный артефакт
  `artifacts/parse_compat/native_fulltext_composition_shadow.json`
  перегенерирован и запиннен
  (`a96cab8ff36b3bfe54b620445d767e54d161ee8671b0c277a546992ac8f2f252`) в обеих
  governor-поверхностях (`bindings.canonical_artifacts` и
  `governor/fixtures/scholarly_parse_plane/contract.positive.json`);
  `data/parse/native_visual_heads.yaml` — `d4b15517…` (перепинен T02), а
  визуальный артефакт `visual_positive_fixture_run.json` byte-идентичен
  (`50391d75…`), поэтому перепин не требуется.

| Проверка (актуальная ревизия) | Результат |
|---|---|
| `cargo test -p da-parse --test native_visual_typed_absence` | exit=0, 8 passed (GREEN-контракт, H17-N1/N2) |
| `cargo test -p da-parse --test native_fulltext_composition_byte_accounting` | exit=0, 6 passed (scope, неаддитивность, serde roundtrip) |
| `cargo test -p da-parse --tests` | 104 suites, 1430 passed, 0 failed, 0 ignored |
| `cargo test -p da-cli --tests` | 37 suites, 584 passed, 0 failed, 0 ignored |
| `cargo test -p da-cli --features pdf-layout --tests` | 37 suites, 612 passed, 0 failed, 0 ignored |
| `cargo clippy -p da-parse --all-targets --no-deps -- -D warnings` | exit=0 |
| `cargo fmt -p da-parse -- --check` / `cargo fmt -p da-cli -- --check` | exit=0 / exit=0 |
| shadow-пример (`--features pdf-layout`) | exit=0; `byte_accounting` native=517 java=490 naive=2967, `byte_scope` у всех 7 компонентов |
| `native_visual_heads_shadow` (`--features pdf-layout`) | exit=0; malformed_probe mapped_lane_absence=malformed_bbox is_not_observed=false |
| `visual_positive_fixture_run` (`--features pdf-layout`) | exit=0; артефакт byte-идентичен (`50391d75…`) |
| `PYTHONPATH=scripts python3 -m pytest scripts/tests/test_governor.py -q` | 368 passed |
| `PYTHONPATH=scripts python3 -m pytest scripts/tests/test_scholarly_parse_governance.py -q` | 437 passed |
| `governor review --registry doc/review/grobid-absorption-roadmap-2026-09-21.yaml` | exit=0 (stages=8 unresolved=8 complete=0 claims=0) |
| `PYTHONPATH=scripts python3 -m governor architecture` (полный гейт, внешний прогон вне in-band `verification_timeout_ms=600000`) | `GATE_RC=0`, `ARCHITECTURE GATE: PASS` (fmt + clippy lib/tests + feature-matrix + per-crate tests + feature-gated example runners/contracts; ~1135s) |

H17 при этом **не помечается PASS**: planning-запись сохраняет
`implementation_pass=false`, `verification_pass=false`, `status: pending`,
`owner: unassigned`, `open_item.accepted=false`, а закрытие этапа требует
независимой корпусной регрессии S05. Измеренный остаток назван, а не замолчан:
при смеси valid+absent head-outcome несёт receipts, а поштучные причины
отсутствующих элементов в нём не публикуются. Замеренная область — публичный
shadow-композитор; production-достижимость не доказана. Замки не менялись:
`deployed=false`, `import_eligible=false`, `auto_promote=forbidden`,
`graph_write=forbidden`, `this_wave_promotes=none`, D127 не снят.

### Свидетельства среза M511-j7wz2f/S05 (независимая семантическая регрессия реального корпуса: замер и названный RED)

S05 закрывает требование H18 «регрессия на РЕАЛЬНОМ корпусе, а не на синтетических
фикстурах» на **измеренной** области из трёх sha256-пиннутых preserved TEI
(`hub_2507.19457`, `rlm_2512.24601`, `reca_2605.26525v1`). Природа RED здесь иная,
чем в S01–S04: дефектного поведения композитора на этом корпусе **не найдено** —
измерено `conserved == admitted` и `fabricated == 0` для всех классов. Поэтому
«историческим RED» S05 является **отсутствие независимой корпусной регрессии**
(H18 держался `pending`), а текущий замер — не «исправленный дефект», а впервые
полученные числа. Ниже — и числа, и фактический RED собственного набора среза,
названный в T06.

- **Оракул (S05/T01).** `crates/da-parse/tests/native_fulltext_corpus_semantics.rs`
  — test-only; `crates/da-parse/src` не тронут. admitted-полоса строится ИЗ
  РЕАЛЬНОГО документа через публичные типизированные парсеры, единственный SUT —
  `compose_native_fulltext`, а судья — перечитывание байтов эмиссии через
  `roxmltree` (dev-dependency). Консервация для header-классов считается как
  occurrence-net (`emitted_matches - source_matches`), потому что композитор
  пере-вставляет исходные header-поддеревья дословно: membership-only оракул дал
  бы вакуумную 1.0 даже для полосы, которая ничего не внесла. Самопроверяющийся
  independence guard сканирует собственный исходник и запрещает call-формы
  `compose_header`/`compose_references`/`run_header_sequence`/`run_reference_sequence`.

- **Измеренный профиль 2026-09-23 (S05/T01); 30 per-class строк заморожены,
  отклонение в любую сторону падает.** 10 классов:

| класс | source_population | admitted | conserved | fabricated | admission_coverage |
|---|---|---|---|---|---|
| `AdmittedAuthorLine` | 36 | 35 | 35 | 0 | 0.972222 |
| `SemanticFieldLoss::HeaderTitle` | 6 | 3 | 3 | 0 | 0.500000 |
| `SemanticFieldLoss::HeaderEmail` | 3 | 3 | 3 | 0 | 1.000000 |
| `SemanticFieldLoss::HeaderIds` | 3 | 3 | 3 | 0 | 1.000000 |
| `SemanticFieldLoss::AuthorAffiliation` | 13 | 13 | 13 | 0 | 1.000000 |
| `source_listbibl_direct_children` | 207 | 207 | 207 | 0 | 1.000000 |
| `CitationFieldRole::doi` | 60 | 60 | 60 | 0 | 1.000000 |
| `CitationFieldRole::arxiv_id` | 35 | 35 | 35 | 0 | 1.000000 |
| `CitationFieldRole::url` | 148 | 148 | 148 | 0 | 1.000000 |
| `CitationFieldRole::article_title` | 66 | 66 | 66 | 0 | 1.000000 |
| **агрегат (micro-occurrence)** | **577** | **573** | **573** | **0** | **0.993068** |

`TP=573 FN=0 FP=0 P=R=F1=1.000000`; `admission_coverage = 573/577 = 0.993068`.
Все сужения допуска названы и типизированы, ни одно не молчит: полосная
дефиниция title (1 из 2 header `<title>` — прямой `titleStmt/title`, второй
source-conserved) ×3 документа и `hub` 20/21 author lines —
`NativePersonalNameParser` отверг один реальный `<persName>` как
`ambiguous:single_token`, что независимо подтверждает объявленный
`hub_native_accepted_header=20`. Классы без реальной carrier-формы
(`AuthorAffiliation` на hub/rlm, `HeaderEmail` на hub/reca) — типизированные
нули с `coverage=1.0` при `0/0` и утверждением, что `coverage == 1.0` ровно
тогда, когда `admitted == source_population`. Per-document `listBibl`-знаменатели:
hub 80 / rlm 49 / reca 78 = 207 reference-текстов.

- **DATA (S05/T02).** Блок `corpus_semantic_regression` вложен внутрь уже
  санкционированного `native_fulltext_composition_shadow` в
  `data/parse/comparison_contract.yaml` (D377: нового файла `data/parse` не
  создано). Каждый скаляр связан с ЖИВЫМИ константами композитора
  (`SemanticFieldLoss::ALL`, `citation_field_role_token`, serde wire form,
  `NATIVE_FULLTEXT_FALLBACK_TARGET`, `NATIVE_FULLTEXT_REFERENCE_LINK_DERIVATION`),
  а не со второй копией словаря. Валидатор `native_fulltext_corpus_data_contract.rs`
  пере-выводит метрику, пере-хэширует sha256 каждого документа из tracked байтов
  и держит DATA↔DATA согласие с `s12_named_denominators` (`references_total=207`,
  `header_raw_total=36`, `header_title_elements_total=6`, `header_emails_total=3`,
  `header_pubnum_elements_total=3`, `affiliation_linking_total=13`,
  `bibl_struct_hub/rlm/reca=80/49/78`). Digest `776167fa…` перепинен в обеих
  governor-поверхностях.

- **Мутационная матрица (S05/T03).** `native_fulltext_corpus_mutations.rs` —
  словарь из DATA, applier-реестр сверяется с DATA-множеством, EXPECTED-детекция
  не пере-объявляется в тесте. **12 declared mutations, 12 detected, 0 survived:**
  drop_admitted_header_title/email/ids/author_affiliation → `SemanticFieldLoss::{HeaderTitle,HeaderEmail,HeaderIds,AuthorAffiliation}`;
  substitute_citation_field_role → `SemanticFieldLoss::CitationRole`;
  truncate_admitted_references → `BibliographyDomainMismatch`;
  inject_fabricated_band_note_group / inject_fabricated_header_title →
  `metric_deviation:precision:below_one`; substitute_byte_scope →
  `ByteAccountingViolation::NativePartitionDisagreesWithLedger`; unbalanced_xml →
  `UnbalancedXml`; raw_ampersand_xml → `MalformedXml`; collapse_identity_to_one_sha →
  `identity_failure:unique_source_sha256_not_three`.

- **Runtime-артефакт (S05/T04).** `artifacts/parse_compat/native_fulltext_corpus_shadow.json`
  — byte-детерминированный
  (`efc1984c3ff778da3f2d8d4929cdf7dc5c46eddbb2d80d6b6d23c1837cbe467f`, два прогона
  из разных рабочих каталогов byte-идентичны). `source_identity.identity_kind=preserved_three_sha`,
  `unique_source_sha256=3`, `denominators.documents=3`, 3 строки с 3 различными
  sha, `corpus.aggregate` TP=573/FP=0/FN=0/P=R=F1=1.0/admission_coverage=0.993068,
  `mutation_summary` (12 id→detector), policy-замки и 15-флаговый redaction-блок.
  `--self-check` доказывает 8 fail-closed guard-ов (включая дрейф checksum
  фикстуры и не-UTF8); pin-набор `native_fulltext_corpus_shadow_test` — 6/6.

- **Governor (S05/T05).** Артефакт запинен в ОБЕИХ governor-поверхностях
  (`bindings.canonical_artifacts` + `governor/fixtures/scholarly_parse_plane/contract.positive.json`)
  с identity `native-fulltext-corpus-shadow.v1`; composition-identity проверка
  итерирует оба composition-артефакта со всеми тремя прежними предикатами, так
  что схлопнутая (3 sha → 1) или переименованная corpus-identity падает ПО ИМЕНИ
  артефакта. Набор `test_scholarly_parse_governance.py` 437 → 443 (≥441).

#### Названный RED собственного набора среза (T06): corpus-артефакт vs GAR-006

Финальный замер среза обнаружил **фактический RED**, а не PASS:
`cargo test -p da-parse --tests` → **exit 101**,
`data_contracts::population_contract_enumerates_every_shadow_report` падает с
`shadow reports missing from population_contract.report_paths: ["artifacts/parse_compat/native_fulltext_corpus_shadow.json"]`.
Причина измерена, а не предположена: артефакт попадает под glob
`artifacts/parse_compat/*_shadow.json` (GAR-006), но объявить его в
`population_contract` нельзя — форма, предписанная S05/T04 (`denominators` +
`corpus`, без разбиения reference/executed/evaluated), даёт ровно пять
контрактных ошибок: 3 отсутствующих блока (`reference_population`,
`executed_population`, `evaluated_population`) и 2 запрещённых legacy-блока
(`denominators`, `corpus`). Проверено эмпирически (артефакт объявлен → пять
ошибок → откат). Это конфликт внутри самого плана S05: T04 предписывает форму
артефакта, несовместимую с GAR-006-контрактом, который T06 обязан держать
зелёным. Разрешение (привести артефакт к разбиению reference/executed/evaluated
либо явно классифицировать его вне перечисления population_contract) лежит вне
файловой поверхности T06 и требует решения на уровне среза. H18 поэтому остаётся
`pending`; PASS не выдуман.

| Проверка (актуальная ревизия `4af6a2dd3`) | Результат |
|---|---|
| `cargo test -p da-parse --test native_fulltext_corpus_semantics` | exit=0, 7 passed |
| `cargo test -p da-parse --test native_fulltext_corpus_data_contract` | exit=0, 3 passed |
| `cargo test -p da-parse --test native_fulltext_corpus_mutations -- --nocapture` | exit=0, 5 passed; 12 declared / 12 detected / 0 survived |
| `cargo run -p da-cli --features pdf-layout --example native_fulltext_corpus_shadow -- --report artifacts/parse_compat/native_fulltext_corpus_shadow.json` | exit=0; documents=3 accepted=3 TP=573 FP=0 FN=0 P=R=F1=1.0 coverage=0.993068; sha256 `efc1984c…` |
| `... native_fulltext_corpus_shadow --self-check` | exit=0, 8 guards |
| `cargo test -p da-cli --features pdf-layout --test native_fulltext_corpus_shadow_test` | exit=0, 6 passed |
| `cargo test -p da-cli --features pdf-layout --tests` | exit=0, 618 passed, 38 suites |
| `cargo test -p da-parse --tests` | **exit=101, 1 failed** (`data_contracts::population_contract_enumerates_every_shadow_report`) — названный RED; fail-fast останавливается после 732 passed (20 targets) на цели `data_contracts` (102 passed / 1 failed); с `--no-fail-fast` полный итог **1444 passed / 1 failed** — единственный провал |
| `PYTHONPATH=scripts python3 -m pytest scripts/tests/test_scholarly_parse_governance.py -q` | 443 passed |
| `PYTHONPATH=scripts python3 -m pytest scripts/tests/test_governor.py -q` | 368 passed |
| `governor review --registry doc/review/grobid-absorption-roadmap-2026-09-21.yaml` | exit=0 (stages=8 unresolved=8 complete=0 claims=0) |
| `PYTHONPATH=scripts python3 -m governor architecture` (внешний прогон вне in-band cap 600s) | `GATE_RC=0`, `ARCHITECTURE GATE: PASS` (~1266s) |

H18 **не помечается PASS**: `status: pending`, `implementation_pass=false`,
`verification_pass=false`, `owner: unassigned`, `open_item.accepted=false`,
`tested_source.verified=false`. Измеренная область — ровно 3 документа, не
статистический корпус; только 4 варианта `CitationFieldRole` (doi, arxiv_id,
url, article_title) имеют реальную biblStruct-форму в корпусе, остальные 9 не
измерены и не должны читаться как покрытые. Замки не менялись: `deployed=false`,
`import_eligible=false`, `auto_promote=forbidden`, `graph_write=forbidden`,
`production_route=forbidden`, D127 не снят. История S01–S04 не переписывается.

### Свидетельства среза M511-j7wz2f/S06 (глобальные бюджеты и bounded reorder: замер и остаток)

Замер S06/T05 исполнен 2026-09-24 на ревизии рабочего дерева `04ffddac…`
(замер — рабочее дерево, а не закоммиченная ревизия). Субстрат — локальный
детерминированный loopback-стаб (`kind: local_deterministic_loopback_stub`,
`live_grobid_participated=false`, `deterministic_substrate=true`), поэтому
production-достижимость не заявляется.

**Нагрузка и бюджет.** Два конкурентных батча по 3 документа (6 документов),
общий process-local бюджет `execution_capacity=4`, `publication_capacity=6`;
медленный ordinal-0 удерживается детерминированным gate `sibling_responses_answered`
(settle 80 мс). Все 6 терминальных исходов — succeeded, failed=0,
`budget_waits` observed=2, `max_in_flight` observed=3,
`max_publication_outstanding` observed=3, `max_reorder_depth` observed=2.

**Измеренные потолки.** `peak_rss_bytes` observed = 21114880 B (21.1 MB);
`latency_p50_ms`=5, `latency_p95_ms`=93, `latency_p99_ms`=93;
`peak_retained_bytes` observed = 61146 B внутри введённого потолка `262144 B`
(256 KiB), `verdict: within_ceiling` в обоих батчах. Байтовый потолок введён
аддитивно (существующий типизированный `ResponseBytes`-отказ, fail-closed) и не
меняет замороженный summary v2.

**Java-CPU — типизированный остаток.** `java_cpu_wall_clock_ns` остаётся
`not_observed` с названной причиной: «measured substrate is the local
deterministic stub; no Java process participated». Ноль как измерение не
публикуется.

**Классификация отказа.** Закрытый `BackendTerminationClass`
(`transport_timeout` / `peer_closed_connection` / `connection_refused` /
`unattributable`) с чистой `classify_backend_termination` доказан на реальном
локальном сокете: транспортный timeout и закрытый peer различаются типом, а не
предположением (H19-N2). Это снимает риск «timeout = убийство Java-процесса» как
типовой, но не доказывает убийство реального Java-процесса.

**Артефакт и его удержание.** `artifacts/parse_compat/parse_batch_budget_shadow.json`
(sha256 `015a2cca82fc7983d5c93b65af1bc85ef721ddb69e592cfe3e216087e84db12b`)
объявлен в `data/parse/acceptance_manifest.yaml#population_contract.report_paths`
и байтово запиннен в ОБЕИХ governor-поверхностях
(`scripts/governor/checks/scholarly_parse_plane.yaml#bindings.canonical_artifacts`
+ `bindings.integration_evidence`, identity `parse-batch-budget-shadow.v1`, и
`governor/fixtures/scholarly_parse_plane/contract.positive.json`). Артефакт,
который никто машинно не удерживает, — не доказательство; здесь удержание
проверяется `--self-check` (9 fail-closed guard'ов) и governance-пробами
`test_checker_flags_missing_batch_budget_shadow_artifact`,
`test_checker_flags_truncated_batch_budget_shadow_artifact`,
`test_batch_budget_shadow_binding_is_mirrored_and_identity_clean`.

**Состояние реестров.** H05 остаётся `in_progress` в horizon-реестре с суженным
accepted open item (остаток — только Java-CPU); H19 в
`doc/review/grobid-absorption-followup.json` переведён в `in_progress` с суженным
open item и `implementation_pass=false`/`verification_pass=false`
(planning-запись PASS не заявляет). Governor review: horizon `stages=13
unresolved=1 complete=12 claims=12`; absorption `stages=8 unresolved=8 complete=0
claims=0`.

**Границы.** Замеренная область — детерминированный стаб, а не живой GROBID;
RSS — process high-water mark, включающий сам харнесс; Java-CPU не измерен.
Потолок удержанных байт ограничивает удержанные исходы, но не RSS.

## Риски, не выдаваемые за воспроизведённые дефекты

| Область | Наблюдение | Требуемая проверка |
|---|---|---|
| Изоляция полос | Любая ошибка compose_inner возвращает общий Java fallback; независимость допуска не равна независимости восстановления после ошибки | Согласовать fail-closed policy и тест header-success/reference-failure |
| budget_waits | Измерено 2026-09-24 (S06): два конкурентных батча дали `budget_waits` observed=2, т.е. действительные ожидания под contention наблюдаются, а не только предварительная проверка permits | Зафиксировать семантику в DATA-контракте потолков (`data/parse/comparison_contract.yaml#batch_budget_ceilings`) |
| Java parity | Большое число oracle-тестов не задаёт полного множества исходных Java-методов | Инвентарь источников, ветвей и исключений, независимые gold/holdout и tolerance |

## Проверки и границы покрытия

| Запуск | Свежий результат |
|---|---|
| cargo test -p da-parse --tests | 1351 passed, 100 suites; включая lib и 99 интеграционных targets |
| cargo test -p da-domain --lib | 771 passed |
| cargo test -p da-cli --lib | 295 passed |
| PYTHONPATH=scripts python3 -m pytest scripts/tests/test_governor.py -q | 368 passed |
| PYTHONPATH=scripts python3 -m pytest scripts/tests/test_scholarly_parse_governance.py -q | 446 passed (S06/T06; +3 новых пробы на batch-budget артефакт) |
| cargo test -p da-parse --tests (S06/T06) | 110 suites / 1470 passed / 0 failed / 0 ignored; `population_contract_enumerates_every_shadow_report` ok |
| governor review: horizon, GAR | exit=0; H05/GAR-015 остаются открытыми |
| Публичный Rust composition probe + независимый XML parser | 2 класса дефектов воспроизведены |

99 файлов `da-parse/tests/*.rs`; непересекающееся разбиение по именам не строилось. Подстроки встречаются: contract 55, properties 16, oracle 15, data_contract 13, same_n 8, metamorphic 1. Категории перекрываются. Процент line/branch/mutation coverage не измерялся. Полная архитектурная проверка, все adapter/application suites и живой Java-vs-Rust corpus-run в этом аудите не выполнялись. Утверждать «всё покрыто» нельзя. Строка `1351 passed` в таблице — замер ревизии аудита `bcdd0cfc5`; после S01/T06 тот же набор даёт 100 suites / 1374 passed / 0 failed / 0 ignored (раздел T04–T06), а после S02/T01–T05 — 101 suites / 1396 passed / 0 failed / 0 ignored (раздел S02),
а после S03/T01–T04 — 102 suites / 1413 passed / 0 failed / 0 ignored (раздел S03),
а после S04/T01–T05 — 104 suites / 1430 passed / 0 failed / 0 ignored (раздел S04),
а после S05/T01–T05 тот же набор `cargo test -p da-parse --tests` валится на
`data_contracts::population_contract_enumerates_every_shadow_report` (exit 101) —
названный RED раздела S05 выше; без `--no-fail-fast` Cargo останавливается на
упавшей цели. После S06/T05–T06 тот же набор снова зелёный: 110 suites /
1470 passed / 0 failed / 0 ignored, `population_contract_enumerates_every_shadow_report
... ok` — целевой гейт снят GAR-006-разбиением reference/executed/evaluated, а не
замазан.

Доказательства GSD exec: `6c9cbd4d-e9eb-4281-8da8-1fdc7d724f1f` (Rust probe), `73a4222b-035b-4439-955e-96d36c068534` (test totals и валидность исходного XML), `c0d5f8f1-490c-4fe0-87c3-4f6e495c9a5b` (scope и test tags), `98f91300-d06b-4d67-8917-19d64e7b8d18` (реестры). S04: `53726af5-9f84-4682-998c-bbaeb79778cd` (перегенерация замороженного shadow-артефакта), `25049e11-e663-41df-bdd6-d81173c818e0` (визуальный runtime), `e680048f-b40b-4479-bd61-bd78156b3dd1` (fmt/clippy/da-parse suites/pytest/governor review), `64050636-4a3d-4630-87c7-7773177b0f31` (целевые H17-таргеты), `e6b2dff5` (полный `governor architecture`, `GATE_RC=0`).

## Индексация

CBM переиндексирован через CLI; живые MCP list_projects, trace_path и check_index_coverage выполнены. Проект root-daily-archive: 499260 узлов, 644211 рёбер. Для трёх проверенных runtime-файлов metadata_match/no_recorded_issue; это best-effort, не полнота call graph. Входящие связи compose_native_fulltext: 12 тестовых вызывающих функций и один shadow-example.

GitNexus MCP context завершился Connection closed. Первая индексация провалилась из-за исчерпания buffer pool 827 MiB; повтор с 4 GiB успешно завершён за 152.1 с: 81431 узел, 130142 ребра, 639 flows. CLI context нашёл compose_native_fulltext и исходящее ребро compose_inner, но incoming пуст даже при 13 callers в CBM и явных вызовах в исходниках. Это измеренное расхождение анализаторов: даже пометка GitNexus epistemic=exact здесь не доказывает отсутствие вызывающих функций. MCP-мост остался недоступен; использован CLI fallback.

## Остаток и политика

Исторический аудит задач: терминальные 1179/1179; это не доказательство полного перевода. Отложенный незапланированный объём вообще не входит в этот знаменатель. Пять parked milestone имеют 13 skipped срезов без задач. H05 не закрыт, но сужен: измерены RSS (21114880 B) и хвосты латентности
(p50 5 / p95 93 / p99 93 мс) на двух конкурентных батчах с медленным ordinal-0,
введён потолок удержанных байт (пик 61146 B внутри 262144 B) и типизированная
классификация отказа; остаток — только Java-CPU (субстрат без Java-процесса).
GAR-015 не закрыт. A-XML закрыт на замеренной области S01, A-REF — на
трёх пиннутых сохранённых TEI в S02; A-FIELDS закрыт на замеренной области S03
(невакуумный runtime-прогон shadow-примера); A-ABSENCE и A-BYTES закрыты на
замеренной области S04. H18 (независимая корпусная регрессия) построен и
измерен на трёх пиннутых сохранённых TEI в S05, но остаётся `pending`:
измеренная область — ровно 3 документа (не статистический корпус),
9 `CitationFieldRole`-вариантов без реальной carrier-формы не измерены, и
собственный набор среза RED по GAR-006-перечислению (раздел S05 выше).
Исторические PASS задним числом не
переписываются.

Следующий план: `doc/review/grobid-absorption-roadmap-2026-09-21.yaml` и `doc/review/grobid-absorption-followup.json`. Проверять командой `PYTHONPATH=scripts python3 -m governor review --registry doc/review/grobid-absorption-roadmap-2026-09-21.yaml`. Прохождение означает целостность planning registry, не выполнение будущих стадий. Новая волна auto не запускается; production promotion, import и graph writes не разрешены.
