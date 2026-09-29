# Pipeline integrity review package

This directory contains a compact, source-bound review of the daily-archive Rust pipeline, cache invalidation, temporal article structure, context conservation, FSMs, agent readiness and multimodal understanding.

## Текущая корректирующая волна — M512-lxafs8/S05 (2026-09-27)

**M512-lxafs8** согласует ADR и живые review-поверхности с измеренным
состоянием, не снимая ни одного замка. S01–S04 измерены: размер корпуса выведен
из данных (`data/parse/holdout_manifests.yaml`,
`data/parse/acceptance_manifest.yaml`), заявленная corpus-scale граница отмечена
(D455), перепись
**610 объявленных = 531 измеримых + 79 типизированных исключений**
(`header_date_absent` 72 / `header_title_absent` 5 / `header_author_absent` 2)
зафиксирована артефактом
`artifacts/parse_compat/external_holdout_release_probe.json`
(sha256 `793d551f…`, identity `sha256:eee622ae…`), а claim-поверхности
пере-пинены (correction C13). Измерено: 79621 source → 74910 admitted →
74910 conserved, 0 lost, 0 fabricated; 531/531 документов согласны;
1062/1062 margin-вердиктов PASS. Исторический RED S05/T06
(`population_contract_enumerates_every_shadow_report`) снят S05/T07
приведением corpus-артефакта к GAR-006-разбиению reference/executed/evaluated и
текущим остатком не является. Ревизия квалификации — `1a22cd0e…`; тот же
литерал стоит в `reviewed_source`
[roadmap governor](grobid-absorption-roadmap-2026-09-21.yaml) (correction C13).
Датированные записи — audit §S05, ADR-076 §24, ADR-075 §10.
[H14–H21](grobid-absorption-followup.json) остаются планом, а не доказательством
реализации. Замки сохранены: `import_eligible=false`, `deployed=false`,
`production_route=forbidden`, `graph_write=forbidden`, `auto_promote=forbidden`,
D127 закрыт. Production promotion и снятие D127 не разрешены. Human release —
только настоящим ответом владельца; GAR-015 автоматически не закрывается.

**Корректировка 2026-09-28 (M513-r7dg4z/S03).** Assessed again 2026-09-28 under wave M513-r7dg4z/S03: the parity ledger now binds each required row to the S02 Java-relative corpus measurement (artifact sha256 a937da95ef987dffdcde46b998e98549fe247999e50986e1b0627a6f485d8275, identity sha256:eee622ae57286faff567b1793f31ee54c5df3118e544a4bb8edf1c7376955c6d; measured = header_composition / reference_segmentation / native_fulltext_composition_shadow, the other 12 required rows typed outside), the single named gap is a typed refusal (D471, residual_owner M513-r7dg4z/S05), GAR-015 stays the single open record with the human release decision still pending (reviewer=null, decision=pending), and no lock changed.

**Корректировка 2026-09-28 (M513-r7dg4z/S04).** Assessed again 2026-09-28 under wave M513-r7dg4z/S04: the long-horizon register no longer carries an unresolved stage — H05 was closed with `completion_evidence` (mode `runtime`) after `java_cpu_wall_clock_ns` was measured against `grobid_sidecar_cgroup_cpuacct` while the real Java GROBID sidecar participated under two concurrent batches with a slow ordinal-0 (artifact `artifacts/parse_compat/parse_batch_budget_live.json`, sha256 `fa13b1fb…`: 106630786000 ns container-wide sidecar CPU over a 30119369791 ns window, 3.54 busy cores) — so `governor review` over the horizon contract reads `stages=13 unresolved=0 complete=13 claims=13 unassigned_unresolved=0 open_items=0`; the [H14–H21](grobid-absorption-followup.json) planning register stays wholly unresolved with H20 `pending`/`unassigned`, GAR-015 stays the single open record with the human release decision still pending (`reviewer=null`, `decision=pending`), and no lock changed.

