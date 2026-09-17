# Scalar CRF Wapiti: формат и границы проверки

Статус: **candidate/shadow-only; Java GROBID остаётся production authority**  
Волна: M507 / S02 / T07; граница приёмки SC3 уточнена в M507 / S09 / T09  
Дата: 2026-09-14

## Назначение

`da-domain::scalar_crf` содержит чистый Rust reader и Viterbi-декодер для
совместимого scalar CRF формата Wapiti. Файловый I/O находится в
`da-adapters::FilesystemCrfModelLoader`, а граница загрузки описана портом
`CrfModelLoaderPort` в `da-ports`. Это исследовательский ABI и не замена
production Java GROBID route.

## Декодированный формат

Reader принимает байтовый текстовый файл со следующими секциями:

1. заголовок `#mdl#<type>#<nact>`; поддерживается scalar model type `2`;
2. `#rdr#` с шаблонами и их kind (`u`, `b`, `*`), включая шаблоны
   наблюдений и переходов;
3. блоки `#qrk#` с таблицами labels и observations;
4. sparse-строки весов вида `<absolute_index>=<C99 hex-float>`.

Netstring-строки читаются по объявленной длине **в байтах**, а не до перевода
строки. CRLF, обрезанные строки, неизвестные kind, несогласованные счётчики,
переполнение checked-арифметики и индексы за `nftr` отвергаются typed error.
C99 hex-float разбирается отдельным точным parser-ом; `f64::from_str` для этой
части не используется. Отсутствующие sparse-веса означают `0.0`, а не ошибку.

Шаблоны `%x[offset,column]` расширяются относительно границ входной
последовательности (`_x-1.._x-#`, `_x+1.._x+#`). Для каждой позиции строятся
упорядоченные `uobs`/`bobs`; неизвестные observations пропускаются. Затем
Viterbi применяет tie-break через строгое `>` при выборе predecessor и финальной
метки. Размер входа, число шаблонов, labels, observations и weights ограничены
ресурсными потолками, включая `MAX_CRF_MODEL_BYTES`.

## Синтетическая fixture и ручной oracle

Контрактная suite находится в:

- `crates/da-adapters/tests/fixtures/scalar_crf_synthetic.wapiti`;
- `crates/da-adapters/tests/fixtures/scalar_crf_variant.wapiti`;
- `crates/da-adapters/tests/fixtures/scalar_crf_malformed.wapiti`;
- `crates/da-adapters/tests/common/scalar_crf_contract.rs`;
- `crates/da-adapters/tests/scalar_crf_contract.rs`.

Модель hand-authored, содержит две метки, шаблоны `u00:%x[0,0]` и `b`, и
hex-float веса. Для основного oracle вручную зафиксированы `Y=2`,
`uoff(0)=0`, `uoff(1)=2`, `boff(2)=4`, `nftr=8`; ожидаемый decode — `A,B`,
а scores — `[1,6]`. Это независимые данные, не результат запуска Java.

## Опциональный harness реальной модели

`crates/da-adapters/tests/scalar_crf_real_model.rs` читает модель **только**
если вызывающий явно задаёт переменную окружения
`DA_GROBID_CRF_MODEL_PATH`, содержащую настроенный локальный resource path.
Переменная не имеет значения по умолчанию, проектные каталоги не сканируются,
а модельные байты не копируются и не коммитятся. При отсутствии переменной тест
печатает `MISSING` и не выдаёт PASS: это явная missingness, а не доказательство
совместимости. При наличии пути harness проверяет размер, разбирает модель и
применяет `validate_loaded_crf_model`; malformed/unavailable/oversized ресурс
завершает проверку ошибкой.

Пример запуска с локально настроенной resource root выполняется оператором
вне репозитория:

```text
DA_GROBID_CRF_MODEL_PATH=<local-audited-resource>/models/<model>.wapiti \
  cargo test -p da-adapters --test scalar_crf_real_model -- --nocapture
```

Плейсхолдер `<local-audited-resource>` намеренно не является путём,
закодированным в плане или исходниках. Harness доказывает лишь чтение и
доменную валидацию выбранного локального файла; он не доказывает sequence
parity с Java и не запускает GROBID.

## Границы evidence и D358

Синтетическая fixture доказывает parser/offset/Viterbi contract, но **не**
доказывает эквивалентность реальным GROBID моделям, Java Wapiti или production
outputs. Реальный model run остаётся optional evidence и должен иметь
происхождение, checksum и права, подтверждённые отдельно. Согласно D358,
конструкция выбрана так, чтобы не распространять bundled GROBID model bytes:
Java `0.9.0-crf` остаётся authority, а Rust path — candidate/shadow-only; не
меняется `import_eligible=false`, не выполняется graph write и не вводится
sidecar authority.

