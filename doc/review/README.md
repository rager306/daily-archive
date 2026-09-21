# Pipeline integrity review package

This directory contains a compact, source-bound review of the daily-archive Rust pipeline, cache invalidation, temporal article structure, context conservation, FSMs, agent readiness and multimodal understanding.

## Текущая корректирующая волна — 2026-09-21

**M511-j7wz2f** запланирована владельцем по новому [аудиту абсорбции](grobid-absorption-audit-2026-09-21.md).
Воспроизведены Accepted с повреждённым XML и висячей bibliography-ссылкой;
исторические закрытые GAR-записи не означают отсутствия этих новых дефектов.
[Roadmap governor](grobid-absorption-roadmap-2026-09-21.yaml) и
[H14–H21](grobid-absorption-followup.json) — план, не доказательство реализации.
GSD S01–S09 задают исполнение; ADR-076 §14 фиксирует границы.
Production promotion и снятие D127 не разрешены. Human release — только
настоящим ответом владельца; GAR-015 автоматически не закрывается.

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

A planning-only long-horizon register now lives in
`grobid-rust-horizon-review.json` (stages H01..H13, all unresolved by default).
It records the GROBID-to-Rust migration waves with per-stage ADR refs, separate
implementation and verification evidence, tested source, GitNexus/CBM freshness
(recorded here as a documented staleness limitation), negative tests and
ADR-051 matrix rows. `claim_scope: planning` forbids every stage from claiming an
implementation PASS; a closure claim needs `claim_scope: evidence` plus
machine-verifiable `completion_evidence`. Its DATA contract is
`scripts/governor/checks/grobid_rust_horizon_review.yaml`.

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
| `grobid-rust-horizon-review.json` | Planning-only long-horizon register: H01..H13, unresolved by default; no stage may claim a PASS |
| `grobid-rust-horizon-review.schema.json` | JSON Schema Draft 2020-12 for the horizon registry document |
| `manifest.yaml` | Dataset identity, standards, policy and Governor integration |
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
