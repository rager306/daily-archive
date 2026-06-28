# M186 M027 and M030 Catalog Drift Diagnosis

## Verdict

**The M027/M030 baseline failures were data drift, not verifier logic drift.**

## Reproduced failures

`uv run pytest tests/test_m027_mixed_source_catalog.py -q` initially failed with two tests:

- `test_m027_wrapper_emits_local_only_handoff_artifacts`
- `test_m030_requested_ref_intake_closeout_baseline_is_current`

## Drift components

1. **M030 normalized identity gap**
   - `data/article_catalog/index.json` contained `stanford/cs224n/gradient-notes` but lacked `normalized_identity`.
   - The M030 verifier therefore derived `stanford:gradient-notes` instead of the expected `stanford:cs224n:gradient-notes`.

2. **Six canonical article directories missing `article.json`**
   - `arxiv/cs-cl/2606.11189v1`
   - `arxiv/cs-cv/2606.11188v1`
   - `arxiv/cs-lg/2606.11182v1`
   - `arxiv/cs-lg/2606.11190v1`
   - `arxiv/mixed-source/2606.11169v1`
   - `arxiv/mixed-source/2606.11173v1`

3. **Duplicate canonical arXiv identity**
   - `arxiv/cs-cl/2507.19457`
   - `arxiv/cs-lg/2507.19457`
   - Both used article key `2507.19457` and canonical URL `https://arxiv.org/abs/2507.19457`.
   - The `cs-lg` copy was stale duplicate data; the `cs-cl` record had richer canonical evidence and matching PDF bytes.

## GitNexus planning note

GitNexus search pointed to M027/M030 catalog intake and index verification surfaces, but the remediation touched data only, not functions/classes/methods.
