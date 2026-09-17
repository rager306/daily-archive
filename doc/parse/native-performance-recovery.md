# Performance recovery нативного ядра S08: границы приёмки и известные пробелы

**Milestone:** M507-4fk6m6 · **Slice:** S08 «Performance recovery и route qualification»
**Remediation:** S09 «Remediation: S08 determinism и claim fidelity» — двухповерхностная модель SMP,
leaf-level доказательство исключения наблюдаемых полей, персистированный projection-дайджест,
перепривязка реестра к детерминированной поверхности
**Нормативная база:** R109 (bounded CPU SMP, детерминированные выходы по числам воркеров,
явные load breakpoints), R085 (измеренная оптимизация и снижение Java-работы), R123
(пофемейная квалификация восстановимости, shadow сохранён), R131 (готовность нативной полосы),
R117 (пороги и маршруты как schema-validated DATA), R113/R116/R130/R138 (closure),
H13/H05 (границы приёмки), ADR-051 (oracle ≠ SUT), ADR-075, D377 (sibling-блоки в
санкционированных DATA, без нового `data/parse/*.yaml`), D378 (единица Java-работы),
D379 (профиль → закрытая диспозиция, ничего не адоптируется), D380 (весь харнесс в `da-cli`),
D127 (import lock).

## Назначение

Документ фиксирует **границу приёмки** среза S08: что именно доказано исполняемыми командами,
чего срез **не** доказывает, в чём измеряется Java-работа и как её переснять, как перезапустить
измерительные продюсеры и как читать поля артефактов, какие пробелы остаются открытыми и какие
локи обязаны остаться закрытыми. Ни одно утверждение ниже не выходит за пределы проверенного в
S08; там, где доказательства нет, это сказано прямо, а не подразумевается.

Все команды выполняются из корня репозитория. Артефакты приёмки (профиль, детерминированный SMP,
Java-работа, реестр маршрутов) байт-воспроизводимы; объявленная observation-поверхность SMP —
намеренно нет (§(d)). Любые числа ниже — **наблюдения артефактов**, снятые командами; в коде и DATA
они не хардкожены.

**Ключевое ограничение среза:** S08 ничего не диспатчит и ничего не промоутит. `deployed=false`,
`import_eligible=false`, `production_route=forbidden`, `auto_promote=forbidden`,
`selection=forbidden`, `decision_point=6`. Все результаты — evidence, а не маршрут.

---

## (a) Что доказывает

Срез доказывает четыре утверждения. Каждое подтверждено командами с ассертами, а не прозой.

### A1. Нативное CRF-ядро профилировано, диспозиция SIMD/PGO закрыта (R109/R085/R117)

Модуль `da_cli::native_core_profile` профилирует реальное ядро на **8 замороженных синтетических
Wapiti-моделях** (`crates/da-parse/tests/fixtures/native_*_synthetic.wapiti`), разбивает работу на
четыре фазы (`render_feature_strings`, `observation_index_build`, `map_input_lookup`,
`viterbi_decode`) и выводит диспозицию **только** через квалифицированный примитив
`da_cli::layout_runtime::gate_simd_candidate(dense, semantic_equal, repeated_cpu_improvement,
full_pipeline_p95_regressed) -> (OptimizationState, reason)`. `da-domain` не изменён.

```bash
cargo test -p da-cli --test native_core_profile_contract   # 7 passed; 0 failed
cargo test -p da-cli --lib native_core_profile             # 2 passed; 0 failed

cargo run -p da-cli --example native_core_profile -- --artifact <dir-a>/profile.json
cargo run -p da-cli --example native_core_profile -- --artifact <dir-b>/profile.json
cmp <dir-a>/profile.json <dir-b>/profile.json
cmp <dir-a>/profile.json artifacts/parse_compat/native_core_profile.json
# оба cmp = 0 ⇒ артефакт байт-воспроизводим
```

Что именно закреплено тестом `native_core_profile_contract`:

| Проверка | Что доказывает |
|---|---|
| два независимых профиля → идентичные байты | детерминизм артефакта (наблюдательные тайминги вынесены из байтов) |
| `scalar_oracle_digest == semantic_digest` на каждой семье | выходы ядра совпадают с scalar-oracle |
| мутированный кернел → `SemanticMismatch` | oracle реально ловит расхождение (оракул не совпадает с SUT) |
| негатив H13-N2: `gate_simd_candidate(false,true,true,false) == (NotApplicable, _)` и `gate_simd_candidate(true,true,true,true).0 == MeasuredRejected` | `measured_retained` **физически недостижим** без кандидата и полного набора условий |
| мутация DATA-порога меняет решение | порог читается из DATA, а не захардкожен |
| redaction: нет token text, нет байт `.wapiti`, нет физических путей и hostname | артефакт не несёт payload |
| аллокаторные дельты стабильны между прогонами, иначе typed unstable-состояние без чисел | измерение аллокаций честное (нестабильность не превращается в число) |

Измеренный результат на момент S08 (артефакт
`artifacts/parse_compat/native_core_profile.json`, 11831 байт,
sha256 `3543b10aad8b8583c35bc6f0efc87d462ca271b890681fc209fbb9bae13f17a8`):
все 8 семейств дают `dense_numeric_kernel_found=true` (плотная фаза `viterbi_decode` перекрывает
DATA-порог 0.25), `optimization_state=measured_rejected`, `selection_forbidden=true`,
`pgo_disposition=not_adopted`, `h05_closure=not_claimed`. **Ничего не адоптировано**: оптимизация
не реализовывалась, кандидат не измерялся, `da-domain::scalar_crf` не тронут, поэтому измеренный
hot spot (пересборка `BTreeMap` и `Vec<Vec<&str>>` в `map_input` на каждом вызове) остаётся
открытым и незаявленным.

### A2. Bounded SMP нативного ядра квалифицирован на всей матрице воркеров (R109)

Модуль `da_cli::native_core_smp` исполняет реальное нативное ядро на детерминированных юнитах
(семья × документ) и переиспользует **только** примитивы `da_cli::layout_runtime`
(`detect_system_topology`, `RuntimeConfig::validate`, `run_bounded`, `Terminal`, `ScratchArena`);
собственного топологического ридера, глобального пула и неограниченных очередей не заводится.

```bash
cargo test -p da-cli --test native_core_smp_contract        # 20 passed; 0 failed
cargo test -p da-cli --features pdf-layout --test layout_smp_contract   # 6 passed; R109-регрессия

# Детерминированная поверхность: два независимых прогона в разные каталоги + cmp с committed.
# --observations-artifact уводит run-snapshot из закоммиченного пути: сравнивать его нельзя
# (он объявлен non-reproducible), а трогать committed-файл прогоном-проверкой не нужно.
cargo run -p da-cli --example native_core_smp \
  --artifact <dir-a>/native_core_smp.json --observations-artifact <dir-a>/native_core_smp_observations.json
cargo run -p da-cli --example native_core_smp \
  --artifact <dir-b>/native_core_smp.json --observations-artifact <dir-b>/native_core_smp_observations.json
cmp <dir-a>/native_core_smp.json <dir-b>/native_core_smp.json
cmp <dir-a>/native_core_smp.json artifacts/parse_compat/native_core_smp.json
# оба cmp = 0 ⇒ детерминированная поверхность байт-воспроизводима и равна committed.
# sha256 observation-поверхности между прогонами ожидаемо различается — это объявленный
# run-snapshot, а не артефакт приёмки.
```

