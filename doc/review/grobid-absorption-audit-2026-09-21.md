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
| A-REF | P0 | `native_fulltext_composition.rs:493–520`: проверяются только переданные anchors, а не фактические body targets; emit меняет id на bibl-ordinal | При исходном body target=#b0 и пустых supplied anchors Rust возвращает Accepted; b0 отсутствует в выходном множестве xml:id | Есть тест чужого переданного anchor, но нет проверки полноты anchors относительно исходного body |
| A-FIELDS | P1 | `native_fulltext_composition.rs:460–520`: header emitter читает authors/date, references emitter только source | Статическое чтение: структурированные header fields, author affiliations и citation fields не сериализуются этими функциями | Сохранение строки не проверяет семантическую эквивалентность title/email/id/affiliation/citation roles |

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

99 файлов `da-parse/tests/*.rs`; непересекающееся разбиение по именам не строилось. Подстроки встречаются: contract 55, properties 16, oracle 15, data_contract 13, same_n 8, metamorphic 1. Категории перекрываются. Процент line/branch/mutation coverage не измерялся. Полная архитектурная проверка, все adapter/application suites и живой Java-vs-Rust corpus-run в этом аудите не выполнялись. Утверждать «всё покрыто» нельзя.

Доказательства GSD exec: `6c9cbd4d-e9eb-4281-8da8-1fdc7d724f1f` (Rust probe), `73a4222b-035b-4439-955e-96d36c068534` (test totals и валидность исходного XML), `c0d5f8f1-490c-4fe0-87c3-4f6e495c9a5b` (scope и test tags), `98f91300-d06b-4d67-8917-19d64e7b8d18` (реестры).

## Индексация

CBM переиндексирован через CLI; живые MCP list_projects, trace_path и check_index_coverage выполнены. Проект root-daily-archive: 499260 узлов, 644211 рёбер. Для трёх проверенных runtime-файлов metadata_match/no_recorded_issue; это best-effort, не полнота call graph. Входящие связи compose_native_fulltext: 12 тестовых вызывающих функций и один shadow-example.

GitNexus MCP context завершился Connection closed. Первая индексация провалилась из-за исчерпания buffer pool 827 MiB; повтор с 4 GiB успешно завершён за 152.1 с: 81431 узел, 130142 ребра, 639 flows. CLI context нашёл compose_native_fulltext и исходящее ребро compose_inner, но incoming пуст даже при 13 callers в CBM и явных вызовах в исходниках. Это измеренное расхождение анализаторов: даже пометка GitNexus epistemic=exact здесь не доказывает отсутствие вызывающих функций. MCP-мост остался недоступен; использован CLI fallback.

## Остаток и политика

Исторический аудит задач: терминальные 1179/1179; это не доказательство полного перевода. Отложенный незапланированный объём вообще не входит в этот знаменатель. Пять parked milestone имеют 13 skipped срезов без задач. H05 и GAR-015 не закрыты. Новые A-XML/A-REF требуют новой корректирующей работы, а не переписывания исторических PASS.

Следующий план: `doc/review/grobid-absorption-roadmap-2026-09-21.yaml` и `doc/review/grobid-absorption-followup.json`. Проверять командой `PYTHONPATH=scripts python3 -m governor review --registry doc/review/grobid-absorption-roadmap-2026-09-21.yaml`. Прохождение означает целостность planning registry, не выполнение будущих стадий. Новая волна auto не запускается; production promotion, import и graph writes не разрешены.