После получения допустимого real-model evidence необходимо отдельно
перепривязать evidence горизонта H08 к конкретной версии модели, checksum,
resource configuration и результатам decode. Отсутствие такой перепривязки не
может быть истолковано как PASS.

## Приёмочная граница SC3 (решение M507 / S09 / T09)

Дата решения: 2026-09-17.

Критерий приёмки SC3 волны M507 в исходной редакции требовал, чтобы scalar CRF
reader/inference «совпадал с Java same-n на зафиксированных фикстурах». Этот
критерий был зафиксирован milestone-валидацией как unmet as written и
переформулирован штатным путём исправления метаданных milestone, а не ослаблен:
в репозитории нет Java same-n эталона для scalar CRF, а производство такого
эталона не укладывается в объём и стоимость среза.

Измеренное состояние (2026-09-17):

- Java-источников в дереве нет: `find . -name '*.java' -not -path './target/*' | wc -l` → `0`;
- JNI/`JavaVM`-binding в CRF-поверхности отсутствует: поиск по
  `crates/da-domain/src/scalar_crf.rs`, `crates/da-ports/src/scalar_crf.rs`,
  `crates/da-adapters/src/scalar_crf_loader.rs` даёт 0 совпадений (это чистый
  reader формата, без JVM);
- Java-артефактов в дереве нет: `find . -name '*.jar' -not -path './target/*'` → `0`;
- модельные байты в дереве отсутствуют:
  `find . -name '*.wapiti' -not -path './target/*' | wc -l` → `11`, из них
  крупнее 1 KiB → `0` (все синтетические, ≤128 B); запрет бандлинга держит
  машинный гейт `cargo test -p da-parse --test data_contracts d358` → `2 passed`
  (включая `d358_model_bytes_are_never_bundled_into_the_live_tree`);
- harness реальной модели без ресурса печатает
  `MISSING: DA_GROBID_CRF_MODEL_PATH is not configured; no real GROBID model claim was made`
  и выдаёт `1 passed`, не заявляя PASS.

Почему оракул не производится здесь. Единственные реальные веса scalar CRF —
`model.wapiti` размером 62 133 818 байт внутри сервисного контейнера GROBID; это
не repo-ресурс, и он не может быть закоммичен (D358). В репозитории нет
JVM-поверхности декодирования, а live-сервис GROBID не публикует
standalone-эндпоинт пооценочного декодирования по произвольным фичам —
live-харнесс фиксирует вызовы моделей и их счётчики, а не последовательности
меток. Производство Java same-n оракула означало бы новый Java/JNI-харнесс плюс
провизионированный не-repo модельный корень, то есть ровно ту owned env-gated
границу Integration, которая объявлена отложенной (D385) и не заявляется ни
одним артефактом волны. Рассмотренная альтернатива — «оставить критерий как
есть и произвести оракул в этой волне» — отвергнута: она требовала бы
провизионированного модельного корня, которого в окружении нет, и заявляла бы
Integration-покрытие, которое волна сознательно не заявляет.

Действующая редакция SC3 после переформулировки (проверяемая и невырожденная):

> Scalar CRF reader/inference детерминирован на зафиксированных фикстурах и
> совпадает с независимым ручным оракулом, посчитанным из байтов этих фикстур
> (`cargo test -p da-domain --lib scalar_crf`; `cargo test -p da-adapters --test
> scalar_crf_contract`): совпадение последовательности меток и
> score-последовательности с независимо посчитанным эталоном при строгом
> tie-break, повторный прогон даёт тот же результат, а любое расхождение —
> красный тест. Паритет с исполнением Java/JVM на реальных весах GROBID в
> критерий не входит: он отложен на owned env-gated границу Integration (D385),
> потому что байты моделей не коммитятся (D358), и без провизионированного
> модельного корня соответствующий harness печатает typed MISSING, не заявляя
> PASS.

Критерий не вырожден: эталон посчитан независимо от декодера (вручную из байтов
фикстуры), а наблюдаемые `A,B` со scores `[1,6]` и повторяемость прогона
ломаются любой мутацией весов, порядка наблюдений или tie-break. Команды
доказательства:

```text
cargo test -p da-domain --lib scalar_crf              # 11 passed: reader + Viterbi, повторный прогон детерминирован
cargo test -p da-adapters --test scalar_crf_contract  # 4 passed: независимый ручной oracle A,B и scores [1,6]
```

Не-claim: этот раздел не заявляет паритет с JVM/Java на реальных весах GROBID и
не заменяет env-gated Integration-прогон. Он фиксирует границу доказательства и
условие, при котором паритет станет доказуемым.