| Проверка | Что доказывает |
|---|---|
| семантический digest идентичен на каждом числе воркеров `1..=physical_core_cap` | SMP не меняет выход |
| запрос выше cap падает `OverCoreCap` **до** спавна и никогда не клампится | предел — типизированный отказ, а не тихая деградация |
| `admitted == terminals`, ровно один terminal на admitted-юнит, ordinal'ы сохранены | консервация работы |
| паника одного job'а → типизированный coordinator failure, остальные терминалы целы | изоляция сбоя |
| эхо конфигурации проваливает ассерт; runtime-метрики наблюдаемы | метрики измеряются, а не декларируются |
| `matrix.breakpoint_vocabulary` — закрытый словарь; вердикты-экземпляры живут только на observation-поверхности | вердикт нельзя изобрести, и наблюдаемая классификация не может попасть в приёмку |
| детерминированные байты не двигаются при пертурбации **каждого** объявленного наблюдаемого поля, а observation-байты обязаны измениться (25 пертурбаций) | исключение доказано leaf-level, а не подстрокой; вакуумный кейс невозможен |
| ни один leaf-путь детерминированной поверхности не лежит под объявленным наблюдаемым именем | `DeterministicEntry` не несёт observational-полей; `queue_wait_ms` остаётся типизированным `not_observed` без числа |

Измеренный результат. **Детерминированная поверхность** (`artifacts/parse_compat/native_core_smp.json`,
38262 байта, sha256 `dc323914f2f0af631b9c04b026d388aa9ddff4a48c2f092db622992aa41ca1aa`, схема
`native-core-smp.v2`): топология — 12 разрешённых CPU / 12 физических ядер; матрица `workers = 1..=12`
(12 записей); на **всех** 12 точках `semantic_digest == matrix.scalar_oracle_digest`
(`sha256:1fc2e3a5b56cdc5b4c9732363aecfd347b1456549fe15d66210787194f0ca54d`),
`admitted == terminals == 48`, `failed == 0`, `coordinator_failed == 0`;
`deterministic_projection.digest = sha256:4ac2c13dc2a80f005baa2288df33c34bf7fd5bd535d986d911ce47b9b0195a9f`
персистится на поверхности и пересчитывается из неё тестом.

**Observation-поверхность** (`artifacts/parse_compat/native_core_smp_observations.json`, схема
`native-core-smp-observations.v1`) — объявленный run-snapshot; её байты и размер намеренно не
цитируются как приёмочные: они меняются от прогона к прогону. Снапшот несёт `reproducible: false`,
`not_a_receipt: true`, а `source_artifact.sha256` пинит ровно байты детерминированной поверхности.
Latency-перцентили, `run_wall_ms`, throughput, queue/scratch high-water marks, process peak RSS и 12
breakpoint-вердиктов живут **только** здесь, исключены из знаменателей приёмки и из выбора
production-маршрута. Эта поверхность не пинится по sha256 и между прогонами ожидаемо различается —
она не является артефактом приёмки.

### A3. Java-работа измерена как число CRF-model invocation'ов с верифицированной ленты (R085)

Модуль `da_cli::native_java_work` принимает `da_parse::StageTape` и выдаёт redacted receipt;
единица — **per-model invocation'ы Java CRF** (одна секция debug-дампа = один вызов модели, D378).

```bash
cargo test -p da-cli --test native_java_work_contract       # 18 passed; 0 failed

# реальный захват (лента и сырой дамп — ВНЕ репозитория):
cargo run -p da-cli --example grobid_stage_tape -- \
  --pdf data/sources/arxiv/cs-cl/2507.19457/source/2507.19457.pdf \
  --out <tape-dir-beyond-repo-root> --endpoint fulltext
cargo run -p da-cli --example grobid_stage_tape -- --verify <tape-dir-beyond-repo-root>/tape.json

cargo run -p da-cli --example native_java_work -- \
  --tape <tape-dir-beyond-repo-root>/tape.json --verify
```

Измеренный результат (`artifacts/parse_compat/native_java_work.json`, 9303 байта,
sha256 `87e69860b04ca0b9a2ca4df2c7c411007a09b16db376a9700844110ba74bf00c`):
`measurement_state=measured`, покрытие `full`, `documents_measured=3/3`, `total_invocations=570`
по 11 различным моделям; все три receipt'а `tape_verification=verified`, `verification_gaps=0`,
`unreadable_tape_count=0`. Числа НЕ хардкожены — они сняты захватом и связаны с identity ленты.

### A4. Восстановимые нативные маршруты сведены в DATA-owned реестр (R123/R131/R085)

Модуль `da_cli::native_route_recovery` сводит DATA-таблицу стадий P0–P9
(`data/parse/grobid_compatibility.yaml#defaults.cascade_stages` →
`production_authority`/`native_surface`), матрицу диспозиций компонентов
(`native_fulltext_composition.yaml#admission.current_matrix`), receipt квалификации ядра
(T01/T02) и receipt Java-работы (T03) в один ledger с **закрытым fail-closed** вердиктом.

```bash
cargo test -p da-cli --test native_route_recovery_contract  # 14 passed; 0 failed
cargo test -p da-cli --lib native_route_recovery            # 13 passed; 0 failed
cargo run -p da-cli --example native_route_recovery         # печатает per-stage вердикты и сумму
```

Стадия становится `recoverable` **только** если одновременно: (a) её native surface
объявлен и присутствует, (b) диспозиция компонента `passed`, (c) выполнены все четыре
`admission.native_substitution_requires` (`exact_source_identity`, `exact_schema_version`,
`optional_sha256_when_present`, `passing_receipt`), (d) есть receipt квалификации ядра и
(e) Java-работа наблюдена. Иначе — `not_qualified` / `fallback` / `not_observed`.
Порядок guard'ов живёт в DATA (`verdict_rule.order`), код лишь сверяет с ним свой закрытый enum.

Измеренный результат (`artifacts/parse_compat/native_route_recovery.json`, 20438 байт,
sha256 `3981e6ef654264d06d2b1fe6e5e83f56d52b28c6636eaabb467ce143c5a56b23`):
`recoverable_java_work.value = 399` из 570 измеренных invocation'ов, `by_stage = {P3: 181, P4: 218}`;
P3/P4 — `recoverable` (`all_conditions_met`), P1/P5/P8/P9 — `not_qualified`
(`no_admitted_component`), P2/P6 — `fallback` (`component_not_admitted`), P0/P7 — `not_qualified`
(`native_surface_absent`). Остаток 171 invocation'ов (P2=3, P5=5, P6=163) честно остаётся за Java.
Блок `core_qualification` публикует `smp_deterministic_projection_sha256` — projection-дайджест,
прочитанный из запиненных байтов SMP-receipt'а, а не пересчитанный: правка объявленного дайджеста
двигает sha256-пин всего receipt'а, поэтому дайджест tamper-evident.

---

## (b) Что не доказывает

