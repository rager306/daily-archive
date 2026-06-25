# M166 P1 Remediation Baseline

## Scope

M166 remediates M165 P1 items 1, 2, and 3 together:

1. Remove `research_graph.cli` import-time environment mutation.
2. Synchronize `doc/onion-layers.md` and ADR-034 with post-M164 four-layer guard enforcement.
3. Record compatibility shim lifecycle policy so deprecated facades do not regrow into canonical APIs.

## Impact

GitNexus impact was run before edits:

| Target | Direction | Risk | Affected processes | Notes |
|---|---|---|---:|---|
| `Function:src/research_graph/cli/__init__.py:run` | upstream | LOW | 0 | Direct callers include CLI-local callback and `run_analysis`. |
| `Function:src/research_graph/cli/__init__.py:run_analysis` | upstream | LOW | 0 | Direct callers include `run` and `run_pipeline`. |

This supports a focused CLI env refactor without broad process blast radius.

## Env mutation baseline

Static evidence: `.gsd/exec/c1507248-a3b9-4e90-86df-d0491d425e50.stdout`

Current source lines:

```text
src/research_graph/cli/__init__.py:17:_env_path = Path(__file__).parent.parent.parent / ".env"
src/research_graph/cli/__init__.py:18:if _env_path.exists():
src/research_graph/cli/__init__.py:25:            os.environ.setdefault(key.strip(), value.strip())
```

Runtime probe evidence: `.gsd/exec/8064da7c-5bfc-4ab6-8453-4b50a5e9f22e.stdout`

Probe result:

```text
mutated-by-import
```

Interpretation: if `src/.env` exists, importing `research_graph.cli` mutates `os.environ`. This is the M165 P1 issue to remove. The process-boundary behavior can remain, but it must be explicit rather than import-time.

## Doc drift baseline

Static evidence found stale ADR-034 lines:

```text
doc/adr/ADR-034-hexagonal-onion-overlay.md:295: domain zero infra imports ... 8 files
doc/adr/ADR-034-hexagonal-onion-overlay.md:296: application zero infra imports ... 6 files
doc/adr/ADR-034-hexagonal-onion-overlay.md:337: Enforcement ... domain+application clean
```

M165 also identified `doc/onion-layers.md` wording that still described the guard as scanning only domain/application. The live post-M164 guard scans four layers:

```text
domain
application
infrastructure
workflows
```

## Shim lifecycle baseline

Current shim evidence includes workflow compatibility modules:

- `src/research_graph/workflows/universal_kb/contracts.py`
- `src/research_graph/workflows/validation/batch_provenance.py`
- `src/research_graph/workflows/validation/batch_state.py`
- `src/research_graph/workflows/validation/logging.py`

M164 closeout states that workflow modules retain compatibility shims and that they can be removed after downstream imports migrate. M165 found this policy was not yet durable as a decision.

## Planned remediation boundaries

### In scope

- Add tests for no import-time CLI env mutation and explicit env application.
- Refactor CLI env setup to an explicit process-boundary function/call.
- Update onion docs and ADR-034 current enforcement text/addendum.
- Save a GSD decision for shim lifecycle policy.
- Document shim lifecycle policy in `doc/onion-layers.md`.

### Out of scope

- Removing compatibility shims immediately.
- Full write-path classification.
- UniversalKBQueue concurrency proof.
- Archive ruff debt.

## Baseline verdict

The three requested P1 items are feasible together in one milestone. The code risk is LOW by GitNexus, and the docs/policy work is additive. The only high-risk edge is preserving CLI process behavior while removing import-time env mutation.
