from __future__ import annotations

import json
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = ROOT / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

# M058 plotextractor pilot requires the optional `plotextractor` package,
# which is not installed in light CI/dev environments. Skip the module
# cleanly rather than failing collection.
pytest.importorskip("plotextractor")

# pyrefly: ignore [missing-import]
import m058_compare_v2_vs_m057 as compare_v2  # noqa: E402  # ty:ignore[unresolved-import]
import m058_plotextractor_embed as embed_v2  # noqa: E402  # pyrefly: ignore [missing-import]  # ty:ignore[unresolved-import]
import m058_plotextractor_extract as extract_v2  # noqa: E402  # pyrefly: ignore [missing-import]  # ty:ignore[unresolved-import]
import m058_plotextractor_similarity as similarity_v2  # noqa: E402  # pyrefly: ignore [missing-import]  # ty:ignore[unresolved-import]

ARTIFACT_ROOT = ROOT / "artifacts" / "m058-plotextractor"
SUMMARY = ARTIFACT_ROOT / "summary.json"
CORPUS = ARTIFACT_ROOT / "figure-caption-corpus.json"
EMBEDDINGS = ARTIFACT_ROOT / "embeddings.json"
EDGES = ARTIFACT_ROOT / "edges.json"
COMPARISON = ARTIFACT_ROOT / "v2-vs-m057.json"
DECISION = ARTIFACT_ROOT / "s01-decision.md"
EXPECTED_IDS = {"2605.18747", "2601.05808", "2602.10090", "2507.19457", "1804.02767"}
EXPECTED_SAFETY_DEFAULTS = {
    "graph_writes_authorized": False,
    "production_import_authorized": False,
    "fact_promotion_authorized": False,
    "external_network_authorized": False,
    "llm_calls_authorized": False,
}
FORBIDDEN_LOOPBACK_ALIAS = "local" + "host"


def _load_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def _assert_five_false_safety_defaults(payload: dict) -> None:
    safety = payload.get("safety_defaults")
    assert safety == EXPECTED_SAFETY_DEFAULTS
    # pyrefly: ignore [bad-argument-type]
    assert len(safety) == 5  # ty:ignore[invalid-argument-type]
    # pyrefly: ignore [missing-attribute]
    assert all(value is False for value in safety.values())  # ty:ignore[unresolved-attribute]


def test_plotextractor_5_pdfs_extracted() -> None:
    summary = _load_json(SUMMARY)
    _assert_five_false_safety_defaults(summary)
    assert summary["sample_size"] == 5
    assert set(summary["sample_arxiv_ids"]) == EXPECTED_IDS
    assert summary["tex_ok_count"] == 5
    assert summary["total_figures"] >= 5
    assert summary["total_captions"] >= 5
    for item in summary["per_pdf"]:
        packet = _load_json(ARTIFACT_ROOT / "per-pdf" / f"{item['arxiv_id']}.json")
        _assert_five_false_safety_defaults(packet)
        assert packet["tex_status"] == "ok"
        assert packet["tex_tarball_size"] > 0
        assert packet["figure_count"] == item["figure_count"]
        assert packet["caption_count"] == item["caption_count"]


def test_figure_caption_v2_corpus() -> None:
    corpus = _load_json(CORPUS)
    _assert_five_false_safety_defaults(corpus)
    figures = corpus["figures"]
    assert corpus["figure_count"] == len(figures)
    assert {figure["arxiv_id"] for figure in figures} == EXPECTED_IDS
    assert all(figure["figure_id"].startswith(f"{figure['arxiv_id']}::") for figure in figures)
    assert any(figure.get("label") for figure in figures)
    assert any(figure.get("image_path") for figure in figures)
    assert all(figure.get("caption") for figure in figures)


def test_figure_embeddings_dim_1024() -> None:
    payload = _load_json(EMBEDDINGS)
    _assert_five_false_safety_defaults(payload)
    assert payload["base_url"] == "http://127.0.0.1:8000"
    assert payload["dimensions"] == 1024
    assert payload["figure_count"] == payload["embedding_count"]
    first_vector = next(iter(payload["embeddings"].values()))
    assert len(first_vector) == 1024


def test_figure_similarity_v2_edges() -> None:
    edge_payload = _load_json(EDGES)
    summary = _load_json(SUMMARY)
    _assert_five_false_safety_defaults(edge_payload)
    _assert_five_false_safety_defaults(summary)
    assert edge_payload["threshold"] == 0.75
    assert summary["threshold"] == 0.75
    assert summary["edges_total"] == len(edge_payload["edges"])
    assert summary["inter_doc_edges"] <= summary["edges_total"]
    if edge_payload["edges"]:
        edge = edge_payload["edges"][0]
        assert {"paper_a", "figure_a_idx", "paper_b", "figure_b_idx", "similarity"} <= set(edge)
        assert edge["similarity"] > 0.75


def test_v2_better_than_v1() -> None:
    comparison = _load_json(COMPARISON)
    _assert_five_false_safety_defaults(comparison)
    metrics = comparison["metrics"]
    assert comparison["decision_for_s02"] == "go"
    assert metrics["label_availability"]["winner"] == "v2"
    assert metrics["image_path_availability"]["winner"] == "v2"
    assert metrics["caption_richness_mean_chars"]["v2"] > 0
    assert DECISION.read_text(encoding="utf-8").startswith("# M058 S01 Decision")


def test_5_safety_defaults_and_loopback_alias() -> None:
    for module in (extract_v2, embed_v2, similarity_v2, compare_v2):
        assert module.SAFETY_DEFAULTS == EXPECTED_SAFETY_DEFAULTS
    paths = [
        SCRIPTS / "m058_plotextractor_extract.py",
        SCRIPTS / "m058_plotextractor_embed.py",
        SCRIPTS / "m058_plotextractor_similarity.py",
        SCRIPTS / "m058_compare_v2_vs_m057.py",
        ARTIFACT_ROOT / "v2-vs-m057.md",
        DECISION,
    ]
    for path in paths:
        assert FORBIDDEN_LOOPBACK_ALIAS not in path.read_text(encoding="utf-8")


def test_safe_extract_rejects_path_traversal(tmp_path: Path) -> None:
    import tarfile

    tarball = tmp_path / "bad.tar"
    with tarfile.open(tarball, "w") as archive:
        payload = tmp_path / "payload.txt"
        payload.write_text("bad", encoding="utf-8")
        archive.add(payload, arcname="../escape.txt")
    try:
        extract_v2.safe_extract_tarball(tarball, tmp_path / "out")
    except extract_v2.TexExtractionError as exc:
        assert "escapes extraction directory" in str(exc)
    else:
        raise AssertionError("path traversal tar member was accepted")


def test_m050_m057_regression_artifacts_present() -> None:
    representative_paths = [
        ROOT / "artifacts" / "m050-work-requests",
        ROOT / "artifacts" / "m052-rlm-e2e",
        ROOT / "artifacts" / "m053-grobid-pilot",
        ROOT / "artifacts" / "m054-pdf-acquisition",
        ROOT / "artifacts" / "m055-parser-benchmark",
        ROOT / "artifacts" / "m056-bfs-graph",
        ROOT / "artifacts" / "m057-fd-marker" / "figure-links" / "summary.json",
    ]
    for path in representative_paths:
        assert path.exists(), path
    m057_summary = _load_json(
        ROOT / "artifacts" / "m057-fd-marker" / "figure-links" / "summary.json"
    )
    assert m057_summary["edges_total"] == 15
    assert m057_summary["inter_doc_edges"] == 15
    assert m057_summary["total_figures"] == 937
