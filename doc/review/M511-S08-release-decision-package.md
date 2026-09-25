# M511-j7wz2f/S08 — release decision package (bounded external holdout probe)

Статус пакета: **`pending_human_review`**. Этот документ подготавливает решение для
**настоящего человека**. Ни одно поле `human_release_review` агентом не заполняется,
и ни один агент не является рецензентом. Пакет описывает, что измерено, что НЕ
заявляется и что именно остаётся заполнить человеку.

Источники (все в репозитории):

| Артефакт | Путь | sha256 | Размер |
|---|---|---|---|
| Замороженный runtime-замер пробы | `artifacts/parse_compat/external_holdout_release_probe.json` | `sha256:30479a606720bcd03cd87348ec9f16b13a75dfbd4e32376dda64a7c1fed6eefa` | 36 511 B |
| Пилотный отчёт 2026-09-12 | `artifacts/parse_compat/holdout_results.json` | (см. файл) | 2 892 B |
| Lane и гейты | `data/parse/acceptance_manifest.yaml` → `external_holdout_contract` / `release_decision_package` | — | — |
| Замороженная identity пакета | `data/parse/holdout_manifests.yaml` | `sha256:9305aa0b6d96cfb7ef5d6a5d87b8098b725cef1d0f488154e599f7444d0d4ab8` | — |
| Byte-пинны 5 TEI | `crates/da-parse/tests/fixtures/holdout/*.tei.xml` | per-document (таблица ниже) | — |
| Latency/release provenance | `crates/da-parse/tests/fixtures/holdout/provenance.yaml` | — | — |
| Форма записи решения человека | `doc/review/HUMAN-REVIEW-TEMPLATE.md` | — | — |

---

## (a) Что ИСПОЛНЕНО и измерено

### a.1 Identity пробы (заморожена ДО измерения)

`data/parse/acceptance_manifest.yaml` → `external_holdout_contract.release_probe`:

- `identity_input`: `canonical_dataset_id_plus_identity`; `digest_is_byte_hash: false`;
  `digest_is_authority_binding: true`; `frozen_before_execution: true`.
- `identity_digest`: `sha256:22df1b7e97b57304056637b8b02d6458e91b7e371b9d686fdc62414d422e810a`
  (ПЕРЕСЧИТАН из `data/parse/holdout_manifests.yaml` по объявленным ручкам
  `digest_input_fields`/`digest_input_order`/разделителям, а не доверен объявленному значению).
- `source_manifest_sha256`: `sha256:9305aa0b6d96cfb7ef5d6a5d87b8098b725cef1d0f488154e599f7444d0d4ab8`.
- `margin_ref`: `external_holdout_contract.acceptance_margins` (второго числового
  словаря нет — дрейф имён метрик → `probe_metric_vocabulary_drift`).

### a.2 Пять документов и byte-пинны

Ровно 5 не-gold arXiv-документов; identity каждой — sha256 TEI-референса:

| article_id | TEI fixture | identity_sha256 | size_bytes |
|---|---|---|---|
| 1203.2295 | `crates/da-parse/tests/fixtures/holdout/1203.2295.tei.xml` | `sha256:a230094fff20d5434e1871ee9511ce61d71777b6ff0454faeaedc79cdfef6629` | 46 984 |
| 2401.17167 | `crates/da-parse/tests/fixtures/holdout/2401.17167.tei.xml` | `sha256:7c9c42adc0ddbc22a9bf889c31dd3387b26d6dd8cd007909fb3127c29628c262` | 191 436 |
| 2411.07228 | `crates/da-parse/tests/fixtures/holdout/2411.07228.tei.xml` | `sha256:97a3f9f0eab21f512a2fed28a7e98058d002c3a056297d86f7bc4e025b4ec0db` | 142 184 |
| 2505.16270 | `crates/da-parse/tests/fixtures/holdout/2505.16270.tei.xml` | `sha256:2d09c727aa300bf8446efbdf55a94d7713b4d7d39e9fd3f84ee84b12453426bc` | 264 028 |
| 2608.22118 | `crates/da-parse/tests/fixtures/holdout/2608.22118.tei.xml` | `sha256:635dc993492fbb3e83b9d8478fe85be56042bcacf435a627b8e254f1761a2714` | 94 186 |

