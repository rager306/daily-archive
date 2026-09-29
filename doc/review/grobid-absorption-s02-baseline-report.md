# S02 baseline: slice-level верификация, положительный контроль и матрица негативных мутаций (M514-1lss2s/S02/T05)

Статус: снято на живом дереве. Это отчёт **слайсового уровня**, а не релиз, не
заявление о качестве и не доказательство независимости. Он дополняет, а не
повторяет: `doc/review/grobid-absorption-progress-baseline.md` (S01/T05 —
байт-привязанный baseline измерительной системы) и
`doc/review/grobid-absorption-s02-corpus-inventory.md` (S02/T04 — инвентарь
локального корпуса и лицензий). Здесь записано то, чего в них нет: прогон
четырёх наборов, `--validate-yaml` и `--phase preflight` на живом дереве,
негативная матрица из пяти мутаций на **копиях** закоммиченного артефакта, и
граница того, что baseline ещё не измеряет.

Жёсткое правило этого отчёта: **факты отделены от гипотез** (§1–§3 — измеренное,
§5 — предположения, каждое названо как непроверенное). Никакой hand-edit не
касался коммитнутых артефактов: все мутации выполнялись над `copy.deepcopy`
загруженного scorecard.

## 1. Положительный контроль на живом дереве

Все команды прогнаны через `gsd_exec` на ревизии дерева этой задачи; ниже —
их evidence id (хранилище `.gsd/exec/<id>.stdout`, эфемерное и gitignored).

| # | Команда | Exit | Наблюдение | Evidence id |
|---|---|---|---|---|
| 1 | `pytest scripts/tests/test_absorption_progress{,_governor,_baseline,_corpus_inventory}.py -q` | 0 | `135 passed in 23.79s` | `12e62fe4-8edb-4e9d-a675-730e442abdde` |
| 2 | `governor --validate-yaml` | 0 | YAML-контракты валидны | `62c565de-ba73-4960-8250-bd213780d329` |
| 3 | `governor --phase preflight` | 0 | отчёт построен, фаза `preflight` | `ef5d447b-5617-4f7c-85db-a02ad67febaf` |
| 4 | подсчёт `absorption_progress` в `artifacts/project-trajectory/trajectory-report.json` | 0 | вхождений `absorption_progress`: **0**, флагов `absorption_progress_*`: **0** | `20fe5662-0348-4a79-bdaa-fff94d3d4ba1` |

Коммитнутый scorecard проходит композитный оракул T01+T04 с **пересчитанным**
хешем evidence: это `test_committed_baseline_is_clean` внутри прогона #1.
Отдельно `preflight` не даёт ни одного флага размерности — вердикт отчёта в целом
(`drift_risk`) порождён другими размерностями и к `absorption_progress` отношения
не имеет.

Позитивный fixture (`governor/fixtures/absorption_progress/scorecard_clean.positive.json`)
и негативный (`...negative.json`) проверяются теми же наборами: позитивный чист,
негативный обязан поднять перечисленные в его `require_violations` метки.

## 2. Матрица негативных мутаций (пять мутаций на копиях артефакта)

Каждая мутация: `copy.deepcopy` закоммиченного
`artifacts/parse_compat/absorption_progress_baseline.json` → мутация → композитный
оракул (`absorption_progress_violations` с пересчитанными наблюдениями: sha256
носителей, разрешённые пороги, выведенный census). Ждём типизированную метку;
прогон на копии, коммитнутый артефакт не редактировался. Все пять живут в
`scripts/tests/test_absorption_progress_baseline.py::test_mutation_matrix` (и
дополнительно в `-k`-прогонах ниже).

| # | Мутация | Ожидаемая метка | Результат | Evidence id |
|---|---|---|---|---|
| 1 | ревизия заморозки порогов изменена на чужой sha256 | `threshold_freeze_revision_mismatch` | `1 passed` | `9c1038fa-b153-4780-8ae4-6ace474233fb` |
| 2 | из объявленного scope удалена обязательная семья | `required_scope_shrunk` | `1 passed` | `537eeee2-cba6-4383-a150-dbb3627c871d` |
| 3 | eval-идентичность добавлена в tuning-набор | `eval_split_reused_for_tuning` | `1 passed` | `260e43f1-b1dd-4ea8-b3ea-e67a72ef103f` |
| 4 | из объявленного census выброшен один документ probe | `census_document_dropped` | `1 passed` | `c14be627-4fe9-4f0f-8a5a-e1971046b574` |
| 5 | документ переведён через границу отказа без привязки набора | `refused_set_unbound` | `1 passed` | `c1f08375-02b6-4851-a156-dff0a0b4f1bb` |

