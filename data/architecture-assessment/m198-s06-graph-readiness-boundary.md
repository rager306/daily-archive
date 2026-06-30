# M198 S06 Graph Readiness Validate Only Boundary

## Verdict

**PASS: S06 may add a validate-only graph readiness probe, but must not enable graph import, schema migration, backend writes, or retired alias restoration.**

## GitNexus evidence

| Target | Result | Scope decision |
|---|---|---|
| `Function:src/research_graph/infrastructure/graph/readiness/review.py:main` | LOW, impacted_count=1, affected_processes=[] | Current CLI input; do not edit. |
| `Function:src/research_graph/infrastructure/graph/readiness/review.py:validate_review_artifacts` | LOW, impacted_count=2, module=Readiness | Current validator input; do not edit. |

## Current module path

Use:

```bash
uv run python -m research_graph.infrastructure.graph.readiness.review \
  --review-dir <review-dir> \
  --events <events.jsonl> \
  --validate-only \
  --require-completed-review
```

Do **not** restore or depend on retired alias:

```bash
uv run python -m arxiv_archive.graph_readiness_review
```

Runtime probe evidence: `gsd_exec[0cad54cb-866a-40f5-9e9b-969888d1e609]` shows the retired alias is absent.

## Minimum completed-review fixture shape

Runtime probe evidence: `gsd_exec[2dfce8e0-ac64-4e4b-bcca-5eea129eb07d]`.

A validate-only success requires:

- one `*-review.md` bundle file;
- `independent-review-summary.md` without unreplaced placeholders;
- an `events.jsonl` line with:
  - `event="independent_review.verdict"`;
  - `verdict="PASS"` or another accepted final verdict;
  - `output_contract_completed=true`.

## Allowed S06 edits

- `scripts/run_m198_graph_readiness_probe.py`
- `tests/test_m198_graph_readiness_probe.py`
- S06 architecture assessment artifacts

## Disallowed S06 edits

- `src/research_graph/infrastructure/graph/readiness/review.py`
- `src/research_graph/infrastructure/graph/*` backend/import code
- schema migration code
- retired `arxiv_archive.graph_readiness_review` alias restoration
- Universal KB queue/rehearsal/smoke runtime code

## Required probe behavior

- Creates or accepts completed-review metadata-only fixture artifacts.
- Runs current validate-only CLI with `--require-completed-review`.
- Writes one `m198.readiness_evidence.v1` JSON file.
- Uses `source_kind=graph_readiness_validate_only`.
- Preserves `graph_writes_allowed=false`, `schema_migration_allowed=false`, and `import_eligible=false`.
- Records validator command, review refs, events refs, diagnostics, checksums, alias absence, and non-goals.
- Rejects missing summary, missing completed verdict, bad import flags, and forbidden payload-shaped terms.

## Downstream dependency map

- S07 consumes S06 evidence for graph-readiness drift classification.
- S08 consumes S06 evidence for metadata-only evidence indexing.
