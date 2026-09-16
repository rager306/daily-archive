# Policy-variant ablation S07: границы приёмки и известные пробелы

**Milestone:** M507-4fk6m6 · **Slice:** S07 «Learning policy с независимой оценкой»
**Нормативная база:** R119 (policy → dispatch), R110/R123/R131 (candidate-ablation),
R118/R125/R130/R138 (closure), ADR-051 (oracle ≠ SUT), ADR-074 (FSM policy), D358/D137 (права).

## Назначение

Документ фиксирует **границу приёмки** среза S07: что именно доказано исполняемыми
командами, чего срез **не** доказывает, как устроены продюсеры и метрики ablation,
какие closure-инварианты обязаны остаться закрытыми и какие пробелы остаются открытыми.
Ни одно утверждение ниже не выходит за пределы проверенного в S07; там, где доказательства
нет, это сказано прямо, а не подразумевается.

Все команды выполняются из корня репозитория.

---

## (a) Что доказывает

Срез доказывает три утверждения. Каждое подтверждено командами с ассертами, а не прозой.

### A1. Смена policy-артефакта меняет фактическую легальную диспетчеризацию (R119)

Единая точка композиции — `pub fn resolve_parsed_dispatch(paper_id) -> Option<(ParseGrammarDecision, &'static str)>`
в `crates/da-cli/src/source_cli.rs`; продакшен-путь `run_pipeline_for` делегирует ей
(parsed-stage: `consult_parse_grammar_cli` → `toc_observations_path(...).exists()` →
`choose_parsed_action`). Тесты гоняют **тот же** селектор на одних и тех же features
(`pages=18`, `bookmarks=0`, `quality=0.85`), подменяя только исполняемый WAT-модуль
через temp-registry (`DA_POLICY_REGISTRY`); committed `data/policies/registry.yaml` не правится.

| Артефакт политики | `grammar_route` | `grammar_profile` | Выбранное действие |
|---|---|---|---|
| `data/policies/fixtures/grammar_route.wat` | `xy_cut` | `arxiv_pdf_dot` | `parse-pdf` |
| `data/policies/fixtures/grammar_route_always_page_outline.wat` | `page_outline` | `page_outline` | `parse-pdf-toc` |
| `data/policies/fixtures/grammar_route_always_grobid.wat` | `grobid_structural` | `arxiv_pdf_dot` | `parse-pdf` |

`policy_hash` всех трёх вариантов попарно различаются, `policy_source = Wasm`.
`policy_hash` — это sha256 **скомпилированного** WASM-модуля (`wat::parse_bytes`), то есть
тех байт, которые исполняет wasmi; равенство с sha256 сырого текста `.wat` отвергается
негативным контролем в том же тесте.

```bash
cargo test -p da-cli --lib source_cli::tests
# 29 passed; 0 failed — включая:
#   policy_variant_changes_parsed_dispatch
#   policy_hash_binds_executed_bytes
#   unknown_route_code_fails_closed
#   stamp_from_grammar_records_variant_provenance
#   missing_or_malformed_policy_registry_yields_no_policy_dispatch
```

Поведение продакшена при этом не менялось: число consult'ов `grammar_route` на parsed-stage
осталось 1, новых DecisionPoints и новых consult'ов не добавлено (см. §(d)).

### A2. Вариант политики оценивается как candidate независимыми продюсерами (R110/R123/R131)

