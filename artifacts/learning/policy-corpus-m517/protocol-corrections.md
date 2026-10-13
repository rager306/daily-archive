# M517 preflight corrections

No evaluated cycle used the preflight captures or the original manifest.

- The first runner decoded exporter versions from stdout. Native PyMuPDF diagnostics preceded its JSON object on PDF `2406.07496`; measured features were absent. All 132 preflight captures are retained under `preflight-captures/` and excluded. The repaired runner reads `toc-versions.json` and keeps diagnostics as separate files. It recaptures the whole selected cohort, not only failed records.
- The first 320-PDF manifest had 318 source-family IDs. The full Trench book was assigned to validation while two supplements of that family were assigned to training. `preflight-manifest.json` preserves it. The corrected selection retains one SHA-ordered PDF per source-directory family before the 320-document selection and every-fourth validation split. It has 320 unique family IDs and PDF hashes.
- The two replacement document rows initially lacked `capture_dir`. Deterministic SHA-bound paths were added. No membership, source bytes, partition or metric changed in that path correction.
- Corrected membership and paths were frozen before the first evaluation. All cycles must use the final manifest SHA `dc2c75a045bbba8e9d90b4fc90e3b2fa5bdaf07276be573808bac395164936a1`. The earlier preflight partition is invalid and is not compared with the new fixed validation set.
- The evaluator provenance initially retained stale source/executable fields beside current fields. The final provenance now has one canonical `executable_sha256` and `sources` map. The earlier identity is explicitly preserved in `preflight-evaluator-provenance.json`.

The eight metrics, support floor of four arm observations, five-token grams and quality tie margin of 0.02 did not change. No missing feature, reference or outcome was filled with an invented zero. Production parser routing and graph/import/promotion locks did not change.