| Не доказано | Почему / где граница |
|---|---|
| **Нет Java CPU wall-clock по стадиям.** | `observations.java_cpu_wall_clock.state = not_observed` и в Java-артефакте, и в реестре. Измерение CPU живого сервиса требует отдельной политики измерения (вне S08). |
| **Нет RSS/CPU/byte-потолков.** | H05 остаётся открытым и **без владельца**; `h05_closure = not_claimed` во всех четырёх артефактах приёмки (объявленный observation-снапшот SMP артефактом приёмки не является, §(d)). `memory_pressure_ceiling_bytes.state = not_observed` — DATA `cpu_smp_candidate` не объявляет memory ceiling, поэтому `memory_pressure` не наблюдаем. `peak_rss_bytes` — наблюдательный runtime-гейдж, исключённый из знаменателей приёмки; потолком он не является. |
| **Нет source-bound parity.** | Ни S08-модули, ни ledger не сравнивают текст источника (PDF/TEI) с нативным выходом; сравниваются счётчики и дайджесты. Содержимое дампа не читается вообще. |
| Нет runtime-маршрута и нет сплайса. | Native-маршрут не подключён ни к `parse-pdf`, ни к `ParserPort`: четыре модуля отсутствуют в `crates/da-cli/src/source_cli.rs` и на любом пути `ParserPort` (grep = 0). `production_route=forbidden`. |
| Нет промоушена и нет выбора. | `promotion.state=candidate`, `promoted=forbidden`, `auto_promote=forbidden`, `selection=forbidden`; ledger вердикты «квалифицируют offline-receipt'ы и никогда не диспатчат маршрут». |
| H13 закрыт лишь частично. | H13 включает не только invocations/SMP/профиль, но и latency-хвосты, RSS, queue age, recovery/rollback; эти части остаются `not_observed`, а H13 `depends_on` H05, который не назначен. S08 отдаёт нативную invocation/SMP/profile-часть и **не** объявляет H13 закрытым. |
| `learning_policy` не наблюдается. | В `data/parse/native_fulltext_composition.yaml` она остаётся `not_observed` / `output_affecting: false`; результаты обучения срезом не требуются (S07 не блокирует S08). |
| Никакого self-oracle. | Scalar-oracle — независимый путь (`da-domain::scalar_crf`), SUT-профиль строится на публичных декодерах семейств; ADR-051 (oracle не строится тем же кодом) соблюдён. |

**Proof level среза:** contract (offline, per-crate, байт-воспроизводимые артефакты приёмки) **плюс одна
opt-in integration-конечность** (живой GROBID debug-захват) с MISSING-полярностью: без явно
заданного `GROBID_URL`/`DA_GROBID_URL` тест печатает MISSING и не выполняет ни одного ассерта.
Source-bound parity и runtime-маршрут не заявляются.

---

## (c) Единица измерения Java-работы и как её переснять

### Единица

`java_model_invocations` — число вызовов Java CRF-моделей, выведенное из **captured + verified**
GROBID debug-ленты (`da_parse::StageTape`, `debugMode=1`): одна секция дампа соответствует одному
model invocation. Это единственная offline-измеримая единица Java-работы — она не требует
замеров CPU живого сервиса и не выдумывает чисел. Единица записана в DATA
(`route_recovery.measured_java_work.unit`) и в артефактах (`unit`).

Корпус — существующий пиновый манифест `data/parse/layout_bakeoff.yaml#corpus`
(`corpus_id = s13-same-input-pdf-corpus`, 3 документа, 202 страницы). Каждый PDF связан с
замороженным sha256, который сверяется с байтами файла; идентичность берётся из DATA, а не из ленты.

### Как переснять

```bash
# 1. Захват: по одному документу из пинового корпуса. <tape-dir-beyond-repo-root> —
#    каталог ВНЕ репозитория: сырой debug-дамп содержит token text и коммиту не подлежит.
cargo run -p da-cli --example grobid_stage_tape -- \
  --pdf <repo-relative pinned source_pdf> \
  --out <tape-dir-beyond-repo-root> --endpoint fulltext

# 2. Верификация ленты: пере-хеширует исходный PDF и сохранённый дамп, пробует живой релиз и
#    завершается ненулевым кодом на любой находке (подмена, отсутствие, дрейф, записанный сбой,
#    нулевые секции). Лента, которую никто не проверил, не является доказательством.
cargo run -p da-cli --example grobid_stage_tape -- --verify <tape-dir-beyond-repo-root>/tape.json

# 3. Измерение: --tape можно повторять (ленты сопоставляются документу по digest, затем по пути);
#    --verify пере-измеряет каждую ленту до приёма её counts.
cargo run -p da-cli --example native_java_work -- \
  --tape <tape-dir-beyond-repo-root>/tape.json --verify
# → artifacts/parse_compat/native_java_work.json

# 4. Offline-плечо (без измерения): артефакт всё равно пишется, каждый документ not_observed,
#    в байтах НЕТ ни одного поля с числами, exit 0.
cargo run -p da-cli --example native_java_work

# 5. Опциональная integration-конечность (только при явно заданном env), MISSING-полярность:
GROBID_URL=<explicit-url> cargo test -p da-cli --test native_java_work_real_grobid
```

### Правила, которые держат измерение честным

1. **Сырой дамп никогда не коммитится** и никогда не читается, не пишется и не встраивается
   модулем: в артефакт из дампа попадают только имена моделей. Проверка:
   `find . -name '*.debug.txt' -not -path './.git/*' | wc -l` → `0`; redaction-ассерты артефакта.
2. **Отсутствие измерения — типизированное `not_observed` без числовых полей**, никогда не ноль
   и никогда PASS: `JavaWorkMeasurement::NotObserved` структурно не имеет `value`/`invocations`
   (governor-правило `zero_count_from_absence` не задевается).
3. **Покрытие читается из `models_filter`**, а не из `capture_state`: `filtered`-лента всегда
   `partial`, а запрос полного покрытия — типизированный отказ.
4. **Отказ вместо понижения:** `failed`/`uncaptured_empty`/нулевые секции/несогласованные counts —
   типизированные отказы, а не «почти измерение».
5. **Привязка к ленте:** receipt несёт `tape_identity` (sha256 от schema|endpoint|source_sha256|
   dump_sha256|backend_version) и `tape_verification` (`verified`/`not_observed`) с привязкой к тем
   же полям. Подсчёт ленты ≠ её верификация, и прогон с findings отказывает.
6. **Границы типов:** записанный `source_pdf` отвергается типом при абсолютности/траверсинге/
   не-unix/пустоте; имена моделей ограничены `[A-Za-z0-9._-]` и длиной 64; свободный текст ошибок
   и коды записанного сбоя эхо-ются в артефакт только как closed-vocabulary токены.
7. **Ни одно число не хардкодится**: снятые значения живут только в артефакте; тестовые фикстуры
   несут заведомо синтетические counts.

---

## (d) Как перезапустить профиль, SMP и ledger, и что значит каждое поле

### Профиль (`artifacts/parse_compat/native_core_profile.json`)

```bash
cargo run -p da-cli --example native_core_profile            # --rows N меняет число строк на семью
```

