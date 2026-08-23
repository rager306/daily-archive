# M186 Manifest Lifecycle GitNexus Map

## Verdict

**The four script-only manifest/cache residuals are mapped to tests and lifecycle gaps.**

## Residuals

| Residual | GitNexus surfaces | Movement constraint |
|---|---|---|
| `scripts/benchmark_m055_corpus_manifest.py` | `tests/test_m055_benchmark_s01.py`, inventory tests `test_m183_benchmark_m055_outputs_get_precise_category_without_manifest_movement` | Needs owner, invalidation, consumer, atomicity, lifecycle tests. |
| `scripts/build_m055deep_corpus_manifest_20.py` | `tests/test_m055deep_corpus_20.py`, inventory tests `test_m183_benchmark_m055deep_outputs_get_precise_category` | Needs multi-input invalidation and consumer contract. |
| `scripts/m058_build_graph_manifest.py` | inventory tests `test_m184_graph_connectivity_probe_outputs_get_precise_category_without_manifest`, M060 graph validation tests using M058 manifest | Needs paired-output lifecycle and graph consumer contract. |
| `scripts/m059_build_manifest.py` | `scripts/m059_build_manifest.py::build_m055deep`, `build_all`, `main`; `tests/test_m059_s01.py` | Needs multi-batch lifecycle, rollback/update policy, and schema evolution rules. |

## GitNexus planning implication

The residuals are test-visible but not yet lifecycle-owned. S08 therefore encodes movement gates as data before S09 attempts any atomic writer or S10-S13 touch residual scripts.
