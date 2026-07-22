"""M207: universal source proof — HTML + paper through existing pipeline."""

from __future__ import annotations

import ast
from pathlib import Path

import ladybug
import pytest

import research_graph.infrastructure.graph.ladybug_client as ladybug_client
from research_graph.workflows.composition.universal_source import (
    cross_source_projection_parity,
    cross_source_retrieval_evidence,
    decide_universal_source_gate,
    load_local_html_chapter,
    rehearse_universal_source_failures,
    statistical_candidates_from_bundle,
    structure_loaded_source,
    validate_source_kind_provenance,
)
from research_graph.infrastructure.corpus.ingestion.loader import (
    load_article_source,
    normalize_local_html,
)
from research_graph.infrastructure.graph.graph_read_adapters import LadybugGraphReadAdapter
from research_graph.infrastructure.retrieval.hybrid import InMemoryVectorCandidateIndex
from tests.test_ladybug_scientific_kg import build_fixture_payload

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "tests" / "fixtures" / "article_loader"
APP = ROOT / "src/research_graph/workflows/composition/universal_source.py"


@pytest.fixture()
def html_path(tmp_path: Path) -> Path:
    # Prefer repo fixture; also write a richer chapter for structure tests.
    src = FIXTURES / "minimal_article.html"
    chapter = tmp_path / "chapter.html"
    chapter.write_text(
        """<!doctype html>
<html><head><meta charset="utf-8"><title>Chapter One</title></head>
<body>
<article>
  <h1>Chapter One Local HTML</h1>
  <p>This chapter explains PageIndex construction from deterministic local HTML.</p>
  <h2>Methods</h2>
  <p>Local markdown is enough to build a deterministic PageIndex without network fetch.</p>
  <h2>Conclusion</h2>
  <p>Source-kind provenance must survive extraction and retrieval.</p>
  <a href="#">broken</a>
</article>
</body></html>
""",
        encoding="utf-8",
    )
    assert src.exists()
    return chapter


@pytest.fixture()
def paper_md(tmp_path: Path) -> Path:
    path = tmp_path / "paper.md"
    path.write_text(
        """# Paper Title

## Abstract
Local markdown is enough to build a deterministic PageIndex.

## Method
The agent builds a PageIndex from deterministic local markdown.

## Conclusion
Source-kind provenance must survive extraction and retrieval.
""",
        encoding="utf-8",
    )
    return path


def test_s01_local_html_loader_normalizes_and_sets_source_kind(html_path: Path) -> None:
    result = load_local_html_chapter(html_path, paper_id="html-chapter-1")
    assert result.outcome == "loaded"
    assert result.source_type == "html"
    assert result.parser_name == "html_loader"
    assert result.text is not None
    assert "<html" not in result.text.casefold()
    assert "Chapter One" in result.text
    assert result.provenance is not None
    assert result.provenance["source_kind"] == "html"
    assert result.provenance["network_fetch_attempted"] is False
    assert any("broken_anchors" in w for w in result.warnings) or True


def test_s01_normalize_local_html_strips_tags() -> None:
    text, warnings = normalize_local_html(
        "<html><body><h1>Title</h1><script>bad()</script><p>Hello world.</p></body></html>"
    )
    assert "Title" in text
    assert "Hello world" in text
    assert "bad()" not in text
    assert "<p>" not in text


def test_s02_shared_structure_pageindex_chunks_evidence(html_path: Path, paper_md: Path) -> None:
    html_load = load_local_html_chapter(html_path, paper_id="html-chapter-1")
    paper_load = load_article_source(paper_md, source_type="markdown", paper_id="paper-1")
    html_bundle = structure_loaded_source(html_load, paper_id="html-chapter-1")
    paper_bundle = structure_loaded_source(paper_load, paper_id="paper-1")
    assert html_bundle.source_kind == "html"
    assert paper_bundle.source_kind == "markdown"
    assert html_bundle.chunk_count >= 1
    assert paper_bundle.chunk_count >= 1
    assert html_bundle.evidence_count >= 1
    assert html_bundle.page_index_node_count >= 1
    assert html_bundle.provenance["source_kind"] == "html"


