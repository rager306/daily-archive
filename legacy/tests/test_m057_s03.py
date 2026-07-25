from __future__ import annotations

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = ROOT / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

# pyrefly: ignore [missing-import]
import m057_figure_caption_build as figure_caption_build  # noqa: E402  # ty:ignore[unresolved-import]
import m057_figure_embed as figure_embed  # noqa: E402  # pyrefly: ignore [missing-import]  # ty:ignore[unresolved-import]
import m057_figure_similarity as figure_similarity  # noqa: E402  # pyrefly: ignore [missing-import]  # ty:ignore[unresolved-import]

ARTIFACT_ROOT = ROOT / "artifacts" / "m057-fd-marker" / "figure-links"
CORPUS = ARTIFACT_ROOT / "figure-caption-corpus.json"
EMBEDDINGS = ARTIFACT_ROOT / "embeddings.json"
EDGES = ARTIFACT_ROOT / "edges.json"
SUMMARY = ARTIFACT_ROOT / "summary.json"
FD_REPORT = ROOT / "artifacts" / "m057-fd-marker" / "fd-validation.json"
TABLE_SUMMARY = ROOT / "artifacts" / "m057-fd-marker" / "table-similarity" / "summary.json"


def _load_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def _assert_five_false_safety_defaults(payload: dict) -> None:
    safety = payload.get("safety_defaults")
    assert isinstance(safety, dict)
    assert set(safety) == {
        "graph_writes_authorized",
        "production_import_authorized",
        "fact_promotion_authorized",
        "external_network_authorized",
        "llm_calls_authorized",
    }
    assert set(safety.values()) == {False}


def test_figure_caption_corpus_built(tmp_path: Path) -> None:
    payload = _load_json(CORPUS)
    _assert_five_false_safety_defaults(payload)
    figures = payload["figures"]
    assert payload["figure_count"] == len(figures)
    assert len(figures) >= 900
    required = {
        "figure_id",
        "arxiv_id",
        "figure_idx",
        "caption",
        "text_repr",
        "source_milestone",
        "source_pdf",
    }
    assert {source["name"] for source in payload["sources"]} >= {
        "m055deep-opendataloader-correctness",
        "m056-wave-1",
        "m056-missing-17",
    }
    assert any(figure["caption_source"] == "packet-json" for figure in figures)
    assert any(figure["caption_source"] == "markdown" for figure in figures)
    for figure in figures:
        assert required <= set(figure)
        assert figure["figure_id"] == f"{figure['arxiv_id']}::{figure['figure_idx']}"
        assert figure["text_repr"].startswith(f"Figure from {figure['arxiv_id']}: ")
        assert figure["caption"] in figure["text_repr"]

    sample_markdown = """
Intro text.
Figure 1: First caption continues
with enough detail to complete the caption.

Fig. 2. Second caption sentence.
"""
    extracted = figure_caption_build.extract_markdown_figure_captions(sample_markdown)
    assert [item["figure_label"].lower() for item in extracted] == ["figure 1", "fig. 2"]
    assert extracted[0]["caption"].startswith("First caption continues")

    output = tmp_path / "corpus.json"
    rebuilt = figure_caption_build.build_corpus(output_path=output)
    assert output.exists()
    assert rebuilt["figure_count"] == payload["figure_count"]


def test_figure_embeddings_dim_1024() -> None:
    corpus = _load_json(CORPUS)
    payload = _load_json(EMBEDDINGS)
    _assert_five_false_safety_defaults(payload)
    embeddings = payload["embeddings"]
    assert payload["base_url"] == "http://127.0.0.1:8000"
    assert payload["dimensions"] == 1024
    assert payload["batch_size"] == 32
    assert payload["embedding_count"] == len(embeddings) == corpus["figure_count"]
    assert set(embeddings) == {figure["figure_id"] for figure in corpus["figures"]}
    first_vector = next(iter(embeddings.values()))
    assert len(first_vector) == 1024
    assert all(isinstance(value, float) for value in first_vector)