| Поле | Значение |
|---|---|
| `schema_version` / `qualification_schema_version` | `native-core-qualification.v1`; `adr_ref: ADR-075` |
| `manifest_binding` | `data/parse/comparison_contract.yaml` — источник порогов и словарей |
| `profile` | процедура: `warmup_iterations`, `repetitions`, `percentile_ranks`, `throughput_units`, единицы аллокатора и private scratch, `scalar_oracle: required`, `selection_forbidden: true` |
| `phases[]` | четыре фазы ядра с `work_kind`/`storage`/`contiguous_numeric_arrays`/`sparse_weight_reads` |
| `families[]` | по семье: sha256 и размер модели, `rows`/`positions`/`matched_observations`, `scalar_oracle_digest`, `semantic_digest`, `cpu_only`, `subprocess_io_included`, scratch capacity/high-water, `dense_numeric_kernel_found`, `dense_phase`, `optimization_state`, `optimization_reason` |
| `allocator` | `state: stable` + `allocations`/`allocated_bytes` (дельта между инжектированными снапшотами); нестабильный replay → `state: unstable` **без чисел** |
| `rule` | `dense-contiguous-numeric-phase`: `share_metric`, `threshold`, `requires_contiguous_numeric_arrays`, `eligible_phases`, `outcome_when_unmet` |
| `disposition` | closed verdict: `dense_numeric_kernel_found`, `dense_phase`, `optimization_state`, `optimization_reason`, `selection_forbidden`, `pgo_disposition` |
| `promotion` | `state: candidate`, `selected/promoted/auto_promote: forbidden`, `rollback_target` |
| `h05_closure` / `h05_reason` | `not_claimed` + причина (потолки и Java CPU не закрываются) |
| `redaction.forbidden` | список запрещённого содержимого (`raw_pdf`, `raw_text`, `raw_tei`, `token_text`, `model_bytes`, `lexicon_bytes`, `physical_path`, `hostname`, `executable`) |

### SMP: две поверхности (`native_core_smp.json` + `native_core_smp_observations.json`)

SMP — единственный продюсер с двумя поверхностями. **Детерминированная** поверхность — артефакт
приёмки, схема `native-core-smp.v2`, байт-воспроизводима. **Observation**-поверхность — объявленный
run-snapshot, схема `native-core-smp-observations.v1`, `reproducible: false`, `not_a_receipt: true`;
она пинит детерминированные байты и не является артефактом приёмки.

```bash
# детерминированная поверхность + observation-снапшот (без --observations-artifact снапшот
# по умолчанию НЕ перезаписывается, если уже является актуальной записью этих детерминированных
# байтов; --observations-artifact — явный путь принудительного обновления)
cargo run -p da-cli --example native_core_smp \
  --artifact <dir>/native_core_smp.json \
  --observations-artifact <dir>/native_core_smp_observations.json
# --documents/--rows-per-document/--repetitions меняют нагрузку (benchmark-ось), не приёмку
```

#### Детерминированная поверхность (`artifacts/parse_compat/native_core_smp.json`, `native-core-smp.v2`)

| Поле | Значение |
|---|---|
| `schema_version` / `adr_ref` / `candidate_schema_version` / `requirement_ids` | `native-core-smp.v2` / `ADR-075` / версия схемы DATA-кандидата / идентификаторы требований |
| `manifest_binding` / `candidate_manifest_binding` | DATA-манифест порогов и словарей + его кандидат-версия |
| `topology` | источник (linux allowed cpu set ∩ sysfs package/core), `allowed_cpus`, `physical_cores`, digest — потолок `physical_core_cap` |
| `workload` | `documents_per_family`, `rows_per_document`, `jobs`, `repetitions`, `scratch_capacity_bytes` (выводится из модели и нагрузки, не измеряется) |
| `families[]` | по семье: `family`, `model_sha256`, `rows`, `documents` — sha256 модели остаётся на детерминированной стороне |
| `matrix.entries[]` | по числу воркеров `1..=cap`: `admitted`, `terminals`, `completed`, `failed`, `coordinator_failed`, `ordinals`, `per_worker_terminal_counts`, `semantic_digest`, `scalar_oracle_digest`, `semantic_equal_to_scalar`, `rows_decoded`, `decoded_positions`, `queue` (только `input_capacity`/`result_capacity`/`max_in_flight` — конфигурация), `queue_wait_ms` (typed `not_observed` без `value`), `scratch_capacity_bytes`, `cpu_only`, `subprocess_io_included`, `coordinator_failure` |
| `matrix.scalar_oracle_digest` | единственный oracle-digest, с которым обязаны совпасть все worker-count'ы |
| `matrix.breakpoint_vocabulary` | закрытый словарь `[not_observed, saturation, first_regression, queue_pressure, memory_pressure, inconclusive]`; сами вердикты наблюдательны |
| `memory_pressure_ceiling_bytes` | typed `not_observed` (DATA не объявляет потолок) |
| `metrics.required` | обязательные метрики приёмки (перцентили, throughput, process RSS, allocation/private capacity, queue wait, оба high-water) |
| `denominators` | acceptance-знаменатель (`corpus_n`, `lanes_n`, `pages_per_lane`, `total_lane_pages`, `fixed`) и benchmark-знаменатель (`separate_from_acceptance: true`, `pooled: forbidden`) |
| `timing_observational` | `disposition`, `timing_fields`, `runtime_observed_fields`, `excluded_from`, `observations_artifact`, `observations_schema_version` — декларация границы (имена, а не значения) |
| `deterministic_projection.digest` / `.scope` | персистированный `sha256:`-digest проекции и группы, которые она покрывает; дайджест ≠ sha256 байтов артефакта (нет самореференции) |
| `optimization` / `promotion` / `selection_forbidden` / `h05_closure` / `redaction` | те же закрытые состояния, что и в профиле |

#### Observation-поверхность (`artifacts/parse_compat/native_core_smp_observations.json`, `native-core-smp-observations.v1`)

| Поле | Значение |
|---|---|
| `schema_version` / `adr_ref` | `native-core-smp-observations.v1` / `ADR-075` |
| `source_artifact.path` / `.sha256` | путь детерминированной поверхности и sha256 **её байтов**: снапшот привязан ровно к той записи, рядом с которой снят |
| `reproducible: false` / `non_reproducible_reason` | wall-clock и process-runtime гейджи по построению различаются между прогонами; заявка на воспроизводимость снята |
| `not_a_receipt: true` | это run-snapshot, а не артефакт приёмки |
| `excluded_from` | `acceptance_denominators`, `production_route_selection` |
| `matrix.entries[]` | `latency_ms` (p50/p95/p99), `run_wall_ms`, `throughput_units_per_second`, `queue.{input,result,in_flight}_high_water`, `scratch_high_water_bytes`, `per_worker_scratch_high_water_bytes` |
| `matrix.breakpoints[]` | по числу воркеров вердикт из закрытого словаря + причина |
| `peak_rss_bytes` | наблюдательный process-гейдж (typed `observed`/`not_observed`; `not_observed` без `value`) |

### Java-работа (`artifacts/parse_compat/native_java_work.json`)