Оракул **не строится тем же кодом, что SUT** (ADR-051, pitfall #1). SUT-free reference producer
живёт в `crates/da-cli/src/policy_ablation_reference.rs`: перевыводит *декларированное* действие
из DATA реестра (`grammar_route.profile_map` / `default_profile` / `native`-пороги) и case-DATA,
не загружая policy-модуль, не консультируя decision point, не импортируя `da-adapters` и не
запуская процессов. Независимость — не декларация, а исполняемый скан собственного исходника
с положительным контролем на сам сканер.

```bash
cargo test -p da-cli --lib policy_ablation_reference
# 14 passed; 0 failed — включая:
#   reference_producer_source_is_free_of_sut_tokens
#   scanner_actually_detects_an_injected_token
#   frozen_case_expectations_match_the_declared_baseline_policy
#   fail_closed_dispatch_stays_in_the_denominator_with_zero_metrics

cargo test -p da-application --lib parse_policy_ablation
# 11 passed; 0 failed — агрегатор переиспользует типы/инварианты
# parse_learning_evaluation (единый EvaluationError), а не копирует их
```

Durable redacted артефакт `artifacts/parse_learning/policy_variant_ablation.json`
воспроизводится байт-в-байт и содержит только digest'ы, действия и счётчики:

```bash
cargo test -p da-cli --features learning-evaluation --test policy_ablation_artifact_test
# 2 passed; 0 failed
#   ablation_artifact_is_byte_stable_candidate_only_and_redacted
#   example_refuses_case_data_outside_the_closed_contract

cargo run -p da-cli --features learning-evaluation --example parse_policy_ablation
sha256sum artifacts/parse_learning/policy_variant_ablation.json
# 5bfc3734877b7e86431a1d132ff3f5d9ab61abb913b840584d24ec3ebc3ad4d7
```

Зафиксированные факты артефакта: `n=6` (4 train / 2 validation, split'ы непусты и не пересекаются),
`baseline_n == candidate_n == 6`, `declared_matches=6`, `metric_one` (`legal_dispatch_agreement`)
`1.0 → 1.0`, `metric_two` (`non_fallback_action_rate`) `0.333… → 0.5`,
`observed_routes=["page_outline"]`, `observed_actions=["parse-pdf","parse-pdf-toc"]`,
`producer_independent=true`, `conservation_veto=false`, `report_digest=a2ac655c84f57f8e…`,
`artifact_digest=9599210d8bacb9a0…`, `registry_digest=300b1c2107cfcf68…`.
Closure проверяется **по факту наблюдения**: множество наблюдённых route/action входит в закрытые
множества, а не принимается на веру из декларации.

### A3. Closure не открывается из мутированного варианта политики (R118/R125/R130/R138)

Негативы гоняются через **тот же** продакшен-селектор, а не через мок: temp-registry строится
serde_yaml-раундтрипом committed реестра, так что мутация не пишет committed DATA.

```bash
cargo test -p da-cli --test policy_variant_closure_test
# 6 passed; 0 failed
#   seventh_point_and_later_names_stay_inert
#   unknown_route_code_never_selects_a_policy_action
#   profile_map_remap_stays_inside_the_closed_action_set
#   missing_grammar_route_falls_open_to_parse_pdf
#   policy_variant_report_keeps_the_promotion_locks
#   committed_closure_data_is_untouched
```

Реестр, объявляющий все десять имён как `parse_pdf_consults` и отравляющий каждый point, кроме
`grammar_route`, всё равно диспетчеризует из артефакта `grammar_route`; действие всегда остаётся
в закрытой паре `{parse-pdf, parse-pdf-toc}`, route — в `{page_outline, xy_cut, grobid_structural}`.
Ни один негатив не паникует: отказ типизирован (`FsmPolicyError::InvalidCode(3)`, `None`,
`EvaluationError::CandidateLockViolation`).

---

## (b) Что не доказывает

| Не доказано | Почему / где граница |
|---|---|
| **Никаких perf-чисел.** | S07 не измеряет latency/throughput/качество recovery. Perf recovery и route qualification — **H13, владелец `M507-4fk6m6/S08`**; S07 их не заявляет и не «почти» заявляет. |
| Нет сплайса native-политики в production-route. | Native-политика не подключена к `run_pipeline_for`; governor держит `locks.production_route: forbidden`. S07 ничего не сплайсил: единственное изменение продакшена — вынос существующей композиции в `resolve_parsed_dispatch` без нового поведения. |
| Нет промоушена и повышения. | `promotion_state='candidate'`, `deployed=false`, `import_eligible=false`, `auto_promote='forbidden'` — и в манифесте варианта, и в артефакте; манифест с `deployed=true`/`import_eligible=true` отвергается типизированно. |
| `da source sona-policy` остаётся **demo**, а не closure. | Демо-CLI отказывает для `empty_native_unknown_point` и `later` до consult'а и печатает `demo=true` + `parse_pdf_consult` из `consult_status` (`crates/da-cli/src/background_tasks.rs:487`, MEM930/MEM1052). S07 не «повышает» демо до продакшен-пути. |
| Learned weights в dispatch не проводятся. | Упаковщика MicroLoRA → WASM в репозитории нет; `crates/da-application/src/parse_policy_training.rs` отдаёт тензоры `down_projection`/`up_projection` (`:68–69`), а не исполняемый модуль. Никакой обученный вес не участвует в `choose_parsed_action`. |
| `learning_policy` не «повышается». | Остаётся `not_observed`, `output_affecting: false`, producer `S17` (см. §(e)). |
| Нет новых модельных байтов и изменений прав. | См. §(f). |

Проверка запретов остаётся за тестами и governor'ом, а не за прозой этого документа:
`cargo test -p da-cli --test policy_variant_closure_test` (локи) и
`PYTHONPATH=scripts python3 -m governor --validate-yaml` (locks секции check'а).

---

## (c) Продюсеры и метрики

### Продюсеры

| Арм | Продюсер | Что это | Идентичность |
|---|---|---|---|
| baseline | **SUT-free reference producer** (`da_cli::policy_ablation_reference`) | перевывод *декларированного* действия из DATA (`profile_map` + документированная таблица кодов route) | `code_digest = digest(REFERENCE_PRODUCER_ID)` |
| candidate | **production producer** (`da_cli::source_cli::resolve_parsed_dispatch`) | реальный продакшен-селектор: consult `grammar_route` → TOC-кэш → `choose_parsed_action` | `code_digest = digest(PRODUCTION_PRODUCER_ID)` |

Оба `code_digest` различны и **ни один не совпадает** с `policy_hash` оцениваемого варианта:
агрегатор получает `policy_hash` варианта как self-oracle-пробу и отвергает совпадение
(тест `identical_producer_or_policy_self_identity_is_rejected`). Арм-ы измеряются **разным кодом**:
декларативный пересчёт DATA против реального продакшен-селектора.

### Метрики (dual)

- `legal_dispatch_agreement` — доля case'ов, где действие арма совпадает с декларированным
  легальным действием варианта.
- `non_fallback_action_rate` — доля case'ов, где политика даёт не-fallback действие.

Обе метрики обязательны; знаменатели baseline и candidate обязаны совпадать (`exact same-n`) —
`EvaluationError::DenominatorMismatch` иначе. `split`-квалификация: train- и validation-качества
считаются по `SplitIdentity`, пересечение split'ов отвергается
(`EvaluationError::SplitOverlap`), итог проходит порог `PromotionQualityPolicy`
через `qualify_quality` (Qualified только при validation ≥ минимума и gap ≤ максимума).

### Fail-closed поведение

Fail-closed продакшен-диспатч (`None`) **не выпадает из знаменателя**: он записывается типизированной
строкой `Timeout` с нулевыми метриками, поэтому отбрасывание отказа не может поднять метрику.
Коммитнутый frozen-набор не содержит fail-closed case'а, поэтому в артефакте
`critic_failures=0` — это записано как известный пробел (§(e)), а сам учёт отказа доказан
unit-тестом `fail_closed_dispatch_stays_in_the_denominator_with_zero_metrics`.

---

## (d) Closure-инварианты

| Инвариант | Значение | Доказательство |
|---|---|---|
| DecisionPoints | ровно **6**: `GrammarRoute, BatchAdmission, ConcurrencyTune, RetryPolicy, SummaryDepth, ProfileRoute` | `sed -n '/pub enum DecisionPoint/,/^}/p' crates/da-domain/src/fsm_policy.rs \| grep -cE '^    [A-Z][A-Za-z]+,'` → `6`; `cargo test -p da-parse --test architecture_contract s10_decision_point` → 2 passed |
| `parse_pdf_consults` | ровно `[grammar_route, profile_route]` | `grep -n 'parse_pdf_consults' data/policies/registry.yaml` → `40:  parse_pdf_consults: [grammar_route, profile_route]`; `cargo test -p da-parse --test data_contracts decision` → 1 passed |
| later-имена остаются later | `[head_fallback, visual_bundle, critic_sample]`, не в `DecisionPoint` и не в consult-списке | `grep -n 'later:' data/policies/registry.yaml` → `43`; контракты `s10_decision_point_stays_six_approved_wire_names` + `visual_bundle_stays_later_name_outside_decision_points` |
| `empty_native_unknown_point` | `[concurrency_tune, retry_policy, summary_depth]` | `grep -n 'empty_native_unknown_point' data/policies/registry.yaml` → `41` |
| `batch_admission` не консультируется parse-pdf | `native_rules_not_consulted: [batch_admission]` | `grep -n 'native_rules_not_consulted' data/policies/registry.yaml` → `42`; `PolicyRegistry` не даёт consult'а из parsed-stage |
| `registry.yaml` неизменён | sha256 `300b1c2107cfcf683a17ced994535667ef49b416e54759f54bc9d61fcc376f14` | `sha256sum data/policies/registry.yaml`; тот же хэш закреплён в `scripts/governor/checks/scholarly_parse_plane.yaml#bindings.source_data` и записан в артефакте как `registry_digest` |
| Policy FSM не сломан | 15 + 3 теста зелёные | `cargo test -p da-domain --lib fsm_policy` → 15 passed; `cargo test -p da-adapters --lib wasm_policy` → 3 passed |
| Data-контракты closure | 99 + 92 + 2 теста зелёные | `cargo test -p da-parse --test data_contracts` (99), `--test architecture_contract` (92), `--test native_fulltext_composition_data_contract` (2) |

Ключевая семантика: закрытый `DecisionPoint` **enum** — это и есть рантайм-гейт. DATA-список
декларативен; реестр, объявляющий лишние имена, не расширяет enum и потому не может расширить
консультируемое множество. Именно это проверяет `seventh_point_and_later_names_stay_inert`.

Дополнительно: `PYTHONPATH=scripts python3 -m governor --validate-yaml` → PASS (65 rules, включая
`scholarly_parse_plane`); `PYTHONPATH=scripts python3 -m governor --phase preflight` →
`exit 0`, `violations=0`, `verdict=DRIFT_RISK` с 3 предсуществующими флагами
(S08 без PLAN-задач, `target/` bloat, hardcoded URL в `da-parse`), ни один не связан с S07.

---

## (e) Known gaps

Пробелы записаны явно; ни один не маскируется под доказанное утверждение.

1. **Исполняемые WAT-байты не привязаны в governor.** `bindings.source_data` пинит только
   `data/policies/registry.yaml`; sha256 `data/policies/fixtures/*.wat` нет ни в governor,
   ни в DATA-контрактах, хотя именно эти байты исполняет production consult. Подмена байт по тому
   же пути изменит фактическую диспетчеризацию незаметно для CI.
   *Митигация S07 (runtime-only):* `policy_hash` = sha256 **скомпилированного** модуля
   и тест `policy_hash_binds_executed_bytes`; в артефакте — `candidate_policy_hash` и
   `observed_policy_hashes`. Это честная, но не CI-уровневая привязка. Закрепление в governor —
   отдельное решение (потребовало бы трогать governor fixtures, чего S07 сознательно не делает).
2. **Ablation-артефакт воспроизводим командой, а не governor-pinned.**
   `artifacts/parse_learning/policy_variant_ablation.json` регенерируется
   `cargo run -p da-cli --features learning-evaluation --example parse_policy_ablation`
   и проверяется тестом на байтовую стабильность; sha256 в governor не заводился осознанно
   (чтобы не менять fixture-набор governor в этом срезе).
3. **H13 остаётся unresolved за S08.** Perf/route qualification не измерены и не заявляются;
   ROADMAP Boundary Map и review-артефакт должны трактовать S07 как dispatch-proof,
   а не как H13-владельца (владелец — `M507-4fk6m6/S08`).
4. **`learning_policy` обязан остаться `not_observed`.** В
   `data/parse/native_fulltext_composition.yaml:217` строка
   `learning_policy: {producer: S17, disposition: not_observed, evidence_state: not_observed,
   output_affecting: false, …}` — S07 не «повышает» её и не меняет `output_affecting`.
   Контракт: `cargo test -p da-parse --test native_fulltext_composition_data_contract` (2 passed).
5. **Граница доверия `paper_id` (advisory, пре-существующая).** `resolve_parsed_dispatch`
   идёт в `toc_observations_path(paper_id)` — сырой join без санитайзера
   (`crates/da-cli/src/source_cli.rs:24`), то есть непроверенный `paper_id` даёт
   traversal-*oracle* существования файла. Рядом тот же сырой join у пре-существующих
   `run_parse_pdf_toc` (чтение кэша + запись `data/processed/<paper_id>/…`) и `run_audit_fidelity`.
   Санитайзер в репозитории есть (`pipeline_cache::rejects_path_traversal_components`), но здесь
   не переиспользован. S07 этот путь не вводил и не расширял; граница записана, чтобы не
   выглядеть закрытой. Санитизация — отдельная работа вне S07.
6. **WASM-хост без ресурсных лимитов (advisory, пре-существующее).**
   `da-adapters::wasm_policy` создаёт `Engine::default()` без fuel/epoch/`StoreLimits`:
   незавершающаяся `route`/`start` вешает процесс, `memory.grow` ведёт к OOM. Типизированные
   отказы покрывают trap и `InvalidCode(3)`, но **не** незавершаемость. Пре-существующее
   свойство адаптера, вне диффа S07; остаточный DoS-вектор для доверенного-но-багованного реестра.
7. **Коммитнутый frozen-набор не содержит fail-closed case'а**, поэтому артефакт несёт
   `critic_failures=0`; артефакт-уровневого доказательства fail-closed-пути нет — он доказан
   unit-тестом. Если следующая волна захочет артефактного доказательства, в case-DATA нужен
   намеренно отказывающий case.
8. **Инструментальные, вне S07:** `cargo clippy -p da-cli --all-targets` падает на
   пре-существующем `dead_code` в `crates/da-cli/examples/acceptance_evidence.rs`
   (файл не тронут с 2026-09-05; гейты используют `--lib --tests`, они зелёные);
   GitNexus-индекс отстаёт, поэтому `impact` не резолвит новые символы S07.

---

## (f) Права

- **Новые модельные ассеты не вводились.** S07 не добавляет модельных байтов; `.wat`-фикстуры —
  собственная рукописная policy-DATA (WAT-модуль на несколько строк), а **не** модельные байты.
  Ветка D358/D137 не затрагивается.
- **`data/parse/grobid_model_rights.yaml` не меняется.** Инвентарь прав остаётся как есть:
  `redistributable: false`, `graph_write: forbidden`, `import_eligible: false`,
  `auto_promote: forbidden`, `controls.no_model_bytes_committed: true`,
  `missing_or_ambiguous_rights: fail_closed`. Проверка:
  `grep -n 'no_model_bytes_committed\|auto_promote\|import_eligible' data/parse/grobid_model_rights.yaml`.
- **Локи неизменны** (`scripts/governor/checks/scholarly_parse_plane.yaml#locks`):
  `import_eligible: false`, `production_route: forbidden`, `graph_write: forbidden`,
  `auto_promote: forbidden`, `deployed: false`. Проверка:
  `grep -n 'locks:' -A6 scripts/governor/checks/scholarly_parse_plane.yaml`.
- **Обученные веса не пакетируются и не исполняются.** `parse_policy_training.rs` возвращает
  тензоры (`down_projection`/`up_projection`), упаковщика MicroLoRA → WASM в репозитории нет;
  D136 (`no LoRA weights in this file`) не нарушен.
- **Никаких новых JSONL/SQLite/Redb-авторитетов.** Ablation-артефакт — redacted JSON-отчёт
  (digest'ы, действия, счётчики), не источник graph-truth; `no_sidecar_authority` сохраняется.
