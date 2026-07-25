from __future__ import annotations

import json
from pathlib import Path

import fitz

from research_graph.application.corpus.parser_replay import ParserReplayUseCase
from research_graph.domain.corpus import ParserReplayDiagnosticCode, ParserReplayStatus
from research_graph.infrastructure.corpus.parsing.replay_adapters import (
    CatalogIndexArticleSelector,
    ExistingFullTextParserAdapter,
    FilesystemParserReplaySourceLoader,
    PageIndexChunkWriterAdapter,
    ParserReplayArtifactWriter,
)


def _write_json(path: Path, payload: dict[str, object]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")


def _catalog_paths(tmp_path: Path, article_id: str = "2605.18747") -> tuple[Path, Path, Path]:
    catalog_parent = tmp_path / "data" / "article_catalog"
    article_dir = catalog_parent / "article_catalog" / "arxiv" / "cs-lg" / article_id
    article_path = article_dir / "article.json"
    index_path = catalog_parent / "article_catalog" / "index.json"
    return catalog_parent, article_path, index_path


def _write_index(index_path: Path, article_id: str = "2605.18747") -> None:
    _write_json(
        index_path,
        {
            "articles": [
                {
                    "article_key": article_id,
                    "article_ref": f"arxiv/cs-lg/{article_id}",
                    "article_path": f"article_catalog/arxiv/cs-lg/{article_id}/article.json",
                }
            ]
        },
    )


def _article_json(*, source_variants: list[dict[str, object]], should_load: bool = True) -> dict[str, object]:
    return {
        "source_variants": source_variants,
        "expected_profile": {"should_load": should_load},
    }


def _substantive_markdown() -> str:
    return "# Title\nThis body line is substantive enough for parser replay.\n"


def test_catalog_index_article_selector_reads_articles_in_order(tmp_path: Path) -> None:
    _catalog_parent, _article_path, index_path = _catalog_paths(tmp_path)
    _write_index(index_path)

    articles = CatalogIndexArticleSelector(index_path).selected_articles()

    assert len(articles) == 1
    assert articles[0].article_id == "2605.18747"
    assert articles[0].article_ref == "arxiv/cs-lg/2605.18747"
    assert articles[0].article_path.endswith("article.json")


def test_filesystem_source_loader_skips_metadata_only_without_source(tmp_path: Path) -> None:
    catalog_parent, article_path, _index_path = _catalog_paths(tmp_path, "metadata-only")
    _write_json(article_path, _article_json(source_variants=[], should_load=False))
    article = CatalogIndexArticleSelector(
        _write_single_index(catalog_parent, "metadata-only")
    ).selected_articles()[0]

    outcome = FilesystemParserReplaySourceLoader(
        catalog_parent=catalog_parent,
        cache_dir=tmp_path / "cache",
    ).load_source(article)

    assert outcome.status == ParserReplayStatus.SKIPPED
    assert outcome.reason == ParserReplayDiagnosticCode.METADATA_ONLY_NO_LOCAL_SOURCE
    assert outcome.message == "metadata_only_no_local_source_artifact"


def test_filesystem_source_loader_skips_missing_source_even_without_expected_profile(
    tmp_path: Path,
) -> None:
    catalog_parent, article_path, index_path = _catalog_paths(tmp_path, "missing-source")
    _write_index(index_path, "missing-source")
    _write_json(article_path, {"source_variants": []})
    article = CatalogIndexArticleSelector(index_path).selected_articles()[0]

    outcome = FilesystemParserReplaySourceLoader(
        catalog_parent=catalog_parent,
        cache_dir=tmp_path / "cache",
    ).load_source(article)

    assert outcome.status == ParserReplayStatus.SKIPPED
    assert outcome.reason == ParserReplayDiagnosticCode.METADATA_ONLY_NO_LOCAL_SOURCE
    assert outcome.message == "metadata_only_no_local_source_artifact"


def test_filesystem_source_loader_ignores_pdf_variant_without_local_path(
    tmp_path: Path,
) -> None:
    catalog_parent, article_path, index_path = _catalog_paths(tmp_path, "metadata-pdf")
    _write_index(index_path, "metadata-pdf")
    _write_json(
        article_path,
        {
            "source_variants": [
                {"source_format": "pdf", "path": None, "capture_status": "not_captured"}
            ],
            "expected_profile": {"should_load": False},
        },
    )
    article = CatalogIndexArticleSelector(index_path).selected_articles()[0]

    outcome = FilesystemParserReplaySourceLoader(
        catalog_parent=catalog_parent,
        cache_dir=tmp_path / "cache",
    ).load_source(article)

    assert outcome.status == ParserReplayStatus.SKIPPED
    assert outcome.reason == ParserReplayDiagnosticCode.METADATA_ONLY_NO_LOCAL_SOURCE
    assert outcome.message == "metadata_only_no_local_source_artifact"


def test_parser_replay_artifact_writer_preserves_legacy_metadata_only_reason(
    tmp_path: Path,
) -> None:
    catalog_parent, article_path, index_path = _catalog_paths(tmp_path, "metadata-only")
    _write_index(index_path, "metadata-only")
    _write_json(article_path, _article_json(source_variants=[], should_load=False))

    result = ParserReplayUseCase(
        article_selector=CatalogIndexArticleSelector(index_path),
        source_loader=FilesystemParserReplaySourceLoader(
            catalog_parent=catalog_parent,
            cache_dir=tmp_path / "cache",
        ),
        full_text_parser=ExistingFullTextParserAdapter(),
        chunk_writer=PageIndexChunkWriterAdapter(tmp_path / "parser-chunking"),
    ).run()
    writer = ParserReplayArtifactWriter(
        output_dir=tmp_path / "parser-chunking",
        events_log=tmp_path / "parser-chunking" / "events.jsonl",
        summary_path=tmp_path / "parser-chunking" / "summary.json",
        schema_version="test-parser-replay.v1",
        repo_root=tmp_path,
    )

    writer.write(result)

    summary = json.loads((tmp_path / "parser-chunking" / "summary.json").read_text())
    events = [
        json.loads(line)
        for line in (tmp_path / "parser-chunking" / "events.jsonl").read_text().splitlines()
        if line.strip()
    ]
    assert summary["skip_reason_counts"] == {"metadata_only_no_local_source_artifact": 1}
    assert events[0]["skip_reason"] == "metadata_only_no_local_source_artifact"


def test_filesystem_source_loader_rejects_low_quality_text_source(tmp_path: Path) -> None:
    catalog_parent, article_path, index_path = _catalog_paths(tmp_path, "low-quality")
    _write_index(index_path, "low-quality")
    source_path = article_path.parent / "article.md"
    source_path.parent.mkdir(parents=True, exist_ok=True)
    source_path.write_text("# Abstract\n## Links\n", encoding="utf-8")
    _write_json(article_path, _article_json(source_variants=[]))
    article = CatalogIndexArticleSelector(index_path).selected_articles()[0]

    outcome = FilesystemParserReplaySourceLoader(
        catalog_parent=catalog_parent,
        cache_dir=tmp_path / "cache",
    ).load_source(article)

    assert outcome.status == ParserReplayStatus.LOW_QUALITY
    assert outcome.reason == ParserReplayDiagnosticCode.LOW_QUALITY_SOURCE
    assert outcome.message == "fallback_reason=no_substantive_body"
    assert outcome.source is not None
    assert outcome.source.text_chars == len("# Abstract\n## Links\n")


def test_filesystem_source_loader_extracts_pdf_to_cache_and_reuses_it(tmp_path: Path) -> None:
    catalog_parent, article_path, index_path = _catalog_paths(tmp_path, "pdf-paper")
    _write_index(index_path, "pdf-paper")
    pdf_path = article_path.parent / "source" / "pdf-paper.pdf"
    pdf_path.parent.mkdir(parents=True)
    doc = fitz.open()
    page = doc.new_page()
    page.insert_text((72, 72), "# Title\nSubstantive PDF body line.")
    doc.save(pdf_path)
    doc.close()
    _write_json(
        article_path,
        _article_json(
            source_variants=[
                {
                    "source_format": "pdf",
                    "path": "article_catalog/arxiv/cs-lg/pdf-paper/source/pdf-paper.pdf",
                }
            ]
        ),
    )
    article = CatalogIndexArticleSelector(index_path).selected_articles()[0]
    loader = FilesystemParserReplaySourceLoader(
        catalog_parent=catalog_parent,
        cache_dir=tmp_path / "cache",
    )

    first = loader.load_source(article)
    second = loader.load_source(article)

    assert first.status == ParserReplayStatus.COMPLETED
    assert first.source is not None
    assert first.source.source_kind == "pdf_converted"
    assert first.source.cache_reused is False
    assert first.source.pdf_pages == 1
    assert Path(first.source.source_path).exists()
    assert second.source is not None
    assert second.source.cache_reused is True


def test_parser_replay_adapters_run_use_case_and_write_compact_page_index(
    tmp_path: Path,
) -> None:
    article_id = "html-paper"
    catalog_parent, article_path, index_path = _catalog_paths(tmp_path, article_id)
    _write_index(index_path, article_id)
    source_path = article_path.parent / "article.md"
    body_text = _substantive_markdown()
    source_path.parent.mkdir(parents=True, exist_ok=True)
    source_path.write_text(body_text, encoding="utf-8")
    _write_json(article_path, _article_json(source_variants=[]))

    result = ParserReplayUseCase(
        article_selector=CatalogIndexArticleSelector(index_path),
        source_loader=FilesystemParserReplaySourceLoader(
            catalog_parent=catalog_parent,
            cache_dir=tmp_path / "cache",
        ),
        full_text_parser=ExistingFullTextParserAdapter(),
        chunk_writer=PageIndexChunkWriterAdapter(tmp_path / "parser-chunking"),
    ).run()

    assert result.succeeded is True
    assert result.completed_count == 1
    assert result.total_chunks > 0
    output_path = tmp_path / "parser-chunking" / "page-index" / f"{article_id}.json"
    artifact = json.loads(output_path.read_text())
    assert artifact["article_ref"] == f"arxiv/cs-lg/{article_id}"
    assert artifact["chunk_count"] == result.total_chunks
    assert "Substantive PDF body" not in output_path.read_text()
    assert "This body line is substantive" not in output_path.read_text()


def _write_single_index(catalog_parent: Path, article_id: str) -> Path:
    index_path = catalog_parent / "article_catalog" / "index.json"
    _write_index(index_path, article_id)
    return index_path