```bash
cargo run -p da-cli --example native_java_work -- --tape <tape>.json [--tape <tape>.json] --verify
# --artifact <rel-path> переопределяет путь вывода (по умолчанию — закоммиченный артефакт)
```

| Поле | Значение |
|---|---|
| `unit` | `java_model_invocations` |
| `aggregate` | `measurement_state`, `documents_measured`/`documents_filtered`/`documents_refused`/`documents_total`, отсортированные per-model counts, `total_invocations`, `coverage` |
| `corpus` | `id`, `corpus_n`, `page_denominator`, `documents[]` (`id`, `arxiv_id`, `revision`, repo-relative `source_pdf`, `sha256`, `pages`) |
| `documents[].receipt` | по документу: `measurement_state`, `source_pdf`, `source_sha256`, `backend_version`, `endpoint`, `capture_state`, `dump_bytes`, `dump_sha256`, `sections`, `distinct_models`, per-model counts, `total_invocations`, `coverage`, `tape_verification`, `tape_identity` |
| `locks` | `deployed: false`, `import_eligible: false`, `production_route/auto_promote/selection: forbidden` |
| `observations` | `java_cpu_wall_clock` и `rss_ceiling` — типизированные `not_observed` |
| `capture_hint` | точная команда пересъёмки (та же, что в §(c)) |
| `verification_requested` / `verification_gaps` / `unmatched_tapes` / `unreadable_tape_count` | полнота верификации и отсутствие «тихого выпадения» документов |
| `h05_closure` / `h05_reason` / `redaction` | `not_claimed` + причина и запрещённое содержимое |

### Реестр маршрутов (`artifacts/parse_compat/native_route_recovery.json`)

```bash
cargo run -p da-cli --example native_route_recovery     # --root . --artifact <rel-path>
```