**Корректировка 2026-09-29 (M513-r7dg4z/S05).** Assessed again 2026-09-29 under wave M513-r7dg4z/S05: the fate of the 16 absent rows and the single named gap is now a recorded decision in `data/parse/grobid_compatibility.yaml#parity_ledger` (14 `out_of_product_scope` / 3 `in_horizon_project_native`, horizon order `ruvllm_ablation` -> `sona_trajectory` -> `microlora_ewc`), judged fail-closed with 37 declared mutations and 0 survivors; the carrier sha was re-pinned at both `scholarly_parse_plane` sites and the fixture mirror and `native_route_recovery.json` was regenerated; the [H14–H21](grobid-absorption-followup.json) planning register keeps H20 `pending`/`unassigned` and forbids PASS claims while the H21 open item is now explicitly accepted as a residual (owner `unassigned`); GAR-015 stays the single open record with the human release decision still pending (`reviewer=null`, `decision=pending`), and no lock changed.

### M511-j7wz2f/S01/T01 — RED зафиксирован как исполняемая регрессия

> Историческая запись: измеренный здесь RED снят в S01/T02 (гейт больше не
> `#[ignore]`), а S01/T03 закрыл найденный остаточный namespace-случай. Счёт
> 1352/1 и ignored-гейт относятся к ревизии T01; актуальные замеры — ниже.

Диагностический probe аудита (`grobid-composition-repro.rs`, rustc вне cargo)
превращён в отслеживаемую регрессию на публичном композиторе:
`crates/da-parse/tests/native_fulltext_composition_contract.rs`

- `a_xml_red_preserved_tei_emissions_are_rejected_by_an_independent_parser`
  (по умолчанию) — фиксирует измеренный RED: `Accepted` на трёх сохранённых TEI
  при том, что независимый парсер отвергает эмиссию; пины — `</title></title>`
  ровно один раз, рост числа закрывающих `</title>`, потеря `xml:lang` у
  синтезированного `<teiHeader>`, и точный диагностический текст.
- `preserved_tei_composition_is_independently_well_formed` — целевой гейт,
  `#[ignore]` пока дефект RED; при `--ignored` падает с
  `Err("expected 'titleStmt' tag, not 'title' at 6:<col>")` на всех трёх.

Независимый парсер — `roxmltree` 0.21.1 как **dev-dependency** (в `src/`
недостижим), Cargo.lock менялся точечно. Каждый прогон проверяет sha256
сохранённых фикстур, поэтому падение не может «съесть» знаменатель.
Замеры: gsd_exec `33c46aa7-0fcf-4421-9223-2e851556cee2` (обычный прогон rc=0 +
ignored-gate rc=101), `6a10f8d4-24ac-4c84-b6b0-afb3defd27f8` (диагностика по
фикстурам). Это измерение публичного shadow-композитора: production-
достижимость не доказана, H14 остаётся незакрытым (`implementation_pass=false`),
замки не менялись.

### M511-j7wz2f/S01/T02 — RED→GREEN: структурная XML-композиция

Эмиттер переписан структурно, а не косметически: `XmlDocument` спаривает
открывающие и закрывающие теги одним стеком (диапазон всегда
`(open start, close end)`), содержимое заголовка переразмещается по родителю
(дети `titleStmt` → в синтезированный `titleStmt`; дети `fileDesc` без
`titleStmt` → в синтезированный `fileDesc`; остальные дети `teiHeader` → после
него), атрибуты исходного `<teiHeader>` воспроизводятся дословно, и эмиссия
перечитывается до `Accepted` (`ComposedOutputNotWellFormed`), иначе отдаётся
байт-идентичный Java-envelope. Гейт
`preserved_tei_composition_is_independently_well_formed` больше не `#[ignore]`
и зелёный на трёх сохранённых TEI; RED-пин T01 заменён независимой
консервацией (`preserved_tei_emissions_conserve_every_source_header_subtree`).
Замеры: `cargo test -p da-parse --test native_fulltext_composition_contract` —
17 passed, 0 ignored; `cargo test -p da-parse --tests` — 100 suites, 1359 passed,
0 failed, 0 ignored (gsd_exec `796cf21e-8a88-4502-a915-fd2a6a60b99d`,
`412df4be-f747-4457-9c3b-a6eff4bf779e`).

