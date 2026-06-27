# M185 GitNexus Refresh and Planning Evidence

## Index refresh

GitNexus was refreshed before M185 planning.

```text
failed command: gitnexus analyze --repo daily-archive
reason: current GitNexus CLI does not accept --repo for analyze
help-verified syntax: gitnexus analyze [options] [path]
success command: gitnexus analyze . --name daily-archive --index-only
result: Repository indexed successfully
nodes=43,792
edges=61,033
clusters=936
flows=300
incremental changed=566 added=1216 deleted=89
```

## Planning query themes

GitNexus planning queries were run for:

1. script wrapper extraction and article catalog wrapper tests;
2. manifest/cache lifecycle, invalidation, consumers, and concurrency;
3. write-path inventory canonical scanner and ratchet coupling.

## Initial findings

- Wrapper contract tests already exist around `tests/test_pipeline_script_wrapper_contracts.py`.
- `tests/test_article_catalog_selection.py` now covers the M184 article catalog wrapper pilot.
- Candidate source seams include test architecture audit, pipeline script audit, M025 verifier boundary, and validation evidence helper scripts.
- Four manifest/cache residuals remain no-move until lifecycle proof is complete.
- GitNexus may still return incomplete process context, so M185 will pair impact/context with focused tests and strict drift.

## Planning posture

M185 uses thin waves. Source movement only happens after impact/context review and focused tests. Manifest/cache movement remains fail-closed.
