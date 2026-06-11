# S04: Graph-readiness gate v1 synthesis — UAT

**Milestone:** M057-s70wkm
**Written:** 2026-06-11T09:26:23.059Z

# S04 UAT

## UAT-01: Combined graph manifest exists and is normalized

- Command: `uv run pytest tests/test_m057_s04.py -q`
- Result: PASS, 7 passed.
- Evidence: combined graph has 9403 edges across citation, table_similarity, and figure_similarity.

## UAT-02: Synthesis documents exist and preserve safety posture

- Command: `uv run pytest tests/test_m057_s04.py -q`
- Result: PASS.
- Evidence: REPORT.md, ADR-011, and decision-deferred.md contain required sections, English `is not authorized` wording, 127.0.0.1, and all five safety defaults false.

## UAT-03: Prior M057 slices still regress green

- Command: `uv run pytest tests/test_m057_s01.py tests/test_m057_s02.py tests/test_m057_s03.py tests/test_m057_s04.py -q`
- Result: PASS, 28 passed.

## UAT-04: Guardrail and trajectory remain valid

- Commands: `uv run pytest tests/test_m045_project_trajectory.py tests/test_m044_sidecar_architecture_guardrail.py -q`, `uv run python scripts/verify_m044_sidecar_architecture_guardrail.py`, `uv run python scripts/check_project_trajectory.py`.
- Result: PASS; trajectory verdict is on_track and M044 guardrail exits 0.