### M511-j7wz2f/S01/T03 — остаточный namespace-случай и доказательства среза

Обязательная для T03 проверка namespace-негативов воспроизвела остаточный
дефект того же класса: объявление `xmlns:ext` на **синтезируемых** обёртках
`fileDesc`/`titleStmt` не воспроизводилось, поэтому композитор возвращал
`Accepted` при эмиссии, отвергаемой независимым парсером
(`an unknown namespace prefix 'ext' at 1:202` / `1:153`, gsd_exec
`c9cbe8d7-b4dc-492a-aa7d-ab7134a5335b`). Закрыто узко: с этих двух обёрток
переносятся только `xmlns`/`xmlns:*` (прочие атрибуты обёрток, например
собственный `xml:lang`, — семантический остаток поля H16, не well-formedness).
Регрессия `wrapper_namespace_declarations_are_carried_so_no_prefix_is_unbound`.
Malformed-набор расширен лишним закрывающим тегом, незавершённым комментарием и
незавершённой CDATA. Shadow-пример проверяется только под обязательной фичей
`pdf-layout`: план-команда без неё даёт rc=101, с фичей check и реальный прогон —
exit=0 (`replay.semantic_equal=true`, `conservation.no_silent_loss=true`,
9 terminal из 9 admitted). Замки не менялись; H14 остаётся planning-записью с
`implementation_pass=false`.

### M511-j7wz2f/S01/T04 — Accepted требует WF-прохода, отдельного от сканера

Эмиттер получил отдельный проход `validate_well_formed` по конкретным
WFC-классам, а не самоудостоверение через `XmlDocument::parse`: ровно один
корневой элемент, легальность `Name`/`QName`, список атрибутов в форме
`Name S? Eq S? AttValue` (кавычки, отсутствие дублей), ссылки `&`/`&#…;`, `]]>` в
CharData, `--` в комментарии, CDATA только внутри документа, лимит глубины,
легальные XML-символы и привязка префиксов. Сырой `&`, `]]>`, дублированный
атрибут и `--` в комментарии дают типизированный `Rejected` с байт-идентичным
Java-конвертом. Исторический RED этого класса (объявление `xmlns:ext = "urn:x"`
на синтезируемой обёртке терялось) снят здесь. Замки не менялись.

### M511-j7wz2f/S01/T05 — консервация обёрток байтовыми интервалами

Обёртки (в том числе неэлементные дети — комментарии, PI и текст/whitespace
между детьми) консервируются байтовыми интервалами, а поиск wrapper'а ограничен
ПРЯМЫМИ детьми, поэтому вложенный `titleStmt` больше не дублируется. Три новые
регрессии RED→GREEN; `cargo test -p da-parse --tests` — 1369 passed / 0 ignored
на той ревизии.

### M511-j7wz2f/S01/T06 — правописание `xml:id`, `<!DOCTYPE`-подстрока, лимиты

Счётчики в разделах T01–T03 выше — снимки тех ревизий (17 contract-тестов,
1359 passed). Актуальный замер — ниже.

- Правописание `xml:id` больше не обходит uniqueness-гейт: `xml:id = 'author-0'` —
  то же, что `xml:id="author-0"`, и раньше эмиссия публиковалась как `Accepted`
  с двумя одинаковыми `xml:id` (gsd_exec
  `b9a3be53-e685-41c7-ba48-f66674baad41`). Атрибуты читаются XML-грамматикой;
  нечитаемый список — типизированный отказ.
- `<!DOCTYPE`/`<!ENTITY` распознаётся там, где начинается разметка: упоминание
  строки в комментарии или CDATA больше не отвергается как декларация. Гейт не
  ослаблен — реальный `<!DOCTYPE` остаётся `ForbiddenExternalEntity`, `<!doctype`
  отклоняется как нелегальная разметка, а `<!DOCTYPE` в значении атрибута — по
  правилу `AttValue`.
