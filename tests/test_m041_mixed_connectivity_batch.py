from __future__ import annotations

import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]
SCRIPT_DIR = str(ROOT / "scripts")
if SCRIPT_DIR not in sys.path:
    sys.path.insert(0, SCRIPT_DIR)

from scripts import select_m041_mixed_connectivity_batch as selector


def _baseline() -> list[dict]:
    return [
        {
            "article_key": f"baseline-{index}",
            "candidate_id": f"real-article:baseline-{index}",
            "m041_category": "baseline",
            "diagnostics": [],
            "safety_flags": dict(selector.SMOKE_SAFETY_FLAGS),
        }
        for index in range(10)
    ]


def _record(arxiv_id: str) -> object:
    return selector.ArxivRecord(
        arxiv_id=arxiv_id,
        title=f"Title {arxiv_id}",
        summary="metadata summary",
        primary_category="cs.AI",
        published="2026-01-01T00:00:00Z",
        updated="2026-01-01T00:00:00Z",
        abs_url=f"https://arxiv.org/abs/{arxiv_id}",
        pdf_url=f"https://arxiv.org/pdf/{arxiv_id}",
    )


def test_build_mixed_manifest_requires_reference_linked_then_uses_hermes(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    written: dict[str, str] = {}
    monkeypatch.setattr(selector, "load_baseline_entries", _baseline)
    monkeypatch.setattr(
        selector, "known_arxiv_ids", lambda: {f"baseline-{index}" for index in range(10)}
    )
    monkeypatch.setattr(
        selector,
        "discover_reference_ids",
        lambda *, known: {
            "reference_source_count": 1,
            "reference_candidate_count": 5,
            "reference_candidates": [
                "2401.00001",
                "2401.00002",
                "2401.00003",
                "2401.00004",
                "2401.00005",
            ],
            "references_by_source": {
                "source/article.html": [
                    "2401.00001",
                    "2401.00002",
                    "2401.00003",
                    "2401.00004",
                    "2401.00005",
                ]
            },
        },
    )
    monkeypatch.setattr(
        selector,
        "load_hermes_review_candidates",
        lambda *, known: {
            "hermes_digest_ref": "artifact:data/article_corpora/hermes-digest-projection.json",
            "hermes_candidate_count": 5,
            "hermes_candidates": [
                "2501.00001",
                "2501.00002",
                "2501.00003",
                "2501.00004",
                "2501.00005",
            ],
            "hermes_refs_by_id": {"2501.00001": ["R01"]},
            "fallback_reason": None,
        },
    )
    monkeypatch.setattr(
        selector, "fetch_arxiv_records", lambda ids: [_record(arxiv_id) for arxiv_id in ids]
    )
    monkeypatch.setattr(
        selector,
        "fetch_fresh_arxiv_ids",
        lambda *, exclude, count: [f"2601.{index:05d}" for index in range(count)],
    )

    def fake_write(record: object, *, category: str, linked_from: list[str]) -> Path:
        # pyrefly: ignore [missing-attribute]
        written[record.arxiv_id] = category  # ty:ignore[unresolved-attribute]
        # pyrefly: ignore [missing-attribute]
        return tmp_path / category / record.arxiv_id / "article.json"  # ty:ignore[unresolved-attribute]

    def fake_entry(article_path: Path, *, category: str, linked_from: list[str]) -> dict:
        arxiv_id = article_path.parent.name
        return {
            "article_key": arxiv_id,
            "candidate_id": f"real-article:{arxiv_id}",
            "m041_category": category,
            "linked_from": linked_from,
            "diagnostics": [],
            "safety_flags": dict(selector.SMOKE_SAFETY_FLAGS),
        }

    monkeypatch.setattr(selector, "write_arxiv_article", fake_write)
    monkeypatch.setattr(selector, "entry_from_article_path", fake_entry)

    manifest, discovery = selector.build_mixed_manifest(target_count=20)

    assert manifest["article_count"] == 20
    assert manifest["category_counts"] == {
        "baseline": 10,
        "hermes_review_section": 5,
        "reference_linked": 5,
    }
    assert manifest["reference_discovery"]["used_reference_linked_count"] == 5
    assert manifest["hermes_review_selection"]["used_count"] == 5
    assert discovery["category_counts"] == manifest["category_counts"]
    assert all(
        entry["safety_flags"] == selector.SMOKE_SAFETY_FLAGS for entry in manifest["articles"]
    )
    assert written["2401.00001"] == "reference_linked"
    assert written["2501.00001"] == "hermes_review_section"


def test_build_mixed_manifest_rejects_unsafe_target_count() -> None:
    with pytest.raises(ValueError, match="between 20 and 30"):
        selector.build_mixed_manifest(target_count=31, no_network=True)


def test_build_mixed_manifest_no_network_documents_insufficient_articles(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(selector, "load_baseline_entries", _baseline)
    monkeypatch.setattr(selector, "known_arxiv_ids", lambda: set())
    monkeypatch.setattr(
        selector,
        "discover_reference_ids",
        lambda *, known: {
            "reference_source_count": 0,
            "reference_candidate_count": 0,
            "reference_candidates": [],
            "references_by_source": {},
        },
    )

    with pytest.raises(ValueError, match="only built 10 articles"):
        selector.build_mixed_manifest(target_count=20, no_network=True)


def test_write_report_records_no_write_boundary(tmp_path: Path) -> None:
    manifest = {
        "article_count": 20,
        "category_counts": {"baseline": 10, "fresh": 7, "reference_linked": 3},
    }
    discovery = {"reference_candidate_count": 3}
    output = tmp_path / "report.md"

    selector.write_report(output, manifest, discovery)

    text = output.read_text(encoding="utf-8")
    assert "Reference-linked articles used: 3" in text
    assert "Graph write/import/promotion: false" in text
    assert "do not authorize graph import" in text
