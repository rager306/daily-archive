# M175 Command Output Candidates

## Verdict

**Candidate review status: PASS.** The safe command-specific candidate family is exact `src/research_graph/cli/__init__.py` daily CLI durable outputs, excluding the temporary atomic-write path.

## Candidate records

| Line | Current category | Operation | Target | Proposed handling |
|---:|---|---|---|---|
| 232 | caller-owned | open | `filepath` | Move to exact daily CLI output category |
| 261 | temporary | write_text | `temp_path` | Keep `temporary` |
| 348 | caller-owned | write_text | `filepath` | Move to exact daily CLI output category |
| 442 | run-scoped | write_text | `day_dir / 'papers.json'` | Move to exact daily CLI output category |
| 445 | run-scoped | write_text | `day_dir / 'scored.json'` | Move to exact daily CLI output category |
| 448 | run-scoped | write_text | `day_dir / 'overview.json'` | Move to exact daily CLI output category |

## Proposed category

```text
daily-cli-output=5
```

Exact scope:

```text
src/research_graph/cli/__init__.py
```

Allowed target families inside that exact file:

```text
filepath
day_dir / 'papers.json'
day_dir / 'scored.json'
day_dir / 'overview.json'
```

Explicitly excluded:

```text
temp_path -> temporary
src/research_graph/cli/commands/article_artifacts.py -> article-artifact-package
```

## Rationale

The five moved records are durable daily CLI output artifacts: markdown session, JSON session, papers JSON, scored JSON, and overview JSON. The temporary path is implementation detail for atomic replacement and should remain `temporary` so temp writes stay visible.

## Safety rules

- Do not classify generic `filepath` across other files.
- Do not classify generic `day_dir` across other files.
- Do not move the article artifact command package; it is already reviewed.
- Do not move `temp_path`.

## Evidence

- Baseline candidate extraction: `gsd_exec[a229bea5-bae6-44d2-9cdd-b1b120ad219d]`
