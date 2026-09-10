# Pipeline integrity review package

This directory contains a compact, source-bound review of the daily-archive Rust pipeline, cache invalidation, temporal article structure, context conservation, FSMs, agent readiness and multimodal understanding.

## Current baseline

Recomputed from `pipeline-integrity-review.jsonl` on **2026-08-19** after M401 S05 closed PIR-017 and PIR-021. Do **not** increment the printed M397 table (Resolved 7 / Open 15) — later waves already resolved PIR-006..PIR-016 and PIR-018..PIR-020 in the JSONL while this README stayed stale.

| Status | Count | Findings |
|---|---:|---|
| Resolved | 22 | PIR-001..PIR-022 |
| Open | 0 | — |

An independent controlled 2026-09-10 absorption review now lives in
`grobid-absorption-review-2026-09-10.jsonl` (GAR-001..GAR-016: 15
unresolved / 1 enclosed). Its machine summary is
`grobid-absorption-review-2026-09-10-summary.md`. Narrative is not engine
input: no prose inside this directory requires work, changes scope, or
substitutes for runtime verification.

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
| `grobid-absorption-review-2026-09-10.jsonl` | Active corrective register: GAR-001..GAR-016 (15 unresolved / 1 enclosed); owned by M502-2w7y8k S01..S09 |
| `grobid-absorption-review-2026-09-10.schema.json` | JSON Schema Draft 2020-12 for each GAR record, including `completion_evidence` for enclosed findings |
| `grobid-absorption-review-2026-09-10-summary.md` | Human-readable audit summary, exclusions and ADR correction matrix |
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
