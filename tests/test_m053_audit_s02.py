"""Tests for M053 S02 audit and M043 closeout update."""

from __future__ import annotations

import copy
import json
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

# pyrefly: ignore [missing-import]
import audit_m053_grobid_pilot as audit  # noqa: E402  # ty:ignore[unresolved-import]
import update_m043_target_subset_post_m053 as update_m043  # noqa: E402  # pyrefly: ignore [missing-import]  # ty:ignore[unresolved-import]

SAFETY_KEYS = {
    "graph_import_allowed",
    "graphdb_written",
    "ladybugdb_written",
    "production_import_attempted",
    "import_eligible",
}


def _packet(
    per_pdf_dir: Path,
    paper_id: str,
    *,
    status: str,
    tei_size_bytes: int = 0,
    ref_count: int = 0,
    body_element_count: int = 0,
    http_status: int | None = None,
    attempts: int = 0,
    m022_repair_candidate: bool = False,
    note: str | None = None,
) -> dict[str, Any]:
    packet = {
        "schema_version": "m053-grobid-pilot.v1",
        "generated_at": "2026-06-10T00:00:00+00:00",
        "paper_id": paper_id,
        "pdf_path": f"/tmp/{paper_id}.pdf",
        "grobid_url": "http://localhost:8070",
        "status": status,
        "tei_path": None,
        "tei_size_bytes": tei_size_bytes,
        "ref_count": ref_count,
        "body_element_count": body_element_count,
        "low_quality_source": status == "low_quality_source",
        "http_status": http_status,
        "attempts": [{"attempt": idx + 1} for idx in range(attempts)],
        "m022_repair_candidate": m022_repair_candidate,
        "safety_defaults": dict(audit.SAFETY_DEFAULTS),
    }
    if note:
        packet["note"] = note
    path = per_pdf_dir / f"{paper_id}.json"
    path.write_text(json.dumps(packet, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return {
        "paper_id": paper_id,
        "status": status,
        "packet_path": str(path),
        "tei_path": None,
        "m022_repair_candidate": m022_repair_candidate,
    }


def _m053_inputs(tmp_path: Path) -> tuple[Path, Path]:
    per_pdf_dir = tmp_path / "m053"
    per_pdf_dir.mkdir()
    packets = [
        _packet(
            per_pdf_dir,
            "1804.02767",
            status="success",
            tei_size_bytes=4096,
            ref_count=12,
            body_element_count=42,
            http_status=200,
            attempts=2,
        ),
        _packet(
            per_pdf_dir,
            "2108.12409",
            status="low_quality_source",
            tei_size_bytes=128,
            ref_count=0,
            body_element_count=0,
            http_status=200,
            attempts=1,
            m022_repair_candidate=True,
            note="tei_too_small",
        ),
        _packet(
            per_pdf_dir,
            "2109.10862",
            status="grobid_unavailable",
            attempts=0,
            m022_repair_candidate=True,
            note="dry_run_skipped_grobid_call",
        ),
    ]
    summary = {
        "schema_version": "m053-grobid-pilot.v1",
        "generated_at": "2026-06-10T00:00:00+00:00",
        "total_pdfs": 3,
        "counts": {
            "success": 1,
            "low_quality_source": 1,
            "blocked": 0,
            "grobid_unavailable": 1,
            "network_error": 0,
            "timeout": 0,
        },
        "packets": packets,
        "safety_defaults": dict(audit.SAFETY_DEFAULTS),
    }
    summary_path = per_pdf_dir / "summary.json"
    summary_path.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return summary_path, per_pdf_dir


def _strip_generated_at(markdown: str) -> str:
    return "\n".join(
        "**Generated at:** <generated>" if line.startswith("**Generated at:**") else line
        for line in markdown.splitlines()
    )


def _m043_target(tmp_path: Path) -> Path:
    target = {
        "article_count": 3,
        "article_keys": ["1804.02767", "2108.12409", "2512.24601"],
        "articles": [
            {
                "article_key": "1804.02767",
                "article_ref": "artifact:data/article_catalog/article_catalog/arxiv/cs-cv/1804.02767/article.json",
                "candidate_id": "real-article:1804.02767",
                "candidate_type": "real_article_metadata",
                "catalog_path": "arxiv/cs-cv/1804.02767",
                "linked_from": ["source/article.html"],
                "local_pdf_present_post_m054": {"status": "acquired", "bytes": 123},
                "m041_category": "linked_article",
                "safety_flags": {"graph_import_allowed": False},
            },
            {
                "article_key": "2108.12409",
                "article_ref": "artifact:data/article_catalog/article_catalog/arxiv/cs-cl/2108.12409/article.json",
                "candidate_id": "real-article:2108.12409",
                "candidate_type": "real_article_metadata",
                "catalog_path": "arxiv/cs-cl/2108.12409",
                "linked_from": [],
                "local_pdf_present_post_m054": {"status": "acquired", "bytes": 456},
                "m041_category": "linked_article",
                "safety_flags": {"graph_import_allowed": False},
            },
            {
                "article_key": "2512.24601",
                "article_ref": "artifact:data/article_catalog/article_catalog/arxiv/cs-ai/2512.24601/article.json",
                "candidate_id": "real-article:2512.24601",
                "candidate_type": "real_article_metadata",
                "catalog_path": "arxiv/cs-ai/2512.24601",
                "linked_from": [],
                "local_pdf_present_post_m054": {"status": "not_in_m054_scope", "bytes": 0},
                "m041_category": "seed_article",
                "safety_flags": {"graph_import_allowed": False},
            },
        ],
        "custom_top_level_field": {"preserve": True},
    }
    target_path = tmp_path / "target-subset.json"
    target_path.write_text(json.dumps(target, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return target_path


def _strip_last_updated(payload: dict[str, Any]) -> dict[str, Any]:
    clone = copy.deepcopy(payload)
    clone["last_updated_at"] = "<updated>"
    return clone


def test_audit_emits_per_pdf_table(tmp_path: Path) -> None:
    summary_path, per_pdf_dir = _m053_inputs(tmp_path)
    output = tmp_path / "audit.md"

    markdown = audit.write_audit(summary_path, per_pdf_dir, output)

    assert (
        "| arxiv_id | status | tei_size_bytes | ref_count | body_element_count | m022_repair_candidate | attempts | error |"
        in markdown
    )
    assert "| `1804.02767` | success | 4096 | 12 | 42 | false | 2 | — |" in markdown
    assert (
        "| `2108.12409` | low_quality_source | 128 | 0 | 0 | true | 1 | tei_too_small |" in markdown
    )
    assert output.read_text(encoding="utf-8") == markdown


def test_audit_status_counts(tmp_path: Path) -> None:
    summary_path, per_pdf_dir = _m053_inputs(tmp_path)
    markdown = audit.write_audit(summary_path, per_pdf_dir, tmp_path / "audit.md")

    assert "## Status counts" in markdown
    assert "| success | 1 |" in markdown
    assert "| low_quality_source | 1 |" in markdown
    assert "| blocked | 0 |" in markdown
    assert "| grobid_unavailable | 1 |" in markdown
    assert "| network_error | 0 |" in markdown
    assert "| timeout | 0 |" in markdown


def test_audit_safety_defaults(tmp_path: Path) -> None:
    summary_path, per_pdf_dir = _m053_inputs(tmp_path)
    markdown = audit.write_audit(summary_path, per_pdf_dir, tmp_path / "audit.md")

    assert set(audit.SAFETY_DEFAULTS) == SAFETY_KEYS
    assert all(value is False for value in audit.SAFETY_DEFAULTS.values())
    assert '"graph_import_allowed": false' in markdown
    assert '"graphdb_written": false' in markdown
    assert '"ladybugdb_written": false' in markdown
    assert '"production_import_attempted": false' in markdown
    assert '"import_eligible": false' in markdown
    assert "Production import is not authorized" in markdown


def test_audit_m022_candidates(tmp_path: Path) -> None:
    summary_path, per_pdf_dir = _m053_inputs(tmp_path)
    markdown = audit.write_audit(summary_path, per_pdf_dir, tmp_path / "audit.md")

    assert "## M022 chunk repair candidates" in markdown
    assert "`2108.12409` — status `low_quality_source`" in markdown
    assert "`2109.10862` — status `grobid_unavailable`" in markdown
    assert "`1804.02767` — status `success`" not in markdown


def test_audit_idempotent(tmp_path: Path) -> None:
    summary_path, per_pdf_dir = _m053_inputs(tmp_path)

    first = audit.write_audit(summary_path, per_pdf_dir, tmp_path / "audit-1.md")
    second = audit.write_audit(summary_path, per_pdf_dir, tmp_path / "audit-2.md")

    assert _strip_generated_at(first) == _strip_generated_at(second)


def test_m043_update_preserves_original_fields(tmp_path: Path) -> None:
    summary_path, per_pdf_dir = _m053_inputs(tmp_path)
    target_path = _m043_target(tmp_path)
    original = json.loads(target_path.read_text(encoding="utf-8"))

    updated = update_m043.write_updated_target(summary_path, per_pdf_dir, target_path, target_path)

    assert updated["custom_top_level_field"] == {"preserve": True}
    first_article = updated["articles"][0]
    for key, value in original["articles"][0].items():
        assert first_article[key] == value
    assert first_article["grobid_outcome_post_m053"]["status"] == "success"


def test_m043_update_idempotent(tmp_path: Path) -> None:
    summary_path, per_pdf_dir = _m053_inputs(tmp_path)
    target_path = _m043_target(tmp_path)

    first = update_m043.write_updated_target(summary_path, per_pdf_dir, target_path, target_path)
    second = update_m043.write_updated_target(summary_path, per_pdf_dir, target_path, target_path)

    assert _strip_last_updated(first) == _strip_last_updated(second)


def test_m043_update_safety_defaults(tmp_path: Path) -> None:
    summary_path, per_pdf_dir = _m053_inputs(tmp_path)
    target_path = _m043_target(tmp_path)

    updated = update_m043.write_updated_target(summary_path, per_pdf_dir, target_path, target_path)

    assert set(updated["safety_defaults"]) == SAFETY_KEYS
    assert all(value is False for value in updated["safety_defaults"].values())
    for article in updated["articles"]:
        safety_defaults = article["grobid_outcome_post_m053"]["safety_defaults"]
        assert set(safety_defaults) == SAFETY_KEYS
        assert all(value is False for value in safety_defaults.values())


def test_m043_update_not_in_m053_scope(tmp_path: Path) -> None:
    summary_path, per_pdf_dir = _m053_inputs(tmp_path)
    target_path = _m043_target(tmp_path)

    updated = update_m043.write_updated_target(summary_path, per_pdf_dir, target_path, target_path)

    by_key = {article["article_key"]: article for article in updated["articles"]}
    outcome = by_key["2512.24601"]["grobid_outcome_post_m053"]
    assert outcome["status"] == "not_in_m053_scope"
    assert outcome["attempts"] == 0
    assert outcome["m022_repair_candidate"] is False


def test_m043_update_records_grobid_metrics(tmp_path: Path) -> None:
    summary_path, per_pdf_dir = _m053_inputs(tmp_path)
    target_path = _m043_target(tmp_path)

    updated = update_m043.write_updated_target(summary_path, per_pdf_dir, target_path, target_path)

    by_key = {article["article_key"]: article for article in updated["articles"]}
    success = by_key["1804.02767"]["grobid_outcome_post_m053"]
    assert success["tei_size_bytes"] == 4096
    assert success["ref_count"] == 12
    assert success["body_element_count"] == 42
    assert success["http_status"] == 200
    assert success["attempts"] == 2
