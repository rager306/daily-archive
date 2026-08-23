# M166 P1 Remediation Closeout

## Verdict

**M166 remediation status: PASS.**

M166 included and completed all requested P1 items 1, 2, and 3 together:

1. **P1 item 1 — CLI import-time env mutation removed.**
2. **P1 item 2 — onion docs and ADR-034 synchronized with post-M164 four-layer guard enforcement.**
3. **P1 item 3 — compatibility shim lifecycle policy recorded and documented.**

## P1 item 1: CLI env mutation

### Before

`research_graph.cli` loaded `src/.env` and called `os.environ.setdefault(...)` at module import time. M166 baseline runtime probe showed:

```text
import research_graph.cli -> M166_IMPORT_MUTATION_PROBE=mutated-by-import
```

### After

`research_graph.cli` now exposes:

```python
apply_cli_env_config(path: str | Path | None = None) -> None
```

Importing `research_graph.cli` no longer mutates `os.environ`. Sync/process-boundary entrypoints call `apply_cli_env_config()` explicitly:

- `run_analysis()`
- `run_pipeline()`
- Typer command `run()`

Post-implementation runtime probe:

```text
import: <unset>
explicit: from-dotenv
```

Focused tests:

```text
uv run pytest tests/test_analysis.py -q
35 passed
```

## P1 item 2: Docs and ADR-034 guard scope

### Before

M165 found stale docs/ADR wording that described the guard as domain/application-only or used old file counts.

### After

`doc/onion-layers.md` now says `scripts/verify_onion_layering.py` scans four guarded source layers:

- `domain`
- `application`
- `infrastructure`
- `workflows`

It also documents:

- infrastructure must not import CLI/workflow/script entry modules,
- workflows must not import local `scripts`,
- reusable script logic belongs in package modules with scripts as thin wrappers.

ADR-034 now has a current-status note:

- M164 expanded enforcement to the full strict boundary matrix,
- current guard evidence supersedes old M104 file counts,
- current guard reports `status=clear`, `violation_count=0`, `allowed_violation_count=0` across all four guarded layers.

## P1 item 3: Shim lifecycle policy

### Decision

D090 records that compatibility shims are deprecated facades:

- new production imports should target canonical domain/application/infrastructure homes,
- shims remain only to preserve legacy imports while downstream callers migrate,
- removal is handled by explicit ratchet cleanup after usage drops or by a future public API support decision.

### Docs

`doc/onion-layers.md` now has `Compatibility shim lifecycle` policy with the same rules.

## Verification

| Check | Result |
|---|---|
| GitNexus pre-edit impact for `run` and `run_analysis` | LOW risk, affected_processes=0 |
| Runtime import baseline before fix | Confirmed mutation |
| Focused CLI env tests after fix | PASS |
| Full `tests/test_analysis.py` | PASS: 35 passed |
| Postimplementation import/explicit env probe | PASS: import `<unset>`, explicit `from-dotenv` |
| Onion guard | PASS: `status=clear`, `violation_count=0`, `allowed_violation_count=0` |
| Test architecture guard | PASS: `status=passed`, `violations=0` |
| Scoped ruff | PASS |
| Pyrefly | PASS: 0 errors |
| Pre-commit | PASS |
| GitNexus detect changes | LOW risk, affected_processes=0 |

## Remaining limitations

M166 intentionally did not address the other M165 P1/P2 backlog items:

- systematic production write-path classification,
- UniversalKBQueue concurrency proof,
- reducing the 3 dynamic script-import and 18 legacy-mixed test allowlist categories,
- broad adapter lifecycle tests beyond the current representative coverage,
- revisiting the historical default CLI dotenv path.

## Files changed

- `src/research_graph/cli/__init__.py`
- `tests/test_analysis.py`
- `doc/onion-layers.md`
- `doc/adr/ADR-034-hexagonal-onion-overlay.md`
- `.gsd/DECISIONS.md`
- `data/architecture-assessment/m166-p1-remediation-baseline.md`
- `data/architecture-assessment/m166-p1-remediation-closeout.md`

## Closeout conclusion

M166 closes the requested P1 items 1, 2, and 3. The repo remains strict on onion import boundaries, and `research_graph.cli` is now safer for async hosts because import no longer mutates process environment.