- Лимиты 4 MiB (фрагмент) и 128 (глубина XML) замерены как уже корректные
  (типизированный отказ, байт-идентичный Java-envelope, без частичного вывода) —
  это pinned regression, не исправление.

Актуальный замер: `cargo test -p da-parse --test
native_fulltext_composition_contract` — 28 passed / 0 ignored;
`native_fulltext_composition_properties` — 8 passed; `cargo test -p da-parse
--tests` — 100 suites / 1374 passed / 0 failed / 0 ignored; clippy и fmt —
exit=0; `governor review --registry
…/grobid-absorption-roadmap-2026-09-21.yaml` — exit=0. H14 не закрывается:
`implementation_pass=false` и planning-семантика сохраняются; замки не
менялись.

### M511-j7wz2f/S02/T01–T05 — целостность библиографических ссылок

A-REF (принятый `Accepted` при висячем `body target="#b0"`) измерен и снят на
той же замеренной области, что и A-XML: три sha256-пиннутых сохранённых TEI
(`hub_2507.19457`, `rlm_2512.24601`, `reca_2605.26525v1`), публичный
shadow-композитор, независимый оракул `roxmltree` — только dev-dependency.

- **Исторический RED (S02/T01).** Композитор эмитировал синтетические
  `xml:id="bibl-{ordinal}"`, тогда как сохранённое тело ссылалось на исходные
  ключи `b0..`; гард резолва был ограничен `candidate.anchors` и при пустом
  anchor-потоке гейт был вакуумным. Измерено: 207 записей (80/49/78), 217
  вхождений `ref type="bibr"` (87/71/59) и **120 обязательных целей (55/41/24),
  все 120 висели**. RED записан явным пином без `#[ignore]`.
- **Текущий GREEN (S02/T02–T05).** Композитор публикует source-published keys
  (declared `xml:id`, иначе `b{ordinal}`) и выполняет post-splice resolve gate:
  обязательные targets выводятся из сохранённого документа, независимо от
  `candidate.anchors`, и `Accepted` невозможен, если ссылка не резолвится в
  id-пространстве эмиссии. На тех же фикстурах — те же 207 записей `biblStruct`
  и **0 висячих**. Типизированные отказы: `BibliographyDomainMismatch`,
  `BibliographyKeyCollision`, `UnresolvedBibliographyTarget`,
  `InconservableBibliographyChild`.
- **Невакуумность на реальном прогоне.** Shadow-пример печатает блок
  `reference_links` (`required_targets=1`, `resolved_targets=1`, `id_space=3`) и
  падает при `required_targets == 0`; замороженный артефакт перегенерирован и
  запиннен (`5b125dcb…`). Правило живёт в sibling-блоке
  `reference_link_integrity` уже санкционированного
  `data/parse/native_fulltext_composition.yaml` (нового файла контракта нет), а
  токены блока машинно сверяются с константами композитора.
- Замер: `reference_integrity` 19 passed / 0 ignored, `data_contract` 5,
  `oracle` 2, `properties` 8, `contract` 28, `cargo test -p da-parse --tests` —
  101 suites / 1396 passed / 0 failed / 0 ignored; shadow-пример exit=0.

**H15 не закрывается:** планирующая запись сохраняет `status: pending`,
`implementation_pass=false`, `verification_pass=false`; измеренный GREEN
относится к трём сохранённым фикстурам, а закрытие этапа требует независимой
корпусной регрессии (S05). Замки не менялись: D127/D321 закрыты, production
promotion и graph writes не разрешены.

## Current baseline

Recomputed from `pipeline-integrity-review.jsonl` on **2026-08-19** after M401 S05 closed PIR-017 and PIR-021. Do **not** increment the printed M397 table (Resolved 7 / Open 15) — later waves already resolved PIR-006..PIR-016 and PIR-018..PIR-020 in the JSONL while this README stayed stale.

| Status | Count | Findings |
|---|---:|---|
| Resolved | 22 | PIR-001..PIR-022 |
| Open | 0 | — |

