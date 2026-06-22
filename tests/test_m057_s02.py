from __future__ import annotations

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = ROOT / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

# pyrefly: ignore [missing-import]
import m057_table_embed as table_embed  # noqa: E402  # ty:ignore[unresolved-import]
import m057_table_similarity as table_similarity  # noqa: E402  # pyrefly: ignore [missing-import]  # ty:ignore[unresolved-import]
import m057_table_text_build as table_text_build  # noqa: E402  # pyrefly: ignore [missing-import]  # ty:ignore[unresolved-import]

ARTIFACT_ROOT = ROOT / "artifacts" / "m057-fd-marker" / "table-similarity"
CORPUS = ARTIFACT_ROOT / "table-text-corpus.json"
EMBEDDINGS = ARTIFACT_ROOT / "embeddings.json"
EDGES = ARTIFACT_ROOT / "edges.json"
SUMMARY = ARTIFACT_ROOT / "summary.json"
FD_REPORT = ROOT / "artifacts" / "m057-fd-marker" / "fd-validation.json"


def _load_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def _assert_five_false_safety_defaults(payload: dict) -> None:
    safety = payload.get("safety_defaults")
    assert isinstance(safety, dict)
    assert len(safety) == 5
    assert set(safety.values()) == {False}


def test_table_text_corpus_built() -> None:
    payload = _load_json(CORPUS)
    _assert_five_false_safety_defaults(payload)
    tables = payload["tables"]
    assert payload["table_count"] == len(tables)
    assert len(tables) >= 469
    required = {"table_id", "arxiv_id", "table_idx", "text_repr", "source_milestone", "source_pdf"}
    for table in tables:
        assert required <= set(table)
        assert table["table_id"] == f"{table['arxiv_id']}::{table['table_idx']}"
        assert table["text_repr"].startswith(f"Table from {table['arxiv_id']}")
        assert "Columns:" in table["text_repr"]
        assert "Sample:" in table["text_repr"]


def test_table_embeddings_dim_1024() -> None:
    corpus = _load_json(CORPUS)
    payload = _load_json(EMBEDDINGS)
    _assert_five_false_safety_defaults(payload)
    embeddings = payload["embeddings"]
    assert payload["dimensions"] == 1024
    assert payload["embedding_count"] == len(embeddings) == corpus["table_count"]
    assert set(embeddings) == {table["table_id"] for table in corpus["tables"]}
    for embedding in embeddings.values():
        assert len(embedding) == 1024
        assert all(isinstance(value, float) for value in embedding)


def test_table_similarity_edges_threshold() -> None:
    payload = _load_json(EDGES)
    summary = _load_json(SUMMARY)
    _assert_five_false_safety_defaults(payload)
    _assert_five_false_safety_defaults(summary)
    edges = payload["edges"]
    assert payload["edge_count"] == len(edges) == summary["edges_total"]
    assert summary["threshold"] == 0.85
    for edge in edges:
        assert edge["similarity"] > 0.85
        assert edge["evidence"] == "fd_cosine_similarity_0.85"


def test_table_similarity_edges_are_mostly_inter_doc() -> None:
    summary = _load_json(SUMMARY)
    assert summary["edges_total"] == summary["intra_doc_edges"] + summary["inter_doc_edges"]
    assert summary["inter_doc_edges"] >= summary["intra_doc_edges"]


def test_5_safety_defaults() -> None:
    for module in (table_text_build, table_embed, table_similarity):
        assert len(module.SAFETY_DEFAULTS) == 5
        assert set(module.SAFETY_DEFAULTS.values()) == {False}
    for artifact in (CORPUS, EMBEDDINGS, EDGES, SUMMARY):
        _assert_five_false_safety_defaults(_load_json(artifact))


def test_markdown_table_extraction_uses_tmp_path(tmp_path: Path) -> None:
    markdown = tmp_path / "paper.md"
    markdown.write_text(
        "## Results\n\nTable 1: Accuracy.\n\n| Model | Score |\n| --- | ---: |\n| A | 0.90 |\n| B | 0.91 |\n",
        encoding="utf-8",
    )
    tables = table_text_build.extract_markdown_tables(markdown.read_text(encoding="utf-8"))
    assert len(tables) == 1
    assert tables[0].caption == "Table 1: Accuracy."
    assert tables[0].header_row == "Model | Score"
    assert tables[0].sample_rows == ("A | 0.90", "B | 0.91")


def test_m057_s01_fd_regression_report_still_passing() -> None:
    report = _load_json(FD_REPORT)
    _assert_five_false_safety_defaults(report)
    tests = report.get("tests", [])
    assert len(tests) >= 7
    assert all(test.get("passed") is True for test in tests)
    single = next(test for test in tests if test["name"] == "test_single_embedding_1024d")
    assert single["details"]["dimension"] == 1024
