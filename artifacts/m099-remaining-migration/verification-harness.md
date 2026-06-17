# M099 Verification and Commit Harness

This harness applies to every remaining package-rename wave after S01.

## Non-negotiable Constraints

- No `git add .` or `git add -A`.
- No push without fresh explicit user confirmation.
- No permanent `src/arxiv_archive` compatibility shims.
- No live MiniMax, GLM, DSPy provider calls, arXiv PDF downloads, Marker calls, graph writes, fact promotion, production imports, or secret logging.
- Old implementations are preserved under `archive/package-rename-waves/wave-XX/` with manifest rows and `Formerly:` breadcrumbs.
- For code edits to functions/classes/methods, run GitNexus impact first and warn before proceeding on HIGH or CRITICAL risk.

## Per-wave Execution Checklist

1. Choose the wave pathset from `target-package-map.md`.
2. Run GitNexus impact for symbols whose definitions will be modified, moved, or renamed.
3. Copy old runtime files into `archive/package-rename-waves/wave-XX/` and add `Formerly:` breadcrumbs.
4. Move canonical runtime files into the selected `research_graph` package.
5. Update all callers and tests atomically.
6. Add or update archive-state tests proving:
   - old runtime `.py` files are absent,
   - archive copies exist,
   - breadcrumbs exist,
   - manifest rows exist,
   - canonical imports resolve.
7. Run package-boundary old-import search.
8. Run targeted local-only tests.
9. Run `py_compile` for moved source and touched scripts.
10. Run `gitnexus_detect_changes()` before commit.
11. Stage only an explicit pathspec file and run forbidden staged-path checks.
12. Commit locally only if verification passes.

## Package-boundary Old-import Search

Use package-boundary patterns, not naive substrings. Example for a wave-specific module set:

```bash
rg --color never -n \
  "arxiv_archive\.(MODULE_A|MODULE_B)(\.|\s|$)|from arxiv_archive\.(MODULE_A|MODULE_B)(\s+import|\.)" \
  src tests scripts \
  --glob '!**/__pycache__/**'
```

For package directories, include import-boundary alternatives:

```bash
rg --color never -n \
  "arxiv_archive\.(identity|staging)(\.|\s|$)|from arxiv_archive\.(identity|staging)(\.|\s+import)" \
  src tests scripts \
  --glob '!**/__pycache__/**'
```

If the search is expected to be clean, use:

```bash
rg --color never -n "<package-boundary-pattern>" src tests scripts --glob '!**/__pycache__/**' && exit 1 || true
```

Do **not** use a pattern that makes `arxiv_archive.chunking` match `arxiv_archive.chunking_benchmark`.

## Archive-state Test Expectations

Archive-state tests should assert old runtime `.py` absence, not old directory absence, because stale ignored `__pycache__` directories can remain.

Suggested assertions per moved file:

- `not Path("src/arxiv_archive/<old>.py").exists()`
- `Path("archive/package-rename-waves/wave-XX/src/arxiv_archive/<old>.py").exists()`
- archive file contains `Formerly: src/arxiv_archive/<old>.py`
- `archive/package-rename-waves/wave-XX/manifest.md` lists old and new paths
- canonical module import succeeds

## Local-only Test Policy

Tests must use fixtures, monkeypatching, or dry-run flags for external boundaries.

Forbidden during migration verification unless separately authorized:

- MiniMax API calls
- GLM/ZAI API calls
- DSPy optimizer/provider execution
- arXiv PDF downloads
- Marker CLI extraction
- graph writes
- fact promotion
- production imports or queue promotion
- printing secrets

## Targeted Test Commands

Choose the smallest local-only test set that covers the moved modules. Examples:

```bash
uv run pytest tests/test_research_graph_package_skeleton.py -q
uv run pytest tests/test_modular_fixture_generation.py tests/test_modular_properties.py -q
```

For each wave, add the tests that directly cover the moved modules. If a test is network-capable, verify it is mocked/local-only before including it.

## py_compile Check

Compile moved source and touched scripts:

```bash
python3 - <<'PY'
from pathlib import Path
import py_compile
paths = [
    # fill from explicit wave pathset
]
for p in paths:
    if Path(p).exists() and Path(p).suffix == '.py':
        py_compile.compile(p, doraise=True)
print(f'py_compile ok: {len(paths)} candidate files')
PY
```

## GitNexus Checks

Before editing symbols:

```text
gitnexus_impact({"target": "<symbol>", "direction": "upstream", "repo": "daily-archive"})
```

Before commit:

```text
gitnexus_detect_changes({"scope": "all", "repo": "daily-archive"})
```

If GitNexus reports a stale index after a local commit, inspect `.gitnexus/meta.json` first. If `stats.embeddings` is `0`, re-index with:

```bash
gitnexus analyze
```

If embeddings are present, preserve them according to project guidance.

## Scoped Staging and Commit

Generate an explicit pathspec:

```bash
python3 - <<'PY'
from pathlib import Path
paths = [
    # Explicit old archive files, new canonical files, touched tests, touched fixtures, touched docs/artifacts.
]
Path('/tmp/m099-waveXX-selected-paths.txt').write_text('\n'.join(paths) + '\n')
print(len(paths))
PY
```

Stage only that pathspec:

```bash
git add --pathspec-from-file=/tmp/m099-waveXX-selected-paths.txt
```

Forbidden staged-path check:

```bash
forbidden=$(git diff --cached --name-only | rg '__pycache__|\.pyc$|^\.gsd/' || true)
if [ -n "$forbidden" ]; then
  printf 'Forbidden staged paths:\n%s\n' "$forbidden" >&2
  exit 1
fi
```

Then inspect staged files:

```bash
git diff --cached --name-status
```

Commit locally only after verification:

```bash
git commit -m "refactor: migrate <wave-name> to research_graph"
```

## Final Milestone UAT

At the end of M099, verify:

```bash
find src/arxiv_archive -path '*/__pycache__' -prune -o -type f -name '*.py' -print
```

Expected result: no production runtime `.py` files remain in `src/arxiv_archive`, except if S12 records an explicit archive-only or package-marker rationale accepted by the user.
