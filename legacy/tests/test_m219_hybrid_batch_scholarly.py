"""M219: hybrid batch gate reports scholarly header/citations artifact metrics."""

from __future__ import annotations

import json
from pathlib import Path

from research_graph.workflows.composition.hybrid_batch_gate import (
    HybridBatchGateRequest,
    run_hybrid_batch_gate,
    scan_scholarly_artifacts,
)

ROOT = Path(__file__).resolve().parents[1]
SELECTION = ROOT / "artifacts" / "m213-hybrid-gate" / "selection.json"


class _FakeGrobidStructured:
    def extract_metrics(self, pdf_path: Path, *, paper_id: str) -> dict:
        return {
            "status": "success",
            "header_title_present": True,
            "bibl_count": 3,
            "structured_parse_ok": True,
            "header": {
                "title": f"Structured {paper_id}",
                "authors": [{"full_name": "X"}],
                "import_eligible": False,
                "graph_writes_allowed": False,
            },
            "citations": [
                {"title": "A", "import_eligible": False},
                {"title": "B", "import_eligible": False},
                {"title": "C", "import_eligible": False},
            ],
            "citation_count": 3,
        }


class _FakeGrobidMetricsOnly:
    def extract_metrics(self, pdf_path: Path, *, paper_id: str) -> dict:
        return {
            "status": "success",
            "header_title_present": True,
            "bibl_count": 5,
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


def _one_paper_selection(tmp_path: Path) -> Path:
    payload = json.loads(SELECTION.read_text(encoding="utf-8"))
    payload["papers"] = payload["papers"][:1]
    path = tmp_path / "selection.json"
    path.write_text(json.dumps(payload), encoding="utf-8")
    return path


def test_scan_scholarly_artifacts_found_and_missing(tmp_path: Path) -> None:
    paper_id = "p1"
    body = tmp_path / "body"
    body.mkdir()
    (body / f"{paper_id}.hybrid.header.json").write_text(
        json.dumps({"title": "Hello"}), encoding="utf-8"
    )
    (body / f"{paper_id}.hybrid.citations.jsonl").write_text(
        json.dumps({"title": "C1"}) + "\n" + json.dumps({"title": "C2"}) + "\n",
        encoding="utf-8",
    )
    found = scan_scholarly_artifacts(tmp_path, paper_id=paper_id)
    assert found["header_found"] is True
    assert found["citations_found"] is True
    assert found["citation_count"] == 2
    assert found["header_title"] == "Hello"
    assert found["import_eligible"] is False

    missing = scan_scholarly_artifacts(tmp_path / "empty", paper_id="nope")
    assert missing["header_found"] is False
    assert missing["citations_found"] is False
    assert missing["citation_count"] == 0


def test_batch_reports_scholarly_when_structured_grobid(tmp_path: Path) -> None:
    sel = _one_paper_selection(tmp_path)
    result = run_hybrid_batch_gate(
        HybridBatchGateRequest(
            selection_path=sel,
            work_dir=tmp_path / "runs",
            enable_live_hybrid=False,
            grobid=_FakeGrobidStructured(),
            opendataloader=_FakeOdlOk(),
            repo_root=ROOT,
            min_hybrid_success=1,
        )
    )
    assert result.gate_pass is True
    assert result.import_eligible_any is False
    assert result.headers_found == 1
    assert result.citations_files_found == 1
    assert result.scholarly_complete_count == 1
    assert result.citation_total == 3
    row = result.rows[0]
    assert row.header_found is True
    assert row.citations_found is True
    assert row.citation_count == 3
    assert row.header_title and "Structured" in row.header_title
    # Artifacts on disk under work_dir/{paper_id}/body/
    paper_id = row.paper_id
    body_dir = tmp_path / "runs" / paper_id / "body"
    assert (body_dir / f"{paper_id}.hybrid.header.json").is_file()
    assert (body_dir / f"{paper_id}.hybrid.citations.jsonl").is_file()
    summary = json.loads((tmp_path / "runs" / "batch-summary.json").read_text(encoding="utf-8"))
    assert summary["schema_version"].startswith("m219")
    assert summary["scholarly_wrapper"]["citation_total"] == 3
    assert summary["scholarly_wrapper"]["import_eligible"] is False


def test_batch_metrics_only_grobid_reports_zero_scholarly(tmp_path: Path) -> None:
    """Legacy metrics-only GROBID payload → no header/cites files → zeros."""
    sel = _one_paper_selection(tmp_path)
    result = run_hybrid_batch_gate(
        HybridBatchGateRequest(
            selection_path=sel,
            work_dir=tmp_path / "runs",
            enable_live_hybrid=False,
            grobid=_FakeGrobidMetricsOnly(),
            opendataloader=_FakeOdlOk(),
            repo_root=ROOT,
            min_hybrid_success=1,
        )
    )
    assert result.hybrid_success_count == 1
    assert result.gate_pass is True
    assert result.headers_found == 0
    assert result.citation_total == 0
    assert result.scholarly_complete_count == 0