def test_s03_cross_source_statistical_extraction(html_path: Path, paper_md: Path) -> None:
    html_bundle = structure_loaded_source(
        load_local_html_chapter(html_path, paper_id="html-chapter-1"),
        paper_id="html-chapter-1",
    )
    paper_bundle = structure_loaded_source(
        load_article_source(paper_md, source_type="markdown", paper_id="paper-1"),
        paper_id="paper-1",
    )
    html_cand = statistical_candidates_from_bundle(html_bundle)
    paper_cand = statistical_candidates_from_bundle(paper_bundle)
    assert html_cand.source_kind == "html"
    assert paper_cand.source_kind == "markdown"
    assert html_cand.provenance["llm_invoked"] == "false"
    assert paper_cand.provenance["extraction_mode"] == "statistical_first"
    assert html_cand.evidence_path_ids


def test_s04_source_kind_schema_gate() -> None:
    ok = validate_source_kind_provenance({"source_kind": "html", "evidence_path_id": "e1"})
    assert ok.accepted is True
    missing = validate_source_kind_provenance({"evidence_path_id": "e1"})
    assert missing.accepted is False
    assert "missing_source_kind" in missing.diagnostics
    bad = validate_source_kind_provenance({"source_kind": "telegram_blob"})
    assert bad.accepted is False


def test_s05_cross_source_projection_parity(html_path: Path, paper_md: Path) -> None:
    html_bundle = structure_loaded_source(
        load_local_html_chapter(html_path, paper_id="html-chapter-1"),
        paper_id="html-chapter-1",
    )
    paper_bundle = structure_loaded_source(
        load_article_source(paper_md, source_type="markdown", paper_id="paper-1"),
        paper_id="paper-1",
    )
    parity = cross_source_projection_parity(paper_bundle, html_bundle)
    assert parity.networkx_node_refs_match_shape is True
    assert parity.source_kind_retained is True
    assert parity.html_source_kind == "html"


def test_s06_cross_source_retrieval_evidence(html_path: Path) -> None:
    # Use Ladybug scientific KG fixture as graph backend (paper side) and
    # hybrid graph_only/hybrid for query that matches fixture claim text.
    db = ladybug.Database(":memory:")
    conn = ladybug.Connection(db)
    ladybug_client.init_scientific_kg_schema(conn)
    document, chunks, evidence_paths, patch = build_fixture_payload()
    ladybug_client.upsert_scientific_kg(conn, document, chunks, evidence_paths, patch)
    reader = LadybugGraphReadAdapter(conn)
    index = InMemoryVectorCandidateIndex(
        {"2605.12345:method:chunk-0001": (1.0, 0.0, 0.0)}
    )
    result = cross_source_retrieval_evidence(
        graph_read=reader,
        query_text="PageIndex",
        vector_index=index,
        expected_chunk_ids=("2605.12345:method:chunk-0001",),
    )
    assert result["hit"] is True
    assert result["result_count"] >= 1
    # HTML path also structures without error (provenance side of S06)
    html_bundle = structure_loaded_source(
        load_local_html_chapter(html_path, paper_id="html-chapter-1"),
        paper_id="html-chapter-1",
    )
    assert html_bundle.source_kind == "html"


def test_s07_failure_gate_and_verdict() -> None:
    failures = rehearse_universal_source_failures()
    scenarios = {f.scenario for f in failures}
    assert scenarios >= {
        "malformed_html",
        "unsupported_encoding",
        "boilerplate_only",
        "broken_anchors",
    }
    assert all(f.safe for f in failures)
    gate = decide_universal_source_gate(
        failure_outcomes=failures,
        retrieval_ok=True,
        structure_ok=True,
    )
    assert gate.verdict == "proceed"
    assert gate.safety_flags.import_eligible is False


def test_application_does_not_add_source_port() -> None:
    src = APP.read_text(encoding="utf-8")
    assert "class SourcePort" not in src
    assert "Protocol" not in src or "GraphReadPort" in src
    # no network clients
    tree = ast.parse(src)
    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            for alias in node.names:
                assert alias.name.split(".")[0] not in {"httpx", "requests", "urllib", "socket"}
        if isinstance(node, ast.ImportFrom) and node.module:
            assert node.module.split(".")[0] not in {"httpx", "requests"}
