"""M247 S02: live expand body root is visible to coverage defaults."""

from __future__ import annotations

from pathlib import Path

from research_graph.workflows.composition.etl_body_coverage import (
    DEFAULT_BODY_ROOTS,
    EtlBodyCoverageRequest,
    run_etl_body_coverage_audit,
)

ROOT = Path(__file__).resolve().parents[1]
EXPAND = ROOT / "artifacts" / "m213-hybrid-gate" / "runs-live-expand"


def test_default_roots_include_expand_path() -> None:
    assert DEFAULT_BODY_ROOTS[0] == Path("artifacts/m213-hybrid-gate/runs-live-expand")


def test_live_expand_body_increases_hybrid_found_if_present() -> None:
    bodies = list(EXPAND.rglob("*.hybrid.body.md")) if EXPAND.is_dir() else []
    if not bodies:
        return
    # expand paper should be countable via default roots
    result = run_etl_body_coverage_audit(
        EtlBodyCoverageRequest(repo_root=ROOT)
    )
    assert result.import_eligible is False
    # M248 expand batch limit-5: expand root holds multiple bodies (was 1, then 6).
    expand_n = len(bodies)
    assert result.package.hybrid_body_found >= 20 + min(expand_n, 20)
    used = " ".join(result.body_roots_used)
    assert "runs-live-expand" in used
    assert result.package.hybrid_body_artifact_files >= result.package.hybrid_body_found
