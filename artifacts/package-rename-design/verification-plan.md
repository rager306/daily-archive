# Verification Plan for `research_graph` Rename Waves

## Required checks for every wave

### 1. GitNexus impact before edits

Before moving functions/classes/modules with public callers, run `gitnexus_impact` on representative public symbols and report risk.

If GitNexus does not know newly moved/untracked symbols, use direct import search plus targeted tests as primary verification and record the limitation.

### 2. Direct import search

Before and after each wave:

```bash
rg -n "from arxiv_archive|import arxiv_archive|arxiv_archive\." src tests scripts pyproject.toml
rg -n "from research_graph|import research_graph|research_graph\." src tests scripts pyproject.toml
```

After a wave, imports for moved modules must use `research_graph.*`.

### 3. Archive manifest

Each wave writes:

```text
archive/package-rename-waves/wave-XX/manifest.md
```

Manifest must include:

- old path;
- new canonical `research_graph` path;
- archive path;
- moved symbols/modules;
- verification commands;
- known intentional breakage, if old imports are no longer supported.

### 4. Breadcrumb test

Each moved canonical module contains:

```text
Formerly: src/arxiv_archive/<old_path>.py
```

Tests should assert the breadcrumb and archive file exist for critical moved modules.

### 5. Targeted tests

Run the smallest complete behavior suite for the context moved. Examples:

- papers artifacts: artifact metrics/reducer/assets/evidence/minimax/worker/e2e/CLI/scaffold tests;
- corpus ingestion/parsing: loader, downloader, parser, evidence path tests;
- graph readiness: review, manifest, export, persistence, retrieval validation tests;
- LLM helpers: provider config and MiniMax usage tests without live provider calls;
- workflows/CLI: CLI and e2e smoke tests.

### 6. Compile checks

For moved modules and affected importers:

```bash
python3 -m py_compile <moved files> <affected importers>
```

Use `python3`, not `python`.

### 7. GitNexus detect changes

After verification:

```text
gitnexus_detect_changes(scope="all", repo="daily-archive")
```

Expected: low/controlled risk for layout-only waves. If high/critical appears, stop and replan.

## No-go constraints

These remain forbidden during package rename waves unless separately authorized:

- live MiniMax/GLM/DSPy calls;
- secrets or `.env` mutation;
- graph writes;
- fact promotion;
- production LadybugDB writes;
- broad behavior rewrites bundled with import moves.

## Stop conditions

Pause and replan if:

- old `arxiv_archive` imports remain for moved modules after tests;
- a move needs semantic changes beyond imports and breadcrumbs;
- targeted tests require live provider calls/secrets;
- GitNexus shows unexpected high/critical blast radius;
- package name collision appears;
- CLI behavior changes unexpectedly;
- archive manifest cannot explain where old code went.

## Milestone completion criteria

A rename-wave milestone can close only when:

- GSD tasks are complete;
- targeted tests and compile checks pass;
- direct import search is clean for moved modules;
- archive manifest exists;
- GitNexus detect_changes is reviewed;
- GSD validation passes.
