# M513-r7dg4z/S07 — пакет релизного решения по отказу от Java GROBID

Статус пакета: **`pending_human_review`**. Этот документ **подготавливает** решение для
настоящего человека — он его не принимает и не фиксирует.

Ни одно поле `human_release_review` агентом не заполнено; **ни один агент не является
рецензентом**. Живая запись человека остаётся пустой: `reviewer=null`, `decision=pending`,
`prose_alone_authorizes_release=false`. Проза (этот документ, ADR, коммит, зелёный тест)
релиз не авторизует.

Разделы ниже — проверяемая проекция уже замороженных носителей: числа даны
marker-delimited блоками и объявлены контрактом
`crates/da-parse/tests/grobid_retirement_package_doc_contract.rs`, который сверяет каждый
блок с носителем как целочисленное тождество. Поэтому правка предложения или подмена
номера ломает контракт, а не проходит молча.

`GAR-015` остаётся `open`; `H20` остаётся `pending` / `unassigned`. Этот документ замков
не снимает: `import_eligible=false`, `deployed=false`, `production_route=forbidden`,
`graph_write=forbidden`, `auto_promote=forbidden`, `this_wave_promotes=none`.

## Источники: шесть привязок пакета

Пути и sha256 взяты из
`artifacts/parse_compat/grobid_retirement_release_decision_package.json#source_bindings`
(не из памяти). Контракт пере-хэширует каждый носитель с диска и сверяет с этим столбцом.

| Роль | Путь | sha256 | identity |
|---|---|---|---|
| `parity_measurement` | `artifacts/parse_compat/external_holdout_release_probe.json` | `a937da95ef987dffdcde46b998e98549fe247999e50986e1b0627a6f485d8275` | `external-holdout-release-probe.v1` |
| `parity_readable_report` | `doc/review/grobid-parity-report.md` | `93c5326f411b37542907d4e1bdfa8368cc72f7b7a0dae204a1c6be7d3d18d7aa` | `grobid-parity-report.md` |
| `parity_ledger_and_remainders` | `data/parse/grobid_compatibility.yaml` | `0065ca553b8b4f647831bec7f372680c4d6c0f9d31701517883d5b17c9d8f4ec` | `grobid-compatibility.v1` |
| `resource_ceilings` | `artifacts/parse_compat/parse_batch_budget_live.json` | `fa13b1fb4921177aade374ea002b1a85889ae816f79992083cbf6be371d2461d` | `parse-batch-budget-live.v1` |
| `rollback_rehearsal` | `artifacts/parse_compat/native_family_promotion_rehearsal.json` | `e7407fdf83a95d458e3daee7bd857fdb3f8aa3514996f544964ee266213f48c8` | `native-family-promotion-rehearsal.v1` |
| `rollback_release_readiness` | `doc/review/grobid-rollback-readiness.md` | `b1ee13878e68ffffa81f5c0ef2eb289a2cd813f103233d2bad5662ee3d6cea9c` | `grobid-rollback-readiness.md` |

---

## (a) Паритет Rust против Java

Область замера — `local_proxy_only`: числа описывают локальный прокси-замер эмиссии на
замороженной популяции, а не корпусную долю. Носитель объявляет
`pooled_quality_claim: forbidden`, и любое объединённое (pooled) заявление о качестве
запрещено. Числа ниже не пере-округляются и не пере-считываются в этом документе: они
приведены как точная проекция `evaluated_population.class_parity.aggregate` носителя.

Популяция (объявлено 610, измеримо 531) — из `reference_population`:

<!-- parity-population:begin -->
```json
{
  "declared": 610,
  "measurable": 531,
  "excluded": 79
}
```
<!-- parity-population:end -->

Агрегат класса — закрытый набор из восьми целочисленных ключей:

<!-- parity-aggregate:begin -->
```json
{
  "covered_union_recall_micros": 947501,
  "denominator": 79621,
  "f1_micros": 972130,
  "fn": 4711,
  "fp": 0,
  "native_capability_recall_micros": 945771,
  "precision_micros": 1000000,
  "tp": 74910
}
```
<!-- parity-aggregate:end -->