An independent controlled 2026-09-10 absorption review now lives in
`grobid-absorption-review-2026-09-10.jsonl` (GAR-001..GAR-016: 1 unresolved / 15 enclosed). Its machine summary is
`grobid-absorption-review-2026-09-10-summary.md`. Narrative is not engine
input: no prose inside this directory requires work, changes scope, or
substitutes for runtime verification.

The single open record is **GAR-015**, recorded as an *accepted open item* and
not as a closure: the bounded external holdout package is declared and
machine-checked in `data/parse/acceptance_manifest.yaml` (rule
`external_holdout_package`), but the probe is
`external_holdout_contract.execution_status: not_executed` and
`release_decision_package.human_release_review.decision` is `pending` with
`reviewer: null`. Owner of record: `M502-2w7y8k/S09` (the slice that delivered
the release decision package). Closure condition: execute the frozen probe
apart from production corpora with the declared per-document margins and
record a named human reviewer's release decision; citation stays blocked until
then. Its claim, title, risk and acceptance criteria are unchanged.

A long-horizon register lives in `grobid-rust-horizon-review.json`: measured
2026-09-28 it holds 13 stages, all complete — H05 closed when M513-r7dg4z/S04
measured `java_cpu_wall_clock_ns` against `grobid_sidecar_cgroup_cpuacct` with
the real Java GROBID sidecar participating under two concurrent batches with a
slow ordinal-0 — and is no longer the all-unresolved default it was created
with. It records the GROBID-to-Rust migration waves with per-stage ADR refs,
separate implementation and verification evidence, tested source, GitNexus/CBM
freshness (recorded here as a documented staleness limitation), negative tests
and ADR-051 matrix rows. The document is registered with `claim_scope: evidence`:
a completed stage needs machine-verifiable `completion_evidence` and an explicit
verification PASS. Its DATA contract is
`scripts/governor/checks/grobid_rust_horizon_review.yaml`; `governor review` over
that contract is exit=0 with `stages=13 unresolved=0 complete=13 claims=13`,
`unassigned_unresolved=0 open_items=0`.

M397 closed canonical composition, StageId, cache source/profile/schema identity, revision-owned corpus materialization and cross-command equivalence. Subsequent waves closed the remaining register rows: M398 temporal AST (PIR-006..PIR-012), M399 live ACAP/promotion/scheduler (PIR-014..PIR-016), M400 multimodal graph candidates (PIR-018..PIR-020), M401 honest multi-process read plane plus read-only verified agent loop (PIR-017, PIR-021). A 200-paper corpus rebuild/seal is still an operational follow-up, not a finding in this register.

## Current control posture (2026-09-10)

Status semantics: open records are unresolved review observations. They do
not create missions, targets, waves, slices, tasks, success criteria or
acceptance conditions. PIR prose never executes through Governor or GSD on
its own; execution continues only through the one authoritative roadmap
engine in `.gsd/`, which has its own explicit user authorization. GSD
auto does not start from `doc/review`; the revoked subagent behavior that
copied findings into planning is archived as a process failure, not a
control signal.

## Long-horizon execution map

| Wave | Title | Owns | Status |
|---|---|---|---|
| M398-cz4pzx | Temporal AST correctness | PIR-006..PIR-012 | complete |
| M399-assurance-fsm | Live assurance and promotion FSM | PIR-014, PIR-015, PIR-016 | complete |
| M400-multimodal-graph | Multimodal graph bridge | PIR-018, PIR-019, PIR-020 | complete |
| M401-agent-readiness | Remote read plane and read-only agent | PIR-017, PIR-021 | complete |

A revision-aware corpus rebuild and honest ontology acceptance remain operational follow-ups; they are not claimed by the current register.

## Files

