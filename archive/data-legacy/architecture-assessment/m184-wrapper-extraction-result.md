# M184 Wrapper Extraction Result

## Verdict

**Script wrapper extraction pilot: PASS.**

## Extraction

Moved article-catalog default selection construction from `scripts/verify_article_catalog.py::build_default_selection` into application-layer helper:

```text
src/research_graph/application/corpus/article_catalog_selection.py::build_current_catalog_index_selection
```

`verify_article_catalog.py` remains a thin wrapper around the M025 verifier core and now calls the application helper for the no-argument default selection path.

## GitNexus

- Pre-edit impact for `build_default_selection`: LOW, exact, 1 direct caller, no affected processes.
- Final `gitnexus_detect_changes`: LOW risk, affected_processes=0.

## Verification

| Check | Result | Evidence |
|---|---|---|
| Candidate artifact | PASS | `gsd_exec[4c8afed1-e2fa-4e87-8e67-05221c1953a9]` |
| Focused extraction tests | PASS: 2 passed | `gsd_exec[d13e0269-47ad-4678-bf60-df92a770eb45]` |
| Ruff extraction files | PASS | `gsd_exec[1ee59846-5474-459f-8bb2-7200714d32f1]` |
| Wrapper help smoke | PASS | `gsd_exec[08d7003b-62ff-4698-9c21-2c502a282a52]` |
| Inventory strict drift | PASS | `gsd_exec[bf02aace-c2d2-4e87-9526-e92e15aa5255]` |
| Combined focused tests | PASS: 40 passed | `gsd_exec[bc1df500-4d9e-4b83-962a-2a47b83a7869]` |
| Artifact assertions | PASS | `gsd_exec[9a03bc35-ac8b-42d4-a82b-fc8efe6e1b2e]` |

## Pattern established

- Keep script CLI and historical core delegation intact.
- Move reusable selection-shape construction into application layer.
- Add focused helper and wrapper tests.
- Do not introduce speculative ports or factories.
