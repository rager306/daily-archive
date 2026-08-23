# M191 GitNexus Parser Scope

## Verdict

**M191 scope is bounded parser readiness expansion using existing M029/M031/parser replay surfaces. It does not claim graph import, production persistence, production retrieval quality, or optimizer readiness.**

## GitNexus planning basis

Primary query:

`parser readiness expansion substantive body source quality labels low quality source zero chunk runtime loader M027 M028 M029 M031`

GitNexus surfaced parser/readiness flows:

| Flow | GitNexus symbol | M191 use | Boundary |
|---|---|---|---|
| M029 readiness verifier | `Function:scripts/verify_m029_unified_readiness.py:check_summary_shape` | Candidate readiness summary verifier. | Use only if local inputs are present; no source success inferred from shape alone. |
| M029 conversion boundary | `Function:scripts/convert_m029_unified_source_quality_boundary.py:convert_captured_row` | Source quality and parser conversion boundary surface. | Do not run unless inputs are local and expected outputs are written first. |
| M031 catalog-backed replay | `Function:scripts/verify_m031_catalog_backed_replay.py:verify_contract` | Candidate catalog-backed acquisition/parser replay verifier and tests. | Metadata-only and fail-closed; no import eligibility promotion. |
| Parser replay use case tests | `tests/test_parser_replay_use_case.py` | Representative fail-closed parser behavior tests. | Low-quality source must not be parsed as success. |
| Chunk baseline measurement | `Function:src/research_graph/infrastructure/repair/chunk_baseline_measurement.py:build_baseline_package` | Existing chunk/readiness measurement surface. | Use through tests/artifacts unless later source edits are impact-checked. |

## Candidate gates

- `tests/test_parser_replay_use_case.py`
- `tests/test_parser_replay_adapters.py`
- `tests/test_m031_catalog_backed_acquisition_loader.py`
- `scripts/verify_m029_unified_readiness.py` if local inputs are present
- `scripts/verify_m031_catalog_backed_replay.py` if local inputs are present
- focused low-quality source criteria tests

## Non-goals

M191 will not:

- activate DSPy/RLM optimizer work;
- claim graph import readiness;
- claim production persistence readiness;
- claim production hybrid retrieval quality;
- treat HTTP 200, non-empty markdown, or abstract-page navigation markdown as parser success;
- promote M031 metadata-only review contracts to import eligibility.

## Scope decision

Proceed to S01 T02 by discovering local command inputs. S02 must write expected parser outputs before S03 runs any parser/readiness execution gates.
