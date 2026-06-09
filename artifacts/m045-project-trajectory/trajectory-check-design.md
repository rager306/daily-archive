# M045 Trajectory Checker Design

## Problem

The project already has many good controls: GSD requirements/decisions, ADRs, governance mirrors, roadmap gates, GitNexus impact checks, milestone summaries, and verifier scripts. The failure mode is not absence of controls; it is loss of the whole trajectory across sessions and local task focus.

## Interface alternatives

### Design A — Monolithic governance gate

Command: `uv run python scripts/check_project_trajectory.py --strict`

- Reads every known source and fails on any unresolved drift.
- Strong as a release gate.
- Too heavy for everyday planning and risks becoming another brittle mega-verifier.

### Design B — Passive dashboard artifact

Command: `uv run python scripts/check_project_trajectory.py --write-report`

- Generates current-state Markdown/JSON, no pass/fail except parser errors.
- Good for orientation.
- Too weak to prevent drift because it does not classify risks.

### Design C — Thin trajectory wrapper with dimensions and drift flags

Command: `uv run python scripts/check_project_trajectory.py --output-dir artifacts/m045-project-trajectory/current`

- Reads canonical sources and generated mirrors, but remains derived/non-canonical.
- Produces JSON/Markdown with dimensions: architecture, functionality, module/code movement, evidence, safety, operations, next gate.
- Emits `verdict`: `on_track`, `drift_risk`, or `blocked`.
- Emits explicit drift flags with evidence refs, not broad semantic claims.
- Composes existing controls: governance sync, GitNexus detect output when supplied, milestones/reports, decisions/requirements.
- Suitable before planning, after slice completion, and before milestone closeout.

## Recommendation

Use Design C.

It is deep enough to preserve trajectory, but shallow enough to avoid parallel governance. It should not create new source-of-truth records. It should summarize what already exists, identify drift risks, and point to the next gate.

## Report schema

```json
{
  "verdict": "on_track | drift_risk | blocked",
  "dimensions": {
    "architecture": {"status": "...", "evidence": [], "flags": []},
    "functionality": {"status": "...", "evidence": [], "flags": []},
    "module_code": {"status": "...", "evidence": [], "flags": []},
    "evidence": {"status": "...", "evidence": [], "flags": []},
    "safety": {"status": "...", "evidence": [], "flags": []},
    "operations": {"status": "...", "evidence": [], "flags": []},
    "next_gate": {"status": "...", "evidence": [], "flags": []}
  },
  "drift_flags": [],
  "progress_summary": [],
  "next_actions": []
}
```

## Dimension definitions

| Dimension | Question |
|---|---|
| architecture | Are recent decisions/ADRs reflected and not contradicted? |
| functionality | Which requirements were advanced/validated/left active? |
| module_code | Do changed files/modules match planned work and GitNexus scope? |
| evidence | Are claims backed by artifacts/tests/reports? |
| safety | Are no-import/no-promotion/no-raw-payload boundaries preserved? |
| operations | Are long-running services/runtime blockers visible? |
| next_gate | Is the next gate clear and aligned with evidence? |

## Drift flags v1

- `governance_mirror_stale`
- `latest_milestone_missing_summary`
- `latest_milestone_missing_readme_reference`
- `prohibited_claim_detected`
- `unvalidated_requirement_claim`
- `missing_next_gate`
- `git_scope_not_checked`
- `service_state_unreported`
- `architecture_decision_not_reflected`

## Boundaries

- Do not query `.gsd/gsd.db` directly.
- Do not replace GSD requirements/decisions or ADRs.
- Do not replace GitNexus impact/detect checks.
- Do not infer semantic truth from sidecar/parser success.
- Do not authorize graph import, fact promotion, or production writes.

## Usage points

1. Before planning a milestone: orientation and trajectory risk.
2. Before executing a slice: check next gate and safety boundaries.
3. Before closeout/commit: verify docs/evidence/governance freshness.