Полная матрица (не только эти пять) дополнительно содержит метки
`evidence_hash_substituted`, `evidence_path_not_relative`,
`evidence_artifact_missing`, `oracle_changed`, `scope_not_accepted`,
`denominator_shrink`, `unnamed_exclusions`, `exclusion_reason_untyped`,
`skipped_as_delivered`, `preservation_pooled_with_extraction`,
`threshold_reference_unbound`, `threshold_missing_for_required_metric`,
`coverage_regression`, `savings_without_paired_cpu`,
`authorization_used_as_product_evidence`, `census_projection_mismatch`,
`census_disposition_untyped` — все проверяются тем же параметризованным тестом.

Двухслойная дисциплина меток (D492) проверяется отдельно:
`test_every_committed_label_is_registered_in_a_governed_vocabulary` требует, чтобы
каждая метка была либо в adapter-словаре `violations`
`scripts/governor/checks/grobid_absorption_progress.yaml`, либо литералом в
`scripts/governor/domain/absorption_progress.py`. Метки scope/split — доменные,
поэтому в YAML их нет и быть не должно.

## 3. Замороженные пороги и их reference-указатели

Заморозка — `prospective_regression_floor` (см. §5), объявлена в
`data/parse/absorption_progress.schema.yaml`, привязана к собственной ревизии
носителя и к отдельно запинованному reference-артефакту
`artifacts/parse_compat/absorption_progress_reference_pins.json`. Числового
литерала в контракте нет: значение выводится из reference.