Ссылочный релиз, которым порождён референс: `java-grobid-http` версия `0.9.1`,
image digest `sha256:223957791ac2bbe48609dcc58a689b16b60baeae13a8734ef440ae6bfb38f4cd`,
профиль `structural` (`consolidateHeader=0`, `consolidateCitations=0`,
`teiCoordinates=figure,table,note,formula`), endpoint `http://127.0.0.1:8070`,
`captured_at_revision: git:602fa8637480fb5b5cedf3115afbc1954cbf5619`.

### a.3 Per-document agreement и bounded uncertainty

Единицы: agreement и uncertainty заморожены целыми в **micros** (`units.agreement_ratio:
ratio_micros`, `units.uncertainty: ratio_micros`), latency — в миллисекундах.

| article_id | source_population | admitted | conserved | lost | fabricated | agreement | uncertainty_low | uncertainty_high | width | latency_ms |
|---|---|---|---|---|---|---|---|---|---|---|
| 1203.2295 | 57 | 56 | 56 | 0 | 0 | 1.000000 | 0.935804 | 1.000000 | 0.064196 | 46 341 |
| 2401.17167 | 283 | 268 | 268 | 0 | 0 | 1.000000 | 0.985868 | 1.000000 | 0.014132 | 10 255 |
| 2411.07228 | 128 | 121 | 121 | 0 | 0 | 1.000000 | 0.969228 | 1.000000 | 0.030772 | 5 420 |
| 2505.16270 | 221 | 219 | 219 | 0 | 0 | 1.000000 | 0.982761 | 1.000000 | 0.017239 | 8 089 |
| 2608.22118 | 42 | 41 | 41 | 0 | 0 | 1.000000 | 0.914330 | 1.000000 | 0.085670 | 3 883 |

Агрегат (`evaluated_population.aggregate`, `counts_only: true`):
`source_population=731`, `admitted=705`, `tp=705`, `fn=0`, `fp=0`,
`agreeing_documents=5`, `margin_pass=10`, `margin_fail=0`,
`no_denominator_documents=0`, `claim_scope=local_proxy_only`,
`documents=5`. **Это счётчики, а не корпусная доля** — pooled-заявление запрещено.

### a.4 Margin-вердикты

Маржи объявлены ДО измерения (`acceptance_margins.declared_before_execution: true`,
`margins_are_provisional: true`) и берутся только по `margin_ref`:

| metric | direction | margin | threshold (micros) |
|---|---|---|---|
| `document_agreement_ratio` | `at_least` | 0.9 | 900 000 |
| `document_uncertainty_interval_width` | `at_most` | 0.25 | 250 000 |

Измерено: **10/10 вердиктов `PASS`** (2 метрики × 5 документов), `margin_fail=0`,
`no_denominator_documents=0`. `failed_or_missing_margin_behavior: fail_visible`;
`claim_without_measured_margin: forbidden`.

### a.5 Latency / cost stance

- Latency (`latency_accounting`): `measurement_source: recorded_wall_clock_per_document`,
  `wall_clock_at_run_time: false`; каждое значение читается из
  `crates/da-parse/tests/fixtures/holdout/provenance.yaml#documents[].latency_ms`,
  никогда не берётся с часов прогона. Диапазон: min **3 883 мс**, max **46 341 мс**
  (`pooled_latency_claim: forbidden`).
- Cost (`cost_accounting`): `status: bounded_not_measured`, `unknown_is_not_zero: true`,
  `currency: USD`, `cost_lower_bound: null`, `cost_upper_bound: null`,
  `release_claim_allowed: false` (`pooled_cost_claim: forbidden`). Верхняя граница
  называет `human_review` и `permitted_external_acquisition_if_later_approved`.
