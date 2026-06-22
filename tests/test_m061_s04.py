from __future__ import annotations

import json
from pathlib import Path

# pyrefly: ignore [missing-import]
from scripts import m061_ingest_to_canonical_catalog as ingest

ROOT = Path(__file__).resolve().parents[1]
REPORT_PATH = ROOT / "artifacts" / "m061-2hop" / "s04-ingest-report.md"
CATALOG_ARXIV_ROOT = ROOT / "data" / "article_catalog" / "article_catalog" / "arxiv"
M061_ROOT = ROOT / "artifacts" / "m061-2hop"
SELECTED_PATHS = sorted(M061_ROOT.glob("anchor-*/acquisition/selected-2hop-papers.json"))
M061_SELECTED_IDS = sorted(
    {
        arxiv_id
        for selected_path in SELECTED_PATHS
        for arxiv_id in json.loads(selected_path.read_text(encoding="utf-8"))["selected_arxiv_ids"]
    }
)


def test_ingest_report_md_exists() -> None:
    assert REPORT_PATH.exists()
    report = REPORT_PATH.read_text(encoding="utf-8")
    assert "## 0. Резюме" in report
    assert "## 5. Lessons + next steps" in report
    assert "Graph writes is not authorized" in report


def test_m061_pdfs_in_canonical_catalog() -> None:
    missing = []
    for arxiv_id in M061_SELECTED_IDS:
        matches = list(CATALOG_ARXIV_ROOT.glob(f"*/{arxiv_id}/source/{arxiv_id}.pdf"))
        if not matches:
            missing.append(arxiv_id)
    assert missing == []


def test_32_unique_arxiv_ids_ingested() -> None:
    assert len(M061_SELECTED_IDS) == 32
    catalog_matches = {
        path.parents[1].name
        for arxiv_id in M061_SELECTED_IDS
        for path in CATALOG_ARXIV_ROOT.glob(f"*/{arxiv_id}/source/{arxiv_id}.pdf")
    }
    assert catalog_matches == set(M061_SELECTED_IDS)


def test_arxiv_category_detected() -> None:
    categories = {
        path.parents[2].name
        for arxiv_id in M061_SELECTED_IDS
        for path in CATALOG_ARXIV_ROOT.glob(f"*/{arxiv_id}/source/{arxiv_id}.pdf")
    }
    assert categories
    assert categories != {ingest.FALLBACK_CATEGORY}
    assert categories & {"cs-cl", "cs-lg", "cs-cv", "cs-ai"}


def test_5_safety_defaults() -> None:
    assert ingest.SAFETY_DEFAULTS == {
        "external_network_authorized": False,
        "graph_writes_authorized": False,
        "production_import_authorized": False,
        "fact_promotion_authorized": False,
        "llm_calls_authorized": False,
    }
    assert ingest.SAFETY_OVERRIDE["external_network_authorized"] is True
    assert "Retry-After" in ingest.SAFETY_OVERRIDE["reason"]  # ty:ignore[unsupported-operator]


def test_idempotent_ingestion(tmp_path: Path) -> None:
    source_root = tmp_path / "m061"
    anchor_dir = source_root / "anchor-0000.00000" / "acquisition"
    pdf_dir = anchor_dir / "pdfs"
    pdf_dir.mkdir(parents=True)
    (anchor_dir / "selected-2hop-papers.json").write_text(
        json.dumps({"count": 1, "selected_arxiv_ids": ["1234.56789"], "source": "test"}),
        encoding="utf-8",
    )
    (pdf_dir / "1234.56789.pdf").write_bytes(b"fake pdf bytes")

    arxiv_root = tmp_path / "catalog" / "article_catalog" / "arxiv"
    calls: list[str] = []

    def fake_fetcher(arxiv_id: str) -> ingest.ArxivMetadata:
        calls.append(arxiv_id)
        return ingest.ArxivMetadata(
            arxiv_id=arxiv_id, category="cs-lg", title="Fixture Paper", source="test"
        )

    first = ingest.ingest_catalog(
        m061_root=source_root,
        arxiv_root=arxiv_root,
        fetcher=fake_fetcher,
        sleep=lambda _: None,
        update_index=False,
    )
    second = ingest.ingest_catalog(
        m061_root=source_root,
        arxiv_root=arxiv_root,
        fetcher=fake_fetcher,
        sleep=lambda _: None,
        update_index=False,
    )

    assert [record.status for record in first.records] == ["ingested"]
    assert [record.status for record in second.records] == ["skipped"]
    assert calls == ["1234.56789"]


def test_5_anchors_all_processed() -> None:
    assert len(SELECTED_PATHS) == 5
    for selected_path in SELECTED_PATHS:
        payload = json.loads(selected_path.read_text(encoding="utf-8"))
        pdf_dir = selected_path.parent / "pdfs"
        assert payload["count"] == 30
        assert len(payload["selected_arxiv_ids"]) == 30
        assert len(list(pdf_dir.glob("*.pdf"))) == 30


def test_m050_m064_s01_s02_s03_regression() -> None:
    trajectory_json = json.loads(
        (ROOT / "artifacts" / "project-trajectory" / "trajectory-report.json").read_text(
            encoding="utf-8"
        )
    )
    trajectory_md = (ROOT / "artifacts" / "project-trajectory" / "trajectory-report.md").read_text(
        encoding="utf-8"
    )
    m044_summary = (
        ROOT / ".gsd" / "milestones" / "M044-qq02k8" / "M044-qq02k8-SUMMARY.md"
    ).read_text(encoding="utf-8")
    serialized = json.dumps(trajectory_json, sort_keys=True) + trajectory_md
    assert "M045" in serialized
    assert trajectory_json["dimensions"]["architecture"]["status"] == "tracked"
    assert trajectory_json["dimensions"]["functionality"]["status"] == "tracked"
    assert "M044" in m044_summary
    assert (ROOT / "artifacts" / "m044-grobid-architecture-guardrail").exists()
    assert trajectory_json.get("graph_write_allowed") is False
    assert trajectory_json.get("production_import_attempted") is False
