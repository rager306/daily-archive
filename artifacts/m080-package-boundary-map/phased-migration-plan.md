# M080 Phased Package Migration Plan

## Rule of engagement

Do not perform a broad package restructure in one milestone. Every package move must be a small, reversible, compatibility-preserving change with fresh verification.

## Phase 0: guardrails before any move

Before moving a module:

1. Run `gitnexus_impact({target: "<symbol>", direction: "upstream"})` for the module's main public symbols.
2. Search direct imports of the old module path.
3. Identify targeted tests that cover old behavior.
4. Decide the canonical target path.
5. Decide whether the old module needs explicit `__all__` re-exports or a wildcard shim.

Do not move modules with HIGH/CRITICAL impact without pausing for user confirmation.

## Phase 1: low-risk package moves

Start with leaf or recently introduced modules where behavior is covered by focused tests.

Candidates:

- LLM follow-ups under `arxiv_archive.llm` after M079.
- Leaf article artifact helpers with low incoming imports.
- Extraction benchmark helper modules only after benchmark gate tests pass.

Required verification:

```bash
uv run pytest <targeted tests> -q
python3 -m py_compile <new module> <old shim>
```

Add one test proving old/new imports reference the same public object when the module has public functions/classes.

## Phase 2: medium-risk bounded contexts

Move cohesive groups one module at a time:

- `article_artifact_*` -> `arxiv_archive.artifacts.*`
- `validation_batch_*` -> `arxiv_archive.validation.*`
- `hybrid_retrieval.py` and related helpers -> `arxiv_archive.retrieval.*`

Each move should be its own slice or milestone if it touches multiple tests.

Required verification:

- targeted pytest for the moved module's behavior;
- import compatibility smoke;
- GitNexus detect_changes;
- artifact note documenting old path and new path.

## Phase 3: high-risk central modules

Defer central modules until the package map has already proven the shim pattern:

- `full_text.py`
- `universal_kb_contracts.py`
- `graph_readiness.py`
- `models_registry.py`
- `page_index.py` / `article_page_index.py`

These require dedicated milestones with broader test scopes and explicit blast-radius review.

## Phase 4: deprecation cleanup

Only after downstream code has moved to canonical paths:

1. Mark old shim modules as deprecated in docstrings.
2. Search for old import paths.
3. Remove shims only in a separate cleanup milestone.
4. Run broad tests and GitNexus impact/detect_changes.

Do not remove shims in the same milestone that introduces canonical paths.

## Compatibility shim standard

For public modules, prefer explicit re-exports:

```python
"""Compatibility shim for arxiv_archive.domain.module."""

from arxiv_archive.domain.module import PublicClass, public_function

__all__ = ["PublicClass", "public_function"]
```

For private helper modules, wildcard shims may be acceptable, but only if tests prove behavior and no public API is implied.

## Targeted tests standard

Each move must include at least one of:

- existing behavioral tests updated to canonical imports;
- new compatibility test asserting old path re-exports canonical objects;
- import smoke check when no public object is stable.

## Documentation standard

Each move milestone should write a small artifact with:

- old path;
- new path;
- shim decision;
- tests run;
- GitNexus risk;
- remaining imports still using old path.

## Do not

- Do not use broad find-and-replace imports without GitNexus/refactor review.
- Do not move more than one bounded context at a time.
- Do not combine package moves with behavior changes.
- Do not remove compatibility shims immediately.
- Do not move central modules before leaf moves validate the pattern.
- Do not commit or push without separate explicit user confirmation.
