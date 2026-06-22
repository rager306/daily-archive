from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MODULE_PATH = ROOT / "scripts" / "audit_m042_connectivity_groups.py"
spec = importlib.util.spec_from_file_location("audit_m042_connectivity_groups", MODULE_PATH)
assert spec is not None
audit_mod = importlib.util.module_from_spec(spec)
sys.modules["audit_m042_connectivity_groups"] = audit_mod
assert spec.loader is not None
spec.loader.exec_module(audit_mod)


def _write_json(path: Path, payload: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    audit_mod.write_json(path, payload)


def _node(article_key: str, category: str, *, linked_from: list[str] | None = None) -> dict:
    entry = {
        "article_key": article_key,
        "article_ref": f"artifact:data/article_catalog/article_catalog/arxiv/cs-ai/{article_key}/article.json",
        "catalog_path": f"arxiv/cs-ai/{article_key}",
        "m041_category": category,
    }
    if linked_from is not None:
        entry["linked_from"] = linked_from  # pyrefly: ignore[bad-assignment]
    return entry


def _manifest(tmp_path: Path) -> Path:
    path = tmp_path / "manifest.json"
    _write_json(
        path,
        {
            "article_count": 5,
            "articles": [
                _node("base-1", "baseline"),
                _node(
                    "linked-1",
                    "reference_linked",
                    linked_from=[
                        "data/article_catalog/article_catalog/arxiv/cs-ai/base-1/source/article.html"
                    ],
                ),
                _node(
                    "linked-2",
                    "reference_linked",
                    linked_from=[
                        "data/article_catalog/article_catalog/arxiv/cs-ai/base-1/source/article.html"
                    ],
                ),
                _node("hermes-1", "hermes_review_section"),
                _node("base-2", "baseline"),
            ],
            "safety_flags": {
                "graph_write_allowed": False,
                "import_eligible": False,
                "production_import_attempted": False,
                "promotion_allowed": False,
            },
        },
    )
    return path


def _repair_report(tmp_path: Path) -> Path:
    path = tmp_path / "repair-report.json"
    _write_json(
        path,
        {
            "records": [
                {"article_key": "linked-1", "after_status": "fetched"},
                {"article_key": "linked-2", "after_status": "fetched"},
            ]
        },
    )
    return path


def test_audit_connectivity_groups_reports_components_and_hermes_group(tmp_path):
    manifest = _manifest(tmp_path)
    repair = _repair_report(tmp_path)

    audit = audit_mod.audit_connectivity(
        manifest_path=manifest, repair_report_path=repair, output_dir=tmp_path / "out"
    )

    assert audit["node_count"] == 5
    assert audit["category_counts"] == {
        "baseline": 2,
        "reference_linked": 2,
        "hermes_review_section": 1,
    }
    assert audit["edge_counts"] == {"local_reference": 2, "selected_node_edges": 2}
    assert audit["largest_component_size"] == 3
    assert audit["components"][0]["article_keys"] == ["base-1", "linked-1", "linked-2"]
    assert audit["isolated_article_count"] == 2
    assert sorted(audit["isolated_articles"]) == ["base-2", "hermes-1"]
    assert audit["hermes_co_selection_group"] == {
        "article_count": 1,
        "article_keys": ["hermes-1"],
        "counts_as_reference_edges": False,
    }
    assert audit["graph_write_allowed"] is False
    assert (tmp_path / "out" / "connectivity-audit.json").exists()
    assert (tmp_path / "out" / "connectivity-audit.md").exists()


def test_audit_connectivity_groups_keeps_external_reference_as_evidence_not_component(tmp_path):
    manifest = tmp_path / "manifest.json"
    _write_json(
        manifest,
        {
            "article_count": 2,
            "articles": [
                _node(
                    "linked-1",
                    "reference_linked",
                    linked_from=[
                        "data/article_catalog/article_catalog/arxiv/cs-ai/external/source/article.html"
                    ],
                ),
                _node("hermes-1", "hermes_review_section"),
            ],
            "safety_flags": {
                "graph_write_allowed": False,
                "import_eligible": False,
                "production_import_attempted": False,
                "promotion_allowed": False,
            },
        },
    )
    repair = _repair_report(tmp_path)

    audit = audit_mod.audit_connectivity(
        manifest_path=manifest, repair_report_path=repair, output_dir=tmp_path / "out"
    )

    assert audit["edge_counts"] == {"local_reference": 1, "selected_node_edges": 0}
    assert audit["component_count"] == 2
    assert audit["largest_component_size"] == 1
    assert audit["evidence_edges"][0]["source"] is None
    assert audit["evidence_edges"][0]["connects_selected_nodes"] is False


def test_audit_connectivity_groups_rejects_enabled_safety_flag(tmp_path):
    manifest = _manifest(tmp_path)
    payload = audit_mod.load_json(manifest)
    payload["safety_flags"]["promotion_allowed"] = True
    _write_json(manifest, payload)

    try:
        audit_mod.audit_connectivity(
            manifest_path=manifest,
            repair_report_path=_repair_report(tmp_path),
            output_dir=tmp_path / "out",
        )
    except ValueError as exc:
        assert "safety flags" in str(exc)
    else:  # pragma: no cover - defensive failure branch
        raise AssertionError("expected ValueError")
