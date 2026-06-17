# Package Architecture Alternatives

## Decision question

What import/package identity should replace the misleading `arxiv_archive` package?

## Alternative 1 — Rename only: `arxiv_archive` to `daily_archive`

Shape:

```text
src/daily_archive/
├── artifacts/
├── ingestion/
├── indexing/
├── llm/
└── ...
```

Example import:

```python
from daily_archive.artifacts.metrics import calculate_article_artifact_metrics
```

### Pros

- Matches the repository/distribution idea more closely than `arxiv_archive`.
- Smaller conceptual rename than a domain rebrand.
- Avoids inventing a broader product name.

### Cons

- Still vague: "daily archive" sounds like a scheduler or archive utility.
- Does not communicate the project's graph/evidence direction.
- Keeps data-source/archive framing rather than research-graph framing.

### Verdict

Better than `arxiv_archive`, but still not a good long-term domain name.

## Alternative 2 — Single coherent domain package: `research_graph`

Shape:

```text
src/research_graph/
├── corpus/
├── papers/
├── graph/
├── staging/
├── llm/
├── evaluation/
├── workflows/
└── cli.py
```

Example import:

```python
from research_graph.papers.artifacts.metrics import calculate_article_artifact_metrics
```

### Pros

- Names the project by outcome, not by input source.
- Supports sources beyond arXiv without renaming again.
- Keeps one installable distribution while still allowing bounded-context subpackages.
- Makes domain boundaries easier to explain: corpus → papers → graph → staging → workflows.
- Lowest-risk path to future splitting: subpackages can become distributions later if they stabilize.

### Cons

- Requires repo-wide import migration over several waves.
- Some current modules are not graph-specific yet and need careful package placement.
- `research_graph` may sound broader than current production readiness; docs should clarify scope.

### Verdict

Recommended.

## Alternative 3 — Multiple top-level packages immediately

Possible shape:

```text
src/research_corpus/
src/research_papers/
src/research_graph/
src/research_llm/
src/research_eval/
```

Example import:

```python
from research_papers.artifacts.metrics import calculate_article_artifact_metrics
from research_graph.readiness.review import validate_review
```

### Pros

- Very explicit bounded contexts.
- Could reduce accidental cross-context imports if each package becomes independently testable.
- Good future state if APIs stabilize.

### Cons

- Premature for the current repo.
- More packaging complexity: pyproject package discovery, import rewrites, test setup, CLI entrypoints, possible circular dependency decisions.
- Harder to migrate safely while many modules are still top-level and workflows cross boundaries.
- Splitting before boundaries stabilize can create artificial package seams.

### Verdict

Do not start here. Revisit after `research_graph` subpackages stabilize.

## Recommendation

Use **Alternative 2**:

```text
src/research_graph/
```

with bounded-context subpackages.

This gives meaningful naming without prematurely splitting the repository into multiple distributions. Future package splits should be treated as separate architecture decisions after import boundaries become stable and tests prove each context can stand alone.

## Naming principle

Avoid package names based on:

- data source only: `arxiv_*`;
- implementation detail: `llm_*`, `kg_*` as top-level identity;
- vague operation: `daily_archive`.

Prefer names based on domain outcome:

```text
research_graph
```

The package means: tools for turning research corpus material into evidence, artifacts, staging contracts, and graph-ready outputs.