- Transfer (`transfer_accounting`): `measured: false`, `transfer_bytes_loaded: false`,
  `transfer_claim_allowed: false`.

### a.6 Narrowing и typed gaps

`class_narrowings` — полная таблица классов, где `admitted < source_population`
(10 строк), в дополнение к поштучным `typed_gaps` (каждая с причиной парсера).
Оба набора не совпадают, и это важно: сужение affiliation у `2411.07228` (11 → 8)
вызвано отклонёнными строками авторов, чьи аффилиации вообще не связаны, поэтому
отдельной gap-строки по аффилиациям нет. Guard доказывает, что `class_narrowings`
равен множеству, выведенному из `class_profiles`, поэтому сужение не может быть
молчаливым.

### a.7 Точные команды воспроизведения

```bash
# Замороженный runtime-замер (перезапись артефакта + печать профиля)
cargo run -p da-cli --features pdf-layout --example external_holdout_release_probe \
  -- --report artifacts/parse_compat/external_holdout_release_probe.json --print

# Независимый per-document оракул (dev-only, da-parse)
cargo test -p da-parse --test external_holdout_probe_semantics

# Контракт lane и негативы по именам (независимый герметичный набор)
cargo test -p da-parse --test external_holdout_probe_contract

# Pin замороженного артефакта + закрытие H20 + byte-детерминизм (6 тестов)
cargo test -p da-cli --test external_holdout_release_probe_test

# Гейт «проба не закрывает H20» и живые замки
cargo test -p da-parse --test external_holdout_package_contract

# Живая заморозка референса: preflight / verify / self-check
cargo run -p da-cli --example external_holdout_capture -- --preflight
cargo run -p da-cli --example external_holdout_capture -- --verify
cargo run -p da-cli --example external_holdout_capture -- --self-check

# Governor: preflight и привязка артефакта к обеим поверхностям
PYTHONPATH=scripts python3 -m governor --phase preflight
PYTHONPATH=scripts python3 -m governor review --registry doc/review/grobid-absorption-roadmap-2026-09-21.yaml
```

Прогон канонического замера требует `--report <path>`: без него инструмент
отказывает и ничего не пишет. Артефакт байт-идентичен двум независимым прогонам из
разных рабочих каталогов (тест `committed_artifact_equals_two_independent_runs`).

---

## (b) Что НЕ заявляется

1. **Внешние dataset-ряды остаются `bytes_absent`.** `external_holdout_contract.execution_status:
   not_executed`, `byte_policy.external_bytes: bytes_absent`,
   `byte_policy.external_rows_are_evidence: false`. Проба — это bounded local proxy над
   замороженными локальными байтами; внешняя полоса НЕ исполнена и НЕ тронута.
2. **Пересечение с materialized production coverage — 4/5, названо.** Измеренная
   census: `1203.2295`, `2401.17167`, `2411.07228`, `2505.16270` также
   materialized production coverage (`data/processed/<id>/`); `2608.22118` — нет.
   Отдельная filesystem-census артефакта: 5/5 объявленных PDF присутствуют в
   `data/sources/arxiv` (sources archive), 0/5 в каноническом каталоге, 0/5 в
   `data/gold_standard`. `identity_binding.production_corpus_identity_overlap:
   forbidden` — независимость пакета ограничена именем `non_gold`, а не
   «независимостью от production».
3. **5 документов — не статистический корпус.** `measured_scope`: «five non-gold local
   holdout documents; every number is a count, never a corpus rate; no pooled claim».
   `claim_scope: local_proxy_only`. Никакого вывода о качестве корпуса из этого не следует.
4. **Production-достижимость не доказана.** Проба измеряет bounded local proxy
   (компактный сканер эмиссии), а не production-маршрут; авторитетный структурный
   оракул остаётся dev-only (`crates/da-parse/tests/external_holdout_probe_semantics.rs`).
