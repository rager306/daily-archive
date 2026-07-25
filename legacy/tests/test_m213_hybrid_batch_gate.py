"""M213 hybrid batch gate: offline fakes, deferred, fail-closed aggregate."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from research_graph.workflows.composition.hybrid_batch_gate import (
    HybridBatchGateRequest,
    HybridBatchGateResult,
    run_hybrid_batch_gate,
)

ROOT = Path(__file__).resolve().parents[1]
SELECTION = ROOT / "artifacts" / "m213-hybrid-gate" / "selection.json"


class _FakeGrobid:
    def extract_metrics(self, pdf_path: Path, *, paper_id: str) -> dict:
        # M217 structured payload so hybrid resolve persists header/cites artifacts.
        return {
            "status": "success",
            "header_title_present": True,
            "bibl_count": 2,
            "structured_parse_ok": True,
            "header": {
                "title": f"Title for {paper_id}",
                "authors": [{"full_name": "A Author"}],
                "import_eligible": False,
                "graph_writes_allowed": False,
            },
            "citations": [
                {"title": "Cite 1", "import_eligible": False},
                {"title": "Cite 2", "import_eligible": False},
            ],
            "citation_count": 2,
        }


class _FakeOdlOk:
    def extract_metrics(self, pdf_path: Path, *, paper_id: str) -> dict:
        md = "# Title\n\n" + ("body paragraph. " * 400)
        return {
            "status": "success",
            "markdown": md,
            "markdown_size_bytes": len(md),
            "bounding_box_count": 12,
            "low_quality_source": False,
        }


class _FakeOdlDown:
    def extract_metrics(self, pdf_path: Path, *, paper_id: str) -> dict:
        return {"status": "unavailable"}


def _two_paper_selection(tmp_path: Path) -> Path:
    payload = json.loads(SELECTION.read_text(encoding="utf-8"))
    payload["papers"] = payload["papers"][:2]
    path = tmp_path / "selection.json"
    path.write_text(json.dumps(payload), encoding="utf-8")
    return path


def test_batch_hybrid_success_with_fakes(tmp_path: Path) -> None:
    sel = _two_paper_selection(tmp_path)
    result = run_hybrid_batch_gate(
        HybridBatchGateRequest(
            selection_path=sel,
            work_dir=tmp_path / "runs",
            enable_live_hybrid=False,
            grobid=_FakeGrobid(),
            opendataloader=_FakeOdlOk(),
            repo_root=ROOT,
            min_hybrid_success=2,
        )
    )
    assert result.paper_count == 2
    assert result.hybrid_success_count == 2
    assert result.gate_pass is True
    assert result.import_eligible_any is False
    assert result.graph_writes_any is False
    assert all(r.body_route == "hybrid" for r in result.rows)
    assert all(r.body_chars >= 5000 for r in result.rows)
    summary = tmp_path / "runs" / "batch-summary.json"
    assert summary.is_file()
    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert payload["hybrid_success_count"] == 2
    assert payload["import_eligible_any"] is False


def test_batch_deferred_when_ports_unavailable(tmp_path: Path) -> None:
    sel = _two_paper_selection(tmp_path)
    result = run_hybrid_batch_gate(
        HybridBatchGateRequest(
            selection_path=sel,
            work_dir=tmp_path / "runs",
            enable_live_hybrid=False,
            grobid=_FakeGrobid(),
            opendataloader=_FakeOdlDown(),
            repo_root=ROOT,
            min_hybrid_success=0,
        )
    )
    assert result.hybrid_success_count == 0
    assert result.hybrid_deferred_count == 2
    assert result.gate_pass is True  # structural + min 0
    assert all(r.hybrid_claimed_success is False for r in result.rows)


def test_batch_fails_min_hybrid_threshold(tmp_path: Path) -> None:
    sel = _two_paper_selection(tmp_path)
    result = run_hybrid_batch_gate(
        HybridBatchGateRequest(
            selection_path=sel,
            work_dir=tmp_path / "runs",
            enable_live_hybrid=False,
            grobid=_FakeGrobid(),
            opendataloader=_FakeOdlDown(),
            repo_root=ROOT,
            min_hybrid_success=1,
        )
    )
    assert result.hybrid_success_count == 0
    assert result.gate_pass is False


def test_result_rejects_import_eligible_flag() -> None:
    with pytest.raises(ValueError, match="cannot authorize"):
        HybridBatchGateResult(
            schema_version="x",
            selection_path="s",
            paper_count=0,
            rows=(),
            hybrid_success_count=0,
            hybrid_deferred_count=0,
            other_route_count=0,
            error_count=0,
            import_eligible_any=True,
            graph_writes_any=False,
            gate_pass=False,
        )