| File | Purpose |
|---|---|
| `pipeline-integrity-review.jsonl` | Historical canonical review dataset: PIR-001..PIR-022, all resolved; do not reopen it |
| `pipeline-integrity-review.schema.json` | JSON Schema Draft 2020-12 for each PIR record |
| `grobid-absorption-review-2026-09-10.jsonl` | Active corrective register GAR-001..GAR-016: 1 unresolved / 15 enclosed; the four P0 findings (GAR-001, GAR-003, GAR-004, GAR-011) were closed by M508-g9b5gi/S05, the eight P1 findings (GAR-005..GAR-009, GAR-012..GAR-014) by M509-yzblqx S01..S05, and GAR-010 and GAR-016 by M510-jsg1pj S02/S01. The single open record GAR-015 is an accepted open item owned by M502-2w7y8k/S09 (external holdout not executed, human release decision pending) |
| `grobid-absorption-review-2026-09-10.schema.json` | JSON Schema Draft 2020-12 for each GAR record, including `completion_evidence` for enclosed findings |
| `grobid-absorption-review-2026-09-10-summary.md` | Human-readable audit summary, exclusions and ADR correction matrix |
| `grobid-rust-horizon-review.json` | Long-horizon register (measured 2026-09-28): 13 complete / 0 unresolved, `claim_scope: evidence`; only a stage with `completion_evidence` and a verification PASS claims closure |
| `grobid-rust-horizon-review.schema.json` | JSON Schema Draft 2020-12 for the horizon registry document |
| `manifest.yaml` | Dataset identity, standards, policy and Governor integration; `last_assessed_at: 2026-09-29`, `current_state` carries the measured S05 reconciliation, the dated S03 parity-ledger correction, the dated S04 H05 closure and the dated S05 absent-capability fate correction (H14..H21 unresolved, census 610 = 531 + 79, GAR-015 open, no lock changed) |
| `grobid-absorption-roadmap-2026-09-21.yaml` | Governor-consumable `horizon-json-v1` wrapper over the H14..H21 follow-up registry: `reviewed_source` equals the audit qualification revision, corrections C01..C14, `measured_state` (locks, parity ledger, tests, horizon) and the typed `revision_label_resolution`; validated by `governor review --registry <this file>` (exit=0, claims=0) |
| `grobid-absorption-followup.json` | Planning-only H14..H21 follow-up registry (`claim_scope: planning`): all eight stages unresolved, `implementation_pass=false` / `verification_pass=false`, owners `unassigned`; H18 and H21 name their honest remaining residues without citing a cleared RED or a resolved label discrepancy |
| `HUMAN-REVIEW-TEMPLATE.md` | Evidence-first disposition template for a human reviewer |
| `README.md` | Usage, lifecycle and reusable LLM review prompt |

The PIR register is historical (22 resolved / 0 open). The GAR register is the active corrective register behind `governor review`. A finding may move to `resolved` / `accepted_risk` / `superseded` only with machine-verifiable `completion_evidence` (mode, ref, pass result, artifact paths, observation date, source revision) — a planned `verification` list or prose alone never closes a record.

## Why JSONL

JSONL was selected instead of one large YAML document because it provides:

- one stable `finding_id` per independently reviewable concern;
- streaming and bounded-context LLM processing;
- simple line-oriented diffs;
- independent schema validation;
- easy filtering by priority, area, status or Governor coverage;
- no need to rewrite the whole review when one finding changes.

YAML remains the manifest format because Governor rules and policies are already DATA-driven YAML.

## Trust model

The JSONL is a **review control dataset**, not proof by itself.

- `evidence` points to code, DATA contracts or artifacts.
- `verification` states how the claim can be checked.
- `governor.coverage=existing` means a current Governor rule covers it.
- `partial` means Governor covers only a safety subset.
- `gap` means the proposed rule is not implemented.
- `proposed_rule_id` is a backlog identifier, not an active rule.

The current Governor does **not** ingest this JSONL directly. Native enforcement requires a deterministic scanner/artifact contract plus a YAML rule/dimension and positive/negative tests. Do not report proposed rules as active.

## Core locks preserved

1. D127 remains closed: `import_eligible=false` until explicit user authorization.
2. No auto-promote and no automatic model deployment.
3. Sidecar TEI, figure and LLM outputs are candidate evidence, not graph truth.
4. Samyama remains the sole KG/vector/persistence store.
5. Cacheable compute stages are graph-pure; graph materialization is always observed separately.
6. Replay proof uses physical store outcomes, not logical counters.
7. Graph snapshot acceptance uses same-process execution unless a real remote GraphStore contract exists.
8. Revision-owned content must not silently fall back to versionless identity.
9. Missingness and skips must be typed and visible.