5. **Замки и D127 не сняты.** `policy_locks`: `import_eligible: false`,
   `deployed: false`, `production_route: forbidden`, `auto_promote: forbidden`,
   `graph_write: forbidden`, `this_wave_promotes: none`. Разрешение M511 не является
   разрешением снять D127; graph write / import / промоушен запрещены.
6. **Latency и cost — bounded, not measured.** Неизвестное значение никогда не ноль;
   pooled-заявления по качеству, латентности и стоимости запрещены.
7. **GAR-015 остаётся `open`.** Разделы ниже описывают подготовку пакета, а не
   выполнение релизного решения; `doc/review/grobid-absorption-review-2026-09-10.jsonl`
   по прежнему несёт `status: open` для GAR-015, и тест
   `finding_stays_open_and_no_executed_external_probe_is_claimed` это удерживает.
8. **Проба не цитируема.** `citation_rule`: `requires_recorded_human_release_decision:
   true`, `citation_without_release_decision: forbidden`, `prose_alone_authorizes_citation:
   false`, `bytes_absent_rows_are_evidence: false`.

### Что человеческое решение изменит

Пакет — это подготовка: исполненный замер, замороженный артефакт и fail-closed
правило цитирования. Он НЕ даёт права на production-маршрут, графовую запись,
редистрибуцию, загрузку внешних байтов или промоушен. Даже при `pass` по гейтам
проза (этот документ, ADR, коммит) релиз не авторизует.

---

## (c) Поля, которые заполняет ЧЕЛОВЕК (`PENDING`)

Ни одно из перечисленных полей агентом НЕ заполнено. Значения ниже — текущие
значения в репозитории, приведённые для удобства рецензента. Форма записи:
`doc/review/HUMAN-REVIEW-TEMPLATE.md` (`review_decision_record`).

| Поле (`data/parse/acceptance_manifest.yaml`) | Текущее значение | Чем заполняется человеком | Статус |
|---|---|---|---|
| `release_decision_package.package_status` | `pending_human_review` | итоговый статус пакета после ревью | **PENDING** |
| `release_decision_package.human_release_review.reviewer` | `null` | именованный рецензент (`named_reviewer`) | **PENDING** |
| `release_decision_package.human_release_review.decision` | `pending` | явное решение (`explicit_decision`) | **PENDING** |
| `release_decision_package.quality.status` | `blocked` | вердикт quality-гейта | **PENDING** |
| `release_decision_package.cost.status` | `bounded_not_measured` | вердикт cost-гейта | **PENDING** |
| `release_decision_package.rights.status` | `blocked` | вердикт rights-гейта | **PENDING** |

Обязательные условия авторизации (`human_release_review.authorization_requires`):
`named_reviewer`, `explicit_decision`, `passing_quality_gate`, `passing_cost_gate`,
`passing_rights_gate`. `prose_alone_authorizes_release: false`.

Запрещённые авторизации до заполнения (`prohibited_authorizations`):
`redistribution: blocked`, `production_use: blocked`, `model_distribution: blocked`,
`external_downloads: blocked`, `graph_write_or_import: blocked`.

Инвалидация (`invalidation`): `on_source_revision_change: required`,
`on_manifest_identity_change: required`, `on_rights_evidence_change: required`.

---

## (d) Условие закрытия H20

H20 (`doc/review/grobid-absorption-followup.json`) закрывается ровно тогда, когда:

1. замороженный пакет исполнен и измерен (исполнено в S08: артефакт
   `sha256:30479a60…`, 5 документов, 731 → 705 → 705, 5/5 согласны, 10/10 margin
   `PASS`); и
2. **записано решение человека о релизе** — `reviewer` назван, `decision` не
   `pending`, гейты `quality`/`cost`/`rights` имеют выставленный вердикт.

Пункт (1) исполнен. Остаётся ровно пункт (2): **нет записанного решения человека**.
До этого проба не цитируема, H20 остаётся `pending`, а GAR-015 — `open`.

Остаток назван, а не замолчан. Никакой автоматический механизм не заполняет
человеческие поля и не выводит `reviewer`/`decision` из прозы, ADR или зелёного теста.