Независимый структурный судья — dev-only
`crates/da-parse/tests/external_holdout_probe_semantics.rs` (roxmltree); компактный
невалидирующий скан эмиссии — производитель счётчиков. Ни один из этих чисел не является
production-достижимостью и не цитируется без записанного человеческого решения
(`citation_requires_recorded_human_release_decision`).

---

## (b) Остатки списком

Остаток — не «мелочь», а явно названная незакрытая вещь со своим авторитетом. Множество
ниже равно `artifact.remainders` (id/state/pointer, без лишних и без пропусков):

<!-- remainders:begin -->
```json
[
  {
    "id": "human_release_decision",
    "state": "pending",
    "pointer": "data/parse/acceptance_manifest.yaml#release_decision_package.human_release_review"
  },
  {
    "id": "named_gap_learning_ruvllm_ablation",
    "state": "typed_refusal",
    "pointer": "data/parse/grobid_compatibility.yaml#parity_ledger.named_gaps",
    "decision_ref": "D471"
  },
  {
    "id": "horizon_native_learning_order",
    "state": "in_horizon_project_native",
    "pointer": "data/parse/grobid_compatibility.yaml#parity_ledger.horizon_order"
  },
  {
    "id": "absent_capability_dispositions",
    "state": "recorded",
    "pointer": "data/parse/grobid_compatibility.yaml#parity_ledger.absent_dispositions"
  },
  {
    "id": "h20_frozen_external_holdout_decision",
    "state": "pending",
    "pointer": "doc/review/grobid-absorption-followup.json#stages.H20"
  },
  {
    "id": "h21_revision_label_residual",
    "state": "accepted_residual",
    "pointer": "data/parse/acceptance_manifest.yaml#external_holdout_contract.release_probe.declared_release.label_divergence_item"
  }
]
```
<!-- remainders:end -->

Человеческий перечень — каждый остаток и его авторитет:

| Остаток | Состояние | Авторитет |
|---|---|---|
| `human_release_decision` | `pending` | решение человека — `release_decision_package.human_release_review` в `acceptance_manifest.yaml` |
| `named_gap_learning_ruvllm_ablation` | `typed_refusal` | типизированный named gap `learning.ruvllm_ablation` (`no_java_donor`), принятый как остаток по решению `D471` |
| `horizon_native_learning_order` | `in_horizon_project_native` | порядок нативной обучающей линии в `parity_ledger.horizon_order` |
| `absent_capability_dispositions` | `recorded` | реестр судеб absent-возможностей: 17 записей — 14 `out_of_product_scope`, 3 `in_horizon_project_native` |
| `h20_frozen_external_holdout_decision` | `pending` | `H20` остаётся `pending` / `unassigned` (`doc/review/grobid-absorption-followup.json#stages.H20`) |
| `h21_revision_label_residual` | `accepted_residual` | принятый остаток `H21` — расхождение метки релиза (`0.9.1` против rollback-цели `0.9.0-crf`) |

---

## (c) Доказанный откат

Откат доказан байтовым равенством: на **каждом** переключённом документе вернувшийся
emission байт-идентичен базовому (`rollback_byte_identical_all`). Ссылки никогда не
сдвигались (`references_never_moved`), а пара switch-and-return непуста
(`switch_and_return_non_vacuous`); носители остались неизменны (`carriers_unchanged`).

Репетиция цитируется **только** как `rehearsal_only`: её `claim_scope` — `rehearsal_only`,
и она не читается ни как измерение паритета, ни как промоушен. Байтовое равенство отката
не является promotion-evidence.

<!-- rollback-rehearsal:begin -->
```json
{
  "documents_total": 3,
  "documents_switched": 2,
  "documents_refused": 1,
  "rollback_byte_identical_all": true,
  "references_never_moved": true,
  "switch_and_return_non_vacuous": true,
  "carriers_unchanged": true,
  "claim_scope": "rehearsal_only"
}
```
<!-- rollback-rehearsal:end -->

Готовность отката зафиксирована отдельным носителем, привязанным по sha256:

<!-- rollback-release:begin -->
```json
{
  "readiness_doc": "doc/review/grobid-rollback-readiness.md",
  "readiness_doc_sha256": "b1ee13878e68ffffa81f5c0ef2eb289a2cd813f103233d2bad5662ee3d6cea9c"
}
```
<!-- rollback-release:end -->