| threshold_id | family | metric | direction | margin | value | reference (path#pointer) |
|---|---|---|---|---|---|---|
| `preservation_micro_occurrence` | `parse.component.native_fulltext_composition_shadow` | `preservation.occurrence_conservation` | `at_least` | 0 | 74910/79621 = 0.9408321925120258 | `artifacts/parse_compat/absorption_progress_reference_pins.json#values[threshold_id=preservation_micro_occurrence]/fraction` |
| `extraction_micro_occurrence` | `stage.reference_segmentation` | `extraction.native_capability_recall` | `at_least` | 0 | 945771/1000000 = 0.945771 | `artifacts/parse_compat/absorption_progress_reference_pins.json#values[threshold_id=extraction_micro_occurrence]/fraction` |
| `extraction_weakest_family` | `stage.header_composition` | `extraction.native_capability_recall` | `at_least` | 0 | 3055/7077 = 0.4316800904337996 | `artifacts/parse_compat/absorption_progress_reference_pins.json#values[threshold_id=extraction_weakest_family]/fraction` |

Пороги покрывают ровно обязательные семьи (state `required`); у
`threshold_pending`-семей порога нет и быть не может (D496). Привязка хешами на
этой ревизии:

| Носитель | sha256 |
|---|---|
| `data/parse/absorption_progress.schema.yaml` | `4ed78e34cb5863cf0fcbf6c09fed1351e1bb722659c73b881a36b1a16317641a` |
| `artifacts/parse_compat/absorption_progress_baseline.json` | `cc6eb1e9e9524d1df8b9cd02142eee742e61bddc736e40bd75dacbc5f515e29a` |
| `artifacts/parse_compat/absorption_progress_reference_pins.json` | `2befd08dd8b095a6e8e6ea17e6b73e92e78e282ab98cbad64c4727218d6fe158` |
| `artifacts/parse_compat/external_holdout_release_probe.json` | `a937da95ef987dffdcde46b998e98549fe247999e50986e1b0627a6f485d8275` |
| `data/parse/holdout_manifests.yaml` | `ef91d3c25c7d91dda2654471140fd41fcae8e1ed87e7094ad24c31fef2b64a39` |

Инвариант фиксированной точки (проверяется тестами, не утверждается):
`threshold_freeze.revision` = sha256(schema) = `4ed78e…641a`;
`threshold_freeze.reference_pins_revision` = sha256(reference pins) = `2befd0…e158`;
reference pins `derived_from.sha256` = sha256(probe) = `a937da…8275` = schema
`threshold_policy.freeze_revision`. Любая правка schema после заморозки —
находка (`threshold_freeze_revision_mismatch`), а не тихая перетюнинг.

## 4. Что baseline утверждает о популяции

Измеренные факты (пересчитываются на каждом прогоне тестами baseline):
назначено 610, исполнено 531, отказано 0, исключено 79 (типизированно:
`header_date_absent` 72, `header_title_absent` 5, `header_author_absent` 2);
`assigned = executed + refused + excluded`. Проекция
`(document_id, disposition)` по всем 610 документам выводится из двух
замороженных источников и имеет `projection_sha256`; replay стабилен. Внешняя
полоса — `bytes_absent`, скачивание запрещено, внешние строки не evidence.

Пороги применяются к уже замороженному прогону S01 по 610-документному корпусу;
равенство кандидата и floor сегодня — следствие общего замороженного источника,
а не независимое подтверждение.

## 5. Чего baseline ещё не измеряет (гипотезы и незакрытые оси)

Это **не** список задач, а граница честности. Всё ниже либо `not_measured`, либо
не проверено и названо как гипотеза.

**Факты:**

- `java_independence` — `state: not_measured`. Продукционный маршрут разбора —
  Java GROBID; независимость парсера от Java не измерена и не заявляется.
- `resources` — `state: not_measured`. Никакого сохранения ресурсов не измерено;
  wall-clock-only claim был бы отвергнут меткой `savings_without_paired_cpu`.
- `reliability` — `state: not_measured`. Есть rehearsal, но это не продуктовое
  измерение.

`test_baseline_declares_full_denominator_and_unmet_axes` падает, если хоть одна
из этих осей объявлена измеренной или подменена нулём, — «зелёный» scorecard,
который бы их заявил, не пройдёт.

**Гипотезы (не подтверждены этой задачей):**

- Что равенство будущего кандидата floor означает отсутствие регрессии.
  Опровергающее: и кандидат, и floor выводятся из одного замороженного probe S01;
  это не доказательство независимости (это же написано в `scope` reference pins).
- Что заморозка порогов исторически предшествовала скорингу. Это
  `prospective_regression_floor`: корпус измерен в S01, поэтому заявление
  «frozen before scoring» статически не доказуемо и не делается.
- Что baseline чем-либо подтверждает независимость локального корпуса или права
  на него. Права пер-документно unknown (§T04), внешние байты отсутствуют.

**Граница контракта (E3, унаследована, не расширяется):** это
consistency/fail-closed поверх self-declaration плюс пересчитанный content hash,
а **не** tamper-evident attestation. Актор, одновременно переписывающий scorecard,
evidence и DATA-правило, её удовлетворит. Гарантия — что дрейф, подстановка и
тихая перемаркировка перестают проходить.

**D491/D127 не двигаются:** без распространения, без записи в граф, без импорта;
`import_eligible=false`, `graph_write=forbidden`, `auto_promote=forbidden`.

## 6. Покрытие требований слайса

- **R141 (operability)** — scope, идентичность независимого корпуса и пороги
  заморожены как DATA; `scorecard.required: true`; удаление артефакта или порог
  без измеримого reference — находка. Перепроверено прогонами #1–#4 и §2–§3.
- **R133 (quality-attribute)** — census не регрессирует: 610/531/79 и
  `projection_sha256` пересчитываются на каждом прогоне, replay стабилен,
  мутации #4–#5 поднимают свои метки.
- **R134 (core-capability)** — дайджест пересчитывается, а не утверждается:
  мутация #1 ловит чужую ревизию заморозки; `test_threshold_reference_does_not_follow_the_candidate_scorecard`
  доказывает, что floor не следует за кандидатом.

## 7. Как это перепроверить

```bash
PYTHONPATH=scripts python3 -m pytest scripts/tests/test_absorption_progress.py \
  scripts/tests/test_absorption_progress_governor.py \
  scripts/tests/test_absorption_progress_baseline.py \
  scripts/tests/test_absorption_progress_corpus_inventory.py -q
PYTHONPATH=scripts python3 -m governor --validate-yaml
PYTHONPATH=scripts python3 -m governor --phase preflight
```

Ожидаемо: `135 passed`, оба governor-вызова exit 0, в
`artifacts/project-trajectory/trajectory-report.json` — 0 вхождений
`absorption_progress`.