def test_figure_similarity_edges_threshold(tmp_path: Path) -> None:
    edge_payload = _load_json(EDGES)
    summary = _load_json(SUMMARY)
    _assert_five_false_safety_defaults(edge_payload)
    _assert_five_false_safety_defaults(summary)
    edges = edge_payload["edges"]
    assert edge_payload["threshold"] == 0.80
    assert summary["threshold"] == 0.80
    assert summary["edges_total"] == len(edges)
    assert summary["edges_total"] > 0
    assert all(edge["similarity"] > 0.80 for edge in edges)
    assert summary["similarity_stats"]["min"] > 0.80
    assert summary["similarity_stats"]["max"] <= 1.0

    mini_corpus = tmp_path / "mini-corpus.json"
    mini_embeddings = tmp_path / "mini-embeddings.json"
    mini_edges = tmp_path / "mini-edges.json"
    mini_summary = tmp_path / "mini-summary.json"
    mini_corpus.write_text(
        json.dumps(
            {
                "figures": [
                    {"figure_id": "a::1", "arxiv_id": "a", "figure_idx": 1},
                    {"figure_id": "b::1", "arxiv_id": "b", "figure_idx": 1},
                    {"figure_id": "b::2", "arxiv_id": "b", "figure_idx": 2},
                ]
            }
        ),
        encoding="utf-8",
    )
    mini_embeddings.write_text(
        json.dumps(
            {
                "embeddings": {
                    "a::1": [1.0, 0.0, 0.0],
                    "b::1": [0.9, 0.1, 0.0],
                    "b::2": [0.0, 1.0, 0.0],
                }
            }
        ),
        encoding="utf-8",
    )
    edges, mini = figure_similarity.compute_similarity_edges(
        corpus_path=mini_corpus,
        embeddings_path=mini_embeddings,
        edges_path=mini_edges,
        summary_path=mini_summary,
        threshold=0.80,
    )
    assert len(edges) == 1
    assert edges[0]["relation_type"] == "inter-doc"
    assert mini["inter_doc_edges"] == 1
    assert mini["intra_doc_edges"] == 0


def test_figure_similarity_no_intra_doc_edges() -> None:
    edge_payload = _load_json(EDGES)
    summary = _load_json(SUMMARY)
    edges = edge_payload["edges"]
    assert summary["edges_total"] == summary["inter_doc_edges"]
    assert summary["intra_doc_edges"] == 0
    assert edges
    assert all(edge["relation_type"] == "inter-doc" for edge in edges)
    assert all(edge["source_arxiv_id"] != edge["target_arxiv_id"] for edge in edges)


def test_5_safety_defaults() -> None:
    for payload in (
        _load_json(CORPUS),
        _load_json(EMBEDDINGS),
        _load_json(EDGES),
        _load_json(SUMMARY),
    ):
        _assert_five_false_safety_defaults(payload)
    for module in (figure_caption_build, figure_embed, figure_similarity):
        _assert_five_false_safety_defaults({"safety_defaults": module.SAFETY_DEFAULTS})
    assert figure_embed.DEFAULT_BASE_URL == "http://127.0.0.1:8000"


def test_m057_s01_s02_regression_artifacts_still_pass() -> None:
    fd_report = _load_json(FD_REPORT)
    _assert_five_false_safety_defaults(fd_report)
    assert fd_report["base_url"] == "http://127.0.0.1:8000"
    assert fd_report["summary"]["all_passed"] is True
    assert fd_report["summary"]["passed"] == fd_report["summary"]["total"] == 7
    assert fd_report["summary"]["failed"] == 0
    assert fd_report["tests"][1]["details"]["dimension"] == 1024

    table_summary = _load_json(TABLE_SUMMARY)
    _assert_five_false_safety_defaults(table_summary)
    assert table_summary["total_tables"] == 1468
    assert table_summary["edges_total"] == 4934
    assert table_summary["inter_doc_edges"] == 2591
    assert table_summary["intra_doc_edges"] == 2343
    assert table_summary["similarity_stats"]["min"] > table_summary["threshold"]