## Reading the dataset

```bash
# Count findings
wc -l doc/review/pipeline-integrity-review.jsonl

# Open P0 findings
uv run python - <<'PY'
import json
from pathlib import Path
for line in Path('doc/review/pipeline-integrity-review.jsonl').read_text().splitlines():
    row = json.loads(line)
    if row['priority'] == 'P0' and row['status'] == 'open':
        print(row['finding_id'], row['title'])
PY

# Governor automation gaps
uv run python - <<'PY'
import json
from pathlib import Path
for line in Path('doc/review/pipeline-integrity-review.jsonl').read_text().splitlines():
    row = json.loads(line)
    if row['governor']['coverage'] == 'gap':
        print(row['finding_id'], row['governor']['proposed_rule_id'])
PY
```

## Schema validation

Validate through uv's isolated environment:

```bash
uv run --with jsonschema python - <<'PY'
import json
from pathlib import Path
import jsonschema
base = Path('doc/review')
schema = json.loads((base / 'pipeline-integrity-review.schema.json').read_text())
rows = (base / 'pipeline-integrity-review.jsonl').read_text().splitlines()
for number, line in enumerate(rows, 1):
    jsonschema.validate(json.loads(line), schema)
print(f'PASS: {len(rows)} JSONL records')
PY
```

Minimal parse/ID/dependency validation without external packages:

```bash
uv run python - <<'PY'
import json
from pathlib import Path
p = Path('doc/review/pipeline-integrity-review.jsonl')
rows = [json.loads(x) for x in p.read_text().splitlines() if x.strip()]
ids = [x['finding_id'] for x in rows]
assert len(ids) == len(set(ids))
known = set(ids)
for row in rows:
    assert set(row['dependencies']) <= known
print(f'PASS: {len(rows)} unique records; dependencies resolve')
PY
```

## Governor workflow

```bash
PYTHONPATH=scripts uv run --with pyyaml python -m governor --validate-yaml
PYTHONPATH=scripts uv run --with pyyaml python -m governor --phase preflight
PYTHONPATH=scripts uv run --with pyyaml python -m governor --phase active
PYTHONPATH=scripts uv run --with pyyaml python -m governor --phase closeout
```

`governor review` selects the registry validator from the contract's declared
`format`. The default GAR contract validates the JSONL register; the horizon
contract selects the H01..H13 stage validator.

```bash
# Default GAR registry
PYTHONPATH=scripts python3 -m governor review
# Long-horizon stage registry
PYTHONPATH=scripts python3 -m governor review --registry scripts/governor/checks/grobid_rust_horizon_review.yaml
```

When implementing a `proposed_rule_id`:

1. Prefer extending `scripts/governor/checks/*.yaml`; do not add a parallel shell guardrail.
2. Add a scanner/adapter only when regex/artifact DATA cannot express the contract.
3. Add a positive fixture and a negative fixture.
4. Run Governor self-tests.
5. Change `governor.coverage` only after the rule is active and verified.
6. Never weaken D127, optimizer or graph-store locks to make a finding pass.

## Finding lifecycle

```text
open → in_progress → resolved
  └────→ blocked
  └────→ accepted_risk
  └────→ superseded
```

A finding may become `resolved` only when:

- every acceptance criterion passes;
- evidence is tied to the tested source revision;
- required negative tests pass;
- relevant Governor and architecture gates pass;
- runtime claims use runtime evidence;
- no safety lock was weakened.

Use `HUMAN-REVIEW-TEMPLATE.md` for subjective or final disposition.

# Example prompt for subsequent LLM analysis and review

Copy the following prompt unchanged, then append any desired scope restriction such as `Review only PIR-001..PIR-008`.

