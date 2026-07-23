"""M247 S01: DEFAULT_BODY_ROOTS includes expand run root (coverage join)."""

from __future__ import annotations

from pathlib import Path

from research_graph.workflows.composition.etl_body_coverage import DEFAULT_BODY_ROOTS


def test_expand_root_is_first() -> None:
    assert len(DEFAULT_BODY_ROOTS) >= 5
    first = DEFAULT_BODY_ROOTS[0]
    assert first == Path("artifacts/m213-hybrid-gate/runs-live-expand")
    # first-root-wins for unique paper_ids in fleet/coverage scans
    assert str(first).endswith("runs-live-expand")


def test_legacy_roots_still_present() -> None:
    roots = {str(p) for p in DEFAULT_BODY_ROOTS}
    assert "artifacts/m213-hybrid-gate/runs-live-20" in roots
    assert "artifacts/m213-hybrid-gate/runs-live" in roots
    assert "artifacts/m213-hybrid-gate/runs-live-scholarly-20" in roots
    assert "artifacts/m213-hybrid-gate/runs-live-scholarly" in roots


def test_no_duplicate_roots() -> None:
    assert len(DEFAULT_BODY_ROOTS) == len(set(DEFAULT_BODY_ROOTS))