---

## (d) Потолки и остаток Java-CPU

Потолки берутся из `artifacts/parse_compat/parse_batch_budget_live.json` (наблюдения и
`java_cpu_remainder`), а **не** из `native_route_recovery.json`: последний несёт
`observations.java_cpu_wall_clock.state: not_observed` и `h05_closure: not_claimed`, то есть
это другой H05-scope, и цитировать его как закрытие потолков нельзя.

Остаток Java назван числом: `java_cpu_remainder.java_cpu_ns` при измеренном
`wall_clock_ns`; `counter_scope` назван явно — `grobid_sidecar_cgroup_cpuacct` (это
container-wide счётчик сайдкара, все потоки, не per-document, поэтому ratio — число ядер и
может превышать `1_000_000`). Неизвестное значение никогда не читается как ноль.

<!-- resource-ceilings:begin -->
```json
{
  "java_cpu_wall_clock_ns": 106630786000,
  "java_cpu_remainder": {
    "basis": "numerator is the GROBID sidecar's container-wide CPU total over the window (cgroup v2 cpu.stat usage_usec delta scaled to ns, all threads, NOT per-document), so the ratio is a count of cores and may exceed 1_000_000; the wall-clock window is measured between the same before/after counter reads that produce the delta, and that window contains no synthetic delay",
    "counter_scope": "grobid_sidecar_cgroup_cpuacct",
    "java_cpu_ns": 106630786000,
    "java_cpu_to_wall_ratio_micros": 3540272,
    "state": "observed",
    "wall_clock_ns": 30119369791
  },
  "latency_p50_ms": 18201,
  "latency_p95_ms": 22473,
  "latency_p99_ms": 22473,
  "max_reorder_depth": 2,
  "peak_retained_bytes": 2358615,
  "budget_waits": 2
}
```
<!-- resource-ceilings:end -->

Латентность в потолках — записанная стенными часами per-document величина (не pooled);
`budget_waits` считает срабатывания потолка, а не ретраи.

---

## (e) Что заполняет ЧЕЛОВЕК

Живое состояние записи человека (проверяется против
`data/parse/acceptance_manifest.yaml#release_decision_package`):

<!-- package-state:begin -->
```json
{
  "package_status": "pending_human_review",
  "reviewer": null,
  "decision": "pending",
  "prose_alone_authorizes_release": false
}
```
<!-- package-state:end -->

Таблица PENDING-полей. Ни одно из них агентом не заполнено — приведены текущие значения
для удобства рецензента. Форма записи решения:
`doc/review/HUMAN-REVIEW-TEMPLATE.md`.

| Поле | Текущее значение | Чем заполняется человеком | Статус |
|---|---|---|---|
| `release_decision_package.package_status` | `pending_human_review` | итоговый статус пакета после ревью | **PENDING** |
| `release_decision_package.human_release_review.reviewer` | `null` | именованный рецензент (`named_reviewer`) | **PENDING** |
| `release_decision_package.human_release_review.decision` | `pending` | явное решение (`explicit_decision`) | **PENDING** |
| `release_decision_package.quality.status` | `blocked` | вердикт quality-гейта | **PENDING** |
| `release_decision_package.cost.status` | `bounded_not_measured` | вердикт cost-гейта | **PENDING** |
| `release_decision_package.rights.status` | `blocked` | вердикт rights-гейта | **PENDING** |

Обязательные условия авторизации (`authorization_requires`): `named_reviewer`,
`explicit_decision`, `passing_quality_gate`, `passing_cost_gate`, `passing_rights_gate`.

Запрещённые до заполнения авторизации (`prohibited_authorizations`): `redistribution`,
`production_use`, `model_distribution`, `external_downloads`, `graph_write_or_import`.

Инвалидация (`invalidation`): `on_source_revision_change: required`,
`on_manifest_identity_change: required`, `on_rights_evidence_change: required`.

**Проза релиз не авторизует.** Даже при зелёном тесте этот документ не выводит
`reviewer`/`decision` из прозы, ADR или коммита: остаётся ровно `reviewer=null`,
`decision=pending`, а `GAR-015` — `open`. Условие закрытия `H20` — именно записанное
решение человека, а не предъявленный пакет.