| Поле | Значение |
|---|---|
| `stages[]` | по стадии P0..P9: `java_stage`, `production_authority`, `native_surface` + `native_surface_state`, `components[]` (matrix/receipt disposition), `conditions` (четыре условия допуска), `recovery_rule`, `java_models_observed[]`, `attributed_java_work`, `verdict`, `verdict_reason` |
| `verdict_rule` | `order` (порядок guard'ов), `component_state_verdicts`, `admission_conditions`, `recovery_rule_invariants`, `attribution_rule` |
| `java_work` | входной receipt: `state`, `unit`, `corpus_documents`, `documents_measured`, `coverage`, `tape_verification`, `total_invocations`, per-model counts |
| `recoverable_java_work` | `state`, `unit`, `value`, `by_stage`, `corpus_id`, `receipt_sha256`, `claim: offline_attribution_only_never_a_deployed_reduction` |
| `core_qualification` | `profile_receipt_present`, `profile_optimization_state`, `profile_dense_phase`, `profile_pgo_disposition`, `smp_receipt_present`, `smp_matrix_entries`, `smp_scalar_oracle_bound`, `smp_deterministic_projection_sha256` (прочитан из запиненных байтов receipt'а), `missing_receipts` |
| `observations` | `java_cpu_wall_clock` / `rss_ceiling` — `not_observed` |
| `denominators` | `corpus_id`, `corpus_documents`, `corpus_documents_pinned_by_sha256`, `unit`, `pooled: forbidden`, `named_gaps` |
| `source_bindings[]` | sha256 всех входов: три артефакта-рецепта и три DATA-таблицы |
| `named_limitations` | явные ограничения (двойное использование модели `date`, `fulltext` → P5, «вердикты ничего не диспатчат», fallback не переклассифицируется, H05 не закрыт) |
| `locks` / `h05_closure` / `redaction` | лок-блок (см. §(f)), `not_claimed`, запрещённое содержимое |

Четыре артефакта приёмки (профиль, **детерминированная** поверхность SMP, Java-работа, реестр
маршрутов) воспроизводимы: повторный прогон эмиттера на тех же входах даёт байт-идентичный файл,
а тесты сверяют закоммиченные байты с повторной эмуляцией. Пятый файл — observation-снапшот SMP —
намеренно **не** воспроизводим и не является артефактом приёмки: он объявлен
`reproducible: false` / `not_a_receipt: true`, пинит детерминированные байты и исключён из
знаменателей приёмки и из выбора production-маршрута.

---

## (e) Названные пробелы

1. **Dense numeric kernel найден, но не адоптирован.** DATA-правило
   `dense-contiguous-numeric-phase` (порог 0.25, допустимая фаза `viterbi_decode`, обязательные
   contiguous numeric array) даёт `dense_numeric_kernel_found=true`, поэтому закрытая диспозиция —
   `measured_rejected`, а не `not_applicable`. Это rejection в смысле «кандидат не реализован и не
   измерялся» (`candidate=none_implemented`), и его **нельзя** читать как утверждение о
   SIMD-пригодности декодера: декодер читает разреженную weight-map. `measured_retained`
   структурно недостижим: он требует DATA-порогов (`minimum_dense_phase_share`,
   `minimum_repeated_cpu_improvement_ratio`, `maximum_full_pipeline_p95_regression_ratio`,
   `requires_scalar_equivalence`) и запрещён к выбору (`selection: forbidden`,
   `safe_rust_only: true`). Измеренный hot spot `map_input` остаётся открытым.
2. **PGO не адоптирован.** `pgo.disposition = not_adopted`; для адоптации обязаны выполниться все
   четыре `pgo.adoption_requires` (`profile`, `scalar_equivalence`,
   `full_pipeline_p95_non_regression`, `data_thresholds`). Кандидат не собирался, поэтому измерять
   было нечего; выбор запрещён (`selection: forbidden`). Адоптация — отдельное решение, не
   следствие этого среза.
3. **Семантика `not_observed`.** Отсутствие измерения — типизированное состояние **без числовых
   полей**, и оно **никогда** не читается как ноль: offline-плечо пишет артефакт с exit 0 и без
   ключей counts; `not_observed` Java-receipt делает все стадии `not_observed` и не публикует сумму;
   `queue_wait_ms` и `memory_pressure_ceiling_bytes` несут typed отсутствие с причиной, а не `0`.
4. **H05 открыт и без владельца.** Per-stage Java CPU wall-clock, RSS-потолки и byte-потолки этим
   срезом не закрываются и не объявляются. Измерение CPU на живом сервисе требует отдельной
   политики измерения; до неё любые потолки остаются не-заявленными.
5. **H13 покрыт частично.** S08 отдаёт нативную invocation/SMP/profile-часть, но latency-хвосты,
   RSS, queue age и recovery/rollback остаются `not_observed`; H13 зависит от незанятого H05,
   поэтому его запись в horizon-реестре остаётся `pending` и PASS по H13 не заявляется.
6. **`queue_wait_ms` не инструментирован.** Общий bounded runtime публикует только high-water
   гейджи; per-unit queue residence не измеряется и не фабрикуется — типизированное `not_observed`
   с причиной.
7. **`memory_pressure` breakpoint ненаблюдаем**: DATA `cpu_smp_candidate` не объявляет memory
   ceiling, поэтому вердикт `memory_pressure` не может быть вычислен и не вычисляется.
8. **Тайминги наблюдательны.** `latency_ms`, `run_wall_ms`, `throughput_units_per_second` и
   выведенные из них breakpoint-вердикты исключены из знаменателей приёмки и из выбора
   production-маршрута (`timing_observational`). Приёмочно значимы только семантические дайджесты,
   консервационные счётчики и типизированные состояния.
9. **Знаменатель мал: 3 документа / 202 страницы.** 10× корпус линейно масштабирует offline-join,
   но не измерен; пулинг запрещён (`denominators.pooled: forbidden`), а названные gap'ы
   (`citation_names = 2198`, `references = 207`) обязаны остаться раздельными ключами — слияние
   структурно отвергается (`NamedGapDrift`).
10. **Расположение сырого дампа защищено процедурой, а не механикой (advisory).** Рекордер
    принимает произвольный `--out`, а `.gitignore`-пина на `*.debug.txt` и fail-closed отказа при
    `--out` внутри репозитория нет. S08 держит дамп вне репозитория по процедуре (§(c), п.1);
    механический пин — рекомендованная отдельная работа вне S08.
11. **Паритет redaction-словарей (advisory, нит).** В block `native_core_qualification` список
    `redaction.forbidden` не содержит `secret`, тогда как sibling-блоки (`native_visual_families`,
    `route_recovery`) его содержат; это расхождение формулировок, а не утечка (артефакт проверен
    независимо по байтам).
12. **Инструментальные, вне S08:** GitNexus-индекс отстаёт и не резолвит новые символы S08;
    governor-траектория несёт предсуществующие/средовые флаги (диск, `target/`, hardcoded URL в
    `crates/da-parse/src/native_feature_facts.rs`), ни один не связан с S08.

### Устранённый дефект: ложный claim байт-воспроизводимости SMP (закрыт в S09)

S08 объявлял `native_core_smp.json` воспроизводимым артефактом (`sha256 e2246ac3…`, 45025 байт) и
утверждал, что повторный прогон эмиттера на тех же входах даёт байт-идентичный файл. Это было ложное
утверждение, и именно оно породило дефект S08: детерминированная поверхность физически несла
измерения, которые не могут быть воспроизводимы. Ни один семантический результат S08 при этом не
пересматривается — консервация, семантические дайджесты и закрытые состояния сохранены; исправлена
только граница claim'а.

| Почему claim был ложным | Чем закрыт |
|---|---|
| артефакт нёс wall-clock/queue/scratch/RSS-поля (`latency_ms`, `run_wall_ms`, `throughput_units_per_second`, high-water marks, `peak_rss_bytes`) прямо на поверхности приёмки | эти поля вынесены на отдельную объявленную observation-поверхность (`native-core-smp-observations.v1`, `reproducible: false`, `not_a_receipt: true`); детерминированная поверхность (`native-core-smp.v2`) несёт только дайджесты, консервацию и типизированные состояния |
| `DeterministicEntry` нёс два поля, объявленных observational, — имя записи не соответствовало её содержимому | запись сужена до детерминированных полей; `queue_wait_ms` осталась типизированным `not_observed` без `value`, `queue` несёт только `input_capacity`/`result_capacity`/`max_in_flight` |
| guard был подстрочным (`!text.contains("latency") && !text.contains("throughput")`) и не видел значение на пути, чьё имя не содержит токена | leaf-level перечисление путей с re-rooting'ом в namespace проекции: `deterministic_artifact_has_no_observational_leaf_path`, `perturbing_every_observational_field_leaves_the_deterministic_bytes_unchanged` (25 пертурбаций: детерминированные байты не двигаются, observation-байты обязаны двигаться) |
| breakpoints расходились по verdict между прогонами, оставаясь на поверхности приёмки | вердикты и причины перенесены на observation-поверхность; на детерминированной осталась только закрытая `matrix.breakpoint_vocabulary` |
| приёмка и реестр не могли отличить детерминированную поверхность от наблюдаемой | `deterministic_projection.digest` персистится и пересчитывается тестом; реестр публикует `core_qualification.smp_deterministic_projection_sha256`, прочитанный из запиненных байтов; observation-путь отсутствует и в `source_bindings`, и в DATA-списке receipt'ов, причём детерминированный там присутствует (положительная полярность) |

Отвергнутая альтернатива — удалить observation-поверхность целиком. Она отвергнута, потому что
наблюдения нужны для диагностики, а удаление лишило бы доказательство положительной полярности:
исключение поля из детерминированной поверхности доказывается парой «байты не двигаются / байты
обязаны двигаться на observation-стороне», и без второй половины leaf-guard может быть вакуумным
(такой вакуумный guard и был найден и исправлен мутационным прогоном в S09). Отвергнута и
«воспроизводимость» наблюдаемой поверхности: wall-clock нельзя сделать воспроизводимым, не выбросив
само измерение.

---

## (f) Локи, права и rollback target

| Лок | Значение | Где закреплён |
|---|---|---|
| `production_route` | `forbidden` | `route_recovery.locks`, `scripts/governor/checks/scholarly_parse_plane.yaml#locks`, артефакты |
| `deployed` | `false` | там же |
| `import_eligible` | `false` | там же (D127) |
| `auto_promote` | `forbidden` | там же |
| `graph_mutation` | `forbidden` | `route_recovery.locks` |
| `selection` | `forbidden` | профиль/SMP/Java-работа/реестр |
| `promotion.state` / `selected` / `promoted` | `candidate` / `forbidden` / `forbidden` | профиль и SMP артефакты |
| `decision_point` | `6` | `route_recovery.locks` + DATA-таблицы каскада |
| `independently_selectable` | ровно `[header, references]` | `admission`, `grobid_compatibility.yaml`, `route_recovery.locks` |
| `rollback_target` | `java-grobid-http@0.9.0-crf` | `promotion` в профиле/SMP и `rollback` в compatibility-таблице |
| `h05_closure` | `not_claimed` | все четыре артефакта + DATA-блоки |

**Ровно шесть DecisionPoints** — закрытый enum `crates/da-domain/src/fsm_policy.rs`:
`GrammarRoute, BatchAdmission, ConcurrencyTune, RetryPolicy, SummaryDepth, ProfileRoute`.
S08 не добавил ни одного; `parse-pdf` по-прежнему консультирует только
`[grammar_route, profile_route]`.

**Никаких новых DATA-файлов.** `data/parse` остаётся ровно **24** санкционированными YAML
(D377 — только sibling-блоки `comparison_contract.yaml#native_core_qualification` и
`native_fulltext_composition.yaml#route_recovery`).

**Харнесс — в composition root.** Все четыре модуля живут в `crates/da-cli` (D380/R138); их
имена отсутствуют в `crates/da-ports`, `crates/da-parse/src`, `crates/da-application/src`,
`crates/da-domain/src` и `crates/da-cli/src/source_cli.rs` (grep = 0), то есть инверсии
onion-зависимостей нет и на пути `ParserPort` они не появляются.

**Права и payload.** Новых модельных/лексиконных байтов и изображений не введено; артефакты несут
только счётчики, дайджесты и ссылки на repo-relative пути. `redaction.forbidden` объявлен в каждом
артефакте; `h05_closure` не читается как измеренный ноль. Ни один артефакт не является источником
graph-truth и не может быть заимпортирован.

**Почему сканер маркеров композера не считает реестр сплайсом.** Архитектурный контракт
`native_fulltext_composition_is_pure_and_stays_outside_every_runtime_route`
(`crates/da-parse/tests/architecture_contract.rs`) сканирует runtime-каталоги, включая
`crates/da-cli/src`, на маркеры композера (`NativeFulltextComposition`, `compose_native_fulltext`,
`native_fulltext_composition`) и делает исключение только для самой поверхности композера и
`da-parse/src/lib.rs`. Имя санкционированной DATA-таблицы содержит имя композера, поэтому сканер
вырезает ровно этот repo-relative путь (`data/parse/native_fulltext_composition.yaml`) до сравнения:
чтение схемо-валидированной таблицы — конфигурационная зависимость, а не проводка композера, и её
sha256 уже связан в `source_bindings` реестра. Маркеры-идентификаторы и API-имена по-прежнему роняют
сканер безусловно — включая строку, где встречаются и путь, и идентификатор (положительный и
отрицательный контроль: `fulltext_composition_scanner_ignores_the_data_path_but_never_the_identifiers`).
Сам факт отсутствия сплайса остаётся доказанным отдельно: реестр не вызывается ни из
`source_cli.rs`, ни с любого пути `ParserPort`.

Проверка локов (перезапускается оператором; каждый шаг — ассерт, дающий 0 ровно при выполненном
условии). Шаги 1–7 — S08-инварианты, шаги 8–11 — S09-граница двух поверхностей: детерминированная
поверхность объявляет projection-дайджест и не пересекается по leaf-путям с observational-именами,
observation-поверхность объявлена run-snapshot'ом, пинит детерминированные байты и отсутствует и в
DATA-списке receipt'ов, и в `source_bindings` реестра:

```bash
python3 - <<'PY'
import hashlib, json, subprocess

def sh(cmd):
    return subprocess.run(cmd, shell=True, capture_output=True, text=True).stdout

profile = json.load(open("artifacts/parse_compat/native_core_profile.json"))
smp = json.load(open("artifacts/parse_compat/native_core_smp.json"))
java = json.load(open("artifacts/parse_compat/native_java_work.json"))
rec = json.load(open("artifacts/parse_compat/native_route_recovery.json"))

# 1. H05 не объявлен ни одним артефактом
for name, doc in (("profile", profile), ("smp", smp), ("java_work", java), ("recovery", rec)):
    assert doc["h05_closure"] == "not_claimed", name

# 2. промоушен и выбор запрещены в артефактах
for doc in (profile, smp):
    assert doc["promotion"]["selected"] == "forbidden"
    assert doc["promotion"]["promoted"] == "forbidden"
    assert doc["promotion"]["auto_promote"] == "forbidden"
    assert doc["promotion"]["rollback_target"] == "java-grobid-http@0.9.0-crf"
# выбор запрещён в обоих артефактах, но поле живёт в разных местах по схеме v1:
# в профиле — внутри `profile` и `disposition`, в SMP — top-level
assert profile["profile"]["selection_forbidden"] is True
assert profile["disposition"]["selection_forbidden"] is True
assert smp["selection_forbidden"] is True
assert profile["disposition"]["pgo_disposition"] == "not_adopted"
assert java["locks"] == {"deployed": False, "import_eligible": False,
                         "production_route": "forbidden", "auto_promote": "forbidden",
                         "selection": "forbidden"}
assert rec["locks"] == {"production_route": "forbidden", "deployed": False,
                        "import_eligible": False, "auto_promote": "forbidden",
                        "graph_mutation": "forbidden", "selection": "forbidden",
                        "decision_point": 6, "independently_selectable": ["header", "references"]}

# 3. ровно шесть DecisionPoints
enum = sh("sed -n '/pub enum DecisionPoint/,/^}/p' crates/da-domain/src/fsm_policy.rs")
assert len([l for l in enum.splitlines() if l.strip().endswith(",")]) == 6

# 4. санкционированное дерево DATA не расширено
assert int(sh("ls data/parse/*.yaml | wc -l").strip()) == 24

# 5. ни один эмиттер не на пути ParserPort / production dispatch
hits = sh("grep -rl 'native_core_profile\\|native_core_smp\\|native_java_work\\|"
         "native_route_recovery' crates/da-ports crates/da-parse/src "
         "crates/da-application/src crates/da-domain/src "
         "crates/da-cli/src/source_cli.rs | wc -l")
assert int(hits.strip()) == 0

# 6. сырой debug-дамп не попал в репозиторий
assert int(sh("find . -name '*.debug.txt' -not -path './.git/*' | wc -l").strip()) == 0

# 7. маркеры композера не появляются в runtime-корнях (сама поверхность и её ре-экспорт исключены)
composer = sh("grep -rn 'NativeFulltextComposition\\|compose_native_fulltext' "
              "crates/da-ports/src crates/da-parse/src crates/da-application/src "
              "crates/da-adapters/src crates/da-cli/src | "
              "grep -v 'da-parse/src/native_fulltext_composition.rs' | "
              "grep -v 'da-parse/src/lib.rs' | wc -l")
assert int(composer.strip()) == 0, composer

# 8. детерминированная поверхность: проекция объявлена, дайджест формы `sha256:<64 hex>`
#    и совпадает с дайджестом, который реестр прочитал из запиненных байтов этого же receipt'а
proj = smp["deterministic_projection"]
digest = proj["digest"]
assert digest.startswith("sha256:"), digest
assert len(digest) == 71, len(digest)
assert all(c in "0123456789abcdef" for c in digest[7:]), digest
assert digest == rec["core_qualification"]["smp_deterministic_projection_sha256"], digest
assert isinstance(proj["scope"], list) and proj["scope"], proj["scope"]

# 9. leaf-level: ни один leaf-путь детерминированной поверхности не лежит под observational-именем.
#    Имена берутся из декларации, а не из подстроки, и сравниваются на границе сегментов.
obs = json.load(open("artifacts/parse_compat/native_core_smp_observations.json"))
observational = (list(smp["timing_observational"]["timing_fields"])
                 + list(smp["timing_observational"]["runtime_observed_fields"])
                 + ["peak_rss_bytes"])

def leaf_paths(value, path=""):
    if isinstance(value, dict):
        out = set()
        for key, item in value.items():
            out |= leaf_paths(item, f"{path}.{key}" if path else key)
        return out
    if isinstance(value, list):
        out = set()
        for item in value:
            out |= leaf_paths(item, f"{path}[]")
        return out
    return {path}

def is_under(path, name):
    norm = lambda p: p.replace("[]", ".__item")
    return norm(path) == norm(name) or norm(path).startswith(norm(name) + ".")

artifact_leaves = leaf_paths(smp)
assert artifact_leaves, "leaf enumeration is empty"
offenders = sorted(l for l in artifact_leaves for n in observational if is_under(l, n))
assert not offenders, offenders
# невакуумность: тот же предикат обязан поймать эти имена на observation-поверхности
matched = sorted(n for n in observational if any(is_under(l, n) for l in leaf_paths(obs)))
assert matched, "no observational name resolves anywhere: disjointness would be vacuous"

# 10. observation-поверхность объявлена run-snapshot'ом и пинит ровно детерминированные байты
with open("artifacts/parse_compat/native_core_smp.json", "rb") as fh:
    deterministic_sha256 = "sha256:" + hashlib.sha256(fh.read()).hexdigest()
assert obs["schema_version"] == "native-core-smp-observations.v1"
assert obs["reproducible"] is False
assert obs["not_a_receipt"] is True
assert obs["source_artifact"]["path"] == "artifacts/parse_compat/native_core_smp.json"
assert obs["source_artifact"]["sha256"] == deterministic_sha256, deterministic_sha256
assert obs["excluded_from"] == ["acceptance_denominators", "production_route_selection"]
assert smp["timing_observational"]["observations_artifact"] == \
    "artifacts/parse_compat/native_core_smp_observations.json"
assert digest != deterministic_sha256, "projection digest must not be a self-reference"

# 11. observation-поверхность не receipt: её нет ни в DATA-списке receipt'ов, ни в source_bindings
#     реестра, тогда как детерминированная поверхность там присутствует
receipts = open("data/parse/native_fulltext_composition.yaml", encoding="utf-8").read()
assert "artifacts/parse_compat/native_core_smp.json" in receipts
assert "artifacts/parse_compat/native_core_smp_observations.json" not in receipts
binding_paths = [b["path"] for b in rec["source_bindings"]]
assert "artifacts/parse_compat/native_core_smp.json" in binding_paths
assert "artifacts/parse_compat/native_core_smp_observations.json" not in binding_paths

print("S08 LOCKS OK")
print("S09 SURFACE LOCKS OK")
PY
```

---

## (g) Integration-граница

Класс Integration этой волны **owned и ограничен**, а не отсутствует по умолчанию: он объявлен как
явно env-gated граница, единственным офлайн-доказательством которой служит типизированная
MISSING-полярность харнессов реальных моделей. Решение записано как **D385**
(`integration-verification`): «Integration verification class for the GROBID-to-Rust wave is an
explicitly owned, env-gated boundary with typed MISSING as the only offline evidence».

### G1. Что именно проверяемо офлайн

Офлайн-доказательство состоит ровно в одном: при непровизионированном ресурсе каждый харнесс
печатает `MISSING` и **никогда** не заявляет PASS — MISSING-плечо возвращается до любого
assert'а, поэтому отсутствующий сервис или отсутствующие модельные байты не могут быть записаны
как зелёный integration-результат. Модельные байты не коммитятся и не обнаруживаются неявно
(D358: ресурсы GROBID не подлежат редистрибуции); харнесс читает caller-owned путь из окружения
и сообщает явную отсутствующесть.

| Харнесс | Гейт-переменная |
|---|---|
| `crates/da-adapters/tests/native_header_real_model.rs` | `DA_GROBID_CRF_MODEL_PATH` |
| `crates/da-adapters/tests/scalar_crf_real_model.rs` | `DA_GROBID_CRF_MODEL_PATH` |
| `crates/da-adapters/tests/native_bibliography_real_model.rs` | `DA_GROBID_MODEL_ROOT` |
| `crates/da-adapters/tests/native_fulltext_real_model.rs` | `DA_GROBID_MODEL_ROOT` |
| `crates/da-adapters/tests/native_visual_family_real_model.rs` | `DA_GROBID_MODEL_ROOT` |
| `crates/da-cli/tests/native_java_work_real_grobid.rs` | `DA_GROBID_URL` (alias `GROBID_URL`) |

Наблюдаемая MISSING-полярность — дословные строки прогона без env; каждый харнесс завершился
exit 0, ни один assert в MISSING-плече не исполнялся:

```text
# native_header_real_model / scalar_crf_real_model (DA_GROBID_CRF_MODEL_PATH)
MISSING: DA_GROBID_CRF_MODEL_PATH is not configured; no real GROBID header model claim was made
MISSING: DA_GROBID_CRF_MODEL_PATH is not configured; no real GROBID model claim was made

# native_bibliography_real_model / native_fulltext_real_model (DA_GROBID_MODEL_ROOT)
MISSING: DA_GROBID_MODEL_ROOT is not configured; no real GROBID bibliography model claim was made
MISSING: DA_GROBID_MODEL_ROOT is not configured; no real GROBID fulltext model claim was made

# native_visual_family_real_model (DA_GROBID_MODEL_ROOT)
MISSING: DA_GROBID_MODEL_ROOT is not configured; no real GROBID visual/funding model claim was made

# native_java_work_real_grobid (DA_GROBID_URL / GROBID_URL)
MISSING: set DA_GROBID_URL or GROBID_URL to enable the live capture; no assertion runs and no PASS is claimed
```

Полярность перепроверяема одной командой на харнесс (все шесть — из корня репозитория):

```bash
cargo test -p da-adapters --test native_header_real_model -- --nocapture
cargo test -p da-adapters --test scalar_crf_real_model -- --nocapture
cargo test -p da-adapters --test native_bibliography_real_model -- --nocapture
cargo test -p da-adapters --test native_fulltext_real_model -- --nocapture
cargo test -p da-adapters --test native_visual_family_real_model -- --nocapture
cargo test -p da-cli --test native_java_work_real_grobid -- --nocapture
```

### G2. Что отложено и не заявлено

Живой прогон — реальные модельные байты GROBID и JVM same-n сравнение — требует
провизионированного модельного корня **вне репозитория** и поднятого сервиса. До этого:

- ни один артефакт среза не несёт claim'а integration-покрытия: `not_observed`, `MISSING` и
  отсутствие ресурса — три разных типизированных состояния, и ни одно не читается как PASS;
- `import_eligible` остаётся `false` (D127), `production_route` — `forbidden`, `deployed` —
  `false`; локи §(f) этой секцией не ослабляются;
- same-n рамки, синтетические или TEI→atoms, не считаются integration-доказательством: они
  проверяют согласованность кода, а не поведение реальной модели.

### G3. Почему это честная граница, а не пробел

- MISSING-полярность машинно-проверяема: она — исполняемая негативная проверка (G1), а не
  обещание в тексте. Её наличие доказывает отсутствие ложного claim'а именно потому, что
  PASS-ветка недостижима без провизионированного ресурса.
- Граница типизирована: D385 фиксирует и саму границу, и условие, при котором живой прогон
  станет допустимым доказательством (провизионированный не-repo модельный корень), поэтому
  состояние «integration не измерен» читается как объявленное, а не как необнаруженный пробел.
- Никаких исходников, артефактов и DATA-файлов эта секция не меняет: она описывает
  существующую границу и наблюдаемую полярность, фиксируя non-claim там, где раньше было
  молчание.

---

## Проверки этого документа

```bash
test -s doc/parse/native-performance-recovery.md
# нет абсолютных физических путей: три запрещённых префикса отсутствуют (0 совпадений)
! grep -qE '/(root|home|tmp)/' doc/parse/native-performance-recovery.md
PYTHONPATH=scripts python3 -m governor --validate-yaml            # PASS
PYTHONPATH=scripts python3 -m governor --phase preflight          # violations=0
PYTHONPATH=scripts python3 -m governor architecture               # ARCHITECTURE GATE: PASS
cargo test -p da-parse --tests                                    # data/architecture контракты зелёные
```
