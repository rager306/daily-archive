# M193 Expected Command Outputs

## Verdict contract

M193 resolves graph-readiness review command-layout drift. Success means the current-layout command is proven and the historical `arxiv_archive` command is formally retired without a runtime shim.

## Required execution order

1. Scope and command decision exist.
2. Expected command outputs exist.
3. Canonical current-layout command is verified.
4. Historical command retirement/no-shim tests are verified.
5. Final validation maps observed results to these labels.

## Canonical command

```bash
uv run python -m research_graph.infrastructure.graph.readiness.review \
  --review-dir <review-dir> \
  --events <events.jsonl> \
  --validate-only \
  --require-completed-review
```

## Retired command

```bash
uv run python -m arxiv_archive.graph_readiness_review ...
```

The retired command must remain unavailable unless a future milestone changes package-skeleton governance.

## Expected artifacts

S03:

- `data/architecture-assessment/m193-command-verification-result.md`

S04:

- `data/architecture-assessment/m193-shim-retirement-test-results.md`

S05:

- `data/architecture-assessment/m193-final-validation-evidence.md`
- `data/architecture-assessment/m193-gsd-validation-result.md`
- `data/architecture-assessment/m193-final-closeout-readiness.md`

## Expected labels

- `canonical_command_available=true`
- `canonical_help_passed=true`
- `canonical_validate_only_incomplete_fails_closed=true`
- `canonical_validate_only_completed_passes=true`
- `historical_arxiv_archive_command_available=false`
- `runtime_shim_added=false`
- `import_eligible=false`
- `promoted_to_fact_count=0`
- `production_import_attempted=false`
- `ladybugdb_written=false`
- `direct_extractor_to_graph_write=false`
- `graph_ready=false`
- `production_retrieval_ready=false`
- `optimizer_enabled=false`

## Allowed claims

M193 may claim:

- current-layout graph-readiness review command works;
- historical `arxiv_archive` runtime command is retired;
- completed-review validation semantics are preserved;
- package skeleton no-shim governance remains intact.

## Disallowed claims

M193 must not claim:

- import eligibility;
- semantic KG readiness;
- graph import readiness;
- production graph persistence readiness;
- LadybugDB production write readiness;
- production retrieval quality;
- DSPy/RLM optimizer readiness.

## Stop conditions

Stop before completion if any condition is true:

- current-layout command cannot run help or validate-only mode;
- incomplete review artifacts pass with `--require-completed-review`;
- synthetic completed review artifacts fail with `--require-completed-review`;
- package skeleton no-shim test fails;
- `src/arxiv_archive` runtime shim appears;
- any generated output promotes import eligibility or graph readiness.