```text
You are reviewing the daily-archive Rust pipeline using an evidence-first, fail-closed protocol.

INPUTS
- doc/review/manifest.yaml
- doc/review/pipeline-integrity-review.schema.json
- doc/review/pipeline-integrity-review.jsonl
- doc/review/HUMAN-REVIEW-TEMPLATE.md
- AGENTS.md
- Relevant ADRs and DATA contracts referenced by each finding
- Current source tree and runtime artifacts

OBJECTIVE
Reassess each selected finding against the current source revision. Determine whether it remains open, is in progress, is blocked, is resolved, is accepted risk, or is superseded. Do not infer resolution from plans, ADR text, milestone summaries, or code presence alone. Require the verification class named in the finding.

MANDATORY STANDARDS
1. Preserve D127: import_eligible=false until explicit user authorization. Never propose weakening Governor guards.
2. No auto-promote and no deployed inference from candidate evaluation.
3. Samyama is the sole KG/vector/persistence store.
4. Sidecar TEI, markdown, figure analyses and LLM outputs are candidate evidence, not graph truth.
5. Cache hit does not prove graph materialization. Verify physical store outcomes separately.
6. Replay requires attempted>0, applied=0, unchanged>0, rejected=0 and unobserved=0.
7. Use same-process snapshot execution unless a real remote GraphStore contract is under test.
8. Revision-owned data must not silently use versionless identities.
9. Missingness, skips and failures must be typed and visible.
10. Respect onion dependencies and DATA-driven ontology/rules.
11. Extraction F1 alone does not prove scientific KG readiness; require semantic graph tasks where applicable.
12. LLM/DSPy/GEPA claims require same-n, independent oracle, contamination, val-gap, latency and cost evidence; no automatic deployment.

METHOD
A. Validate every selected JSONL line against the JSON Schema.
B. Confirm each evidence path exists; inspect the named symbol or artifact rather than relying on the summary.
C. Trace the actual production execution flow. Distinguish da load, legacy CLI, examples and test-only paths.
D. For code changes, assess cache identity, invalidation, revision identity, temporal semantics, context conservation, observability and failure behavior.
E. Run the least expensive relevant verification first. Use per-crate Rust tests; avoid cargo test --workspace. Run the full architecture gate only at closeout.
F. For every proposed Governor rule, state whether it is deterministic enough for YAML/regex, needs a scanner, or still requires human/LLM judgment. Require positive and negative fixtures before calling it active.
G. Do not edit finding IDs. Do not mark a finding resolved unless every acceptance criterion has current evidence.

REQUIRED OUTPUT
Return JSONL only, one record per reviewed finding, with this shape:
{
  "finding_id": "PIR-NNN",
  "previous_status": "open",
  "recommended_status": "open|in_progress|blocked|resolved|accepted_risk|superseded",
  "verdict": "pass|fail|inconclusive|needs_human",
  "tested_source_revision": "git-sha-or-gsd-sha256",
  "evidence_checked": [
    {"ref": "path-or-execution-id", "result": "pass|fail|missing", "note": "bounded factual note"}
  ],
  "acceptance_results": [
    {"criterion": "exact criterion", "result": "pass|fail|inconclusive", "evidence_ref": "..."}
  ],
  "governor_assessment": {
    "coverage": "existing|partial|gap|not_applicable",
    "rule_id": "existing-or-proposed-id-or-null",
    "automation": "yaml_regex|artifact_reader|new_scanner|runtime_check|human|llm",
    "negative_fixture_required": true
  },
  "residual_risk": "what remains after current evidence",
  "next_action": "one bounded executable next step"
}

FINAL CHECKS
- Report missing evidence as missing, never as pass.
- Report example-only behavior as example-only.
- Report candidate-only behavior as candidate-only.
- Do not claim a 200-paper seal unless the exact current lineage and semantic-task acceptance passed.
- Do not expose credentials or secret values.
```

## Maintenance

- Each record carries an `assessment` object with assessment date, source revision and owning wave.
- Keep existing `finding_id` values stable.
- Add new findings at the end with the next ID.
- Update `manifest.yaml` record count and ID range.
- Prefer superseding a materially changed finding over silently rewriting its historical claim.
- Keep findings concise; detailed execution evidence belongs in GSD artifacts or durable runtime reports.
