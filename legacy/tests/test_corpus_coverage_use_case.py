from __future__ import annotations

from research_graph.application.corpus.coverage import (
    CatalogCoverageInput,
    CorpusCoverageRequest,
    CorpusCoverageUseCase,
    CoverageSourceArtifact,
    GraphProbeCoverageInput,
    ParserCoverageInput,
)


def _m121_request(*, parser_errors: int = 0) -> CorpusCoverageRequest:
    return CorpusCoverageRequest(
        corpus_id="r024-218-document-corpus-v1",
        catalog=CatalogCoverageInput(
            total_records=166,
            index_entries=221,
            ingested_count=166,
            skipped_count=0,
            failed_count=0,
            source_artifact=CoverageSourceArtifact(
                path="data/r024-218-document-corpus-v1/ingest-summary.json",
                artifact_type="json-summary",
                schema_version="r024-218-ingest-summary.v00.01",
            ),
        ),
        parser=ParserCoverageInput(
            total=221,
            completed=219,
            skipped=2,
            errors=parser_errors,
            chunk_count_total=2576,
            source_kind_counts={"html_native": 21, "pdf_converted": 198},
            skip_reason_counts={"metadata_only_no_local_source_artifact": 2},
            skipped_article_refs=(
                "arxiv/mixed-source/2605.29548",
                "stanford/cs224n/gradient-notes",
            ),
            source_artifact=CoverageSourceArtifact(
                path="data/r024-218-document-corpus-v1/parser-chunking/summary.json",
                artifact_type="json-summary",
                schema_version="r024-218-document-parser-chunking-summary.v00.01",
            ),
        ),
        graph_probe=GraphProbeCoverageInput(
            corpus_size=219,
            skipped_metadata_only=2,
            chunk_count_total=2576,
            n_nodes=3891,
            n_edges=10102,
            citation_relations_count=6212,
            peak_memory_mb=13.81,
            source_artifact=CoverageSourceArtifact(
                path="data/r024-218-document-corpus-v1/networkx-probe/summary.json",
                artifact_type="json-summary",
            ),
        ),
    )


def test_coverage_use_case_aggregates_m121_core_counts() -> None:
    result = CorpusCoverageUseCase().run(_m121_request())

    assert result.corpus_id == "r024-218-document-corpus-v1"
    assert result.catalog_records == 221
    assert result.cumulative_corpus_records == 166
    assert result.parser_total == 221
    assert result.source_backed_records == 219
    assert result.metadata_only_records == 2
    assert result.parser_errors == 0
    assert result.chunk_count_total == 2576
    assert result.source_kind_counts == {"html_native": 21, "pdf_converted": 198}
    assert result.graph_nodes == 3891
    assert result.graph_edges == 10102
    assert result.citation_relations == 6212
    assert result.graph_peak_memory_mb == 13.81
    assert result.succeeded is True


def test_coverage_use_case_records_explicit_denominators() -> None:
    result = CorpusCoverageUseCase().run(_m121_request())
    denominators = {item.name: item for item in result.denominators}

    catalog = denominators["catalog_articles"]
    assert catalog.total == 221
    assert catalog.included == 221
    assert "canonical article catalog index" in catalog.definition

    parser = denominators["parser_replay_articles"]
    assert parser.total == 221
    assert parser.included == 219
    assert parser.excluded == 2
    assert parser.errors == 0

    source_backed = denominators["source_backed_articles"]
    assert source_backed.total == 221
    assert source_backed.included == 219
    assert source_backed.excluded == 2
    assert "local source artifacts" in source_backed.definition


def test_coverage_use_case_preserves_metadata_only_skip_diagnostics() -> None:
    result = CorpusCoverageUseCase().run(_m121_request())

    assert result.skip_reason_counts == {"metadata_only_no_local_source_artifact": 2}
    assert result.skipped_article_refs == (
        "arxiv/mixed-source/2605.29548",
        "stanford/cs224n/gradient-notes",
    )
    assert len(result.diagnostics) == 1
    diagnostic = result.diagnostics[0]
    assert diagnostic.code == "metadata_only_no_local_source_artifact"
    assert diagnostic.count == 2
    assert diagnostic.source_artifact == (
        "data/r024-218-document-corpus-v1/parser-chunking/summary.json"
    )
    assert "parser replay skipped" in diagnostic.notes


def test_coverage_use_case_records_source_artifact_references() -> None:
    result = CorpusCoverageUseCase().run(_m121_request())

    assert [artifact.path for artifact in result.source_artifacts] == [
        "data/r024-218-document-corpus-v1/ingest-summary.json",
        "data/r024-218-document-corpus-v1/parser-chunking/summary.json",
        "data/r024-218-document-corpus-v1/networkx-probe/summary.json",
    ]
    assert all(artifact.artifact_type == "json-summary" for artifact in result.source_artifacts)


def test_coverage_use_case_deduplicates_explicit_source_artifacts() -> None:
    request = _m121_request()
    explicit_artifact = request.parser.source_artifact
    assert explicit_artifact is not None
    result = CorpusCoverageUseCase().run(
        CorpusCoverageRequest(
            corpus_id=request.corpus_id,
            catalog=request.catalog,
            parser=request.parser,
            graph_probe=request.graph_probe,
            source_artifacts=[explicit_artifact],
        )
    )

    assert [artifact.path for artifact in result.source_artifacts].count(explicit_artifact.path) == 1
    assert len(result.source_artifacts) == 3


def test_coverage_use_case_parser_errors_make_result_not_succeeded() -> None:
    result = CorpusCoverageUseCase().run(_m121_request(parser_errors=1))

    assert result.succeeded is False
    assert result.parser_errors == 1
    diagnostics_by_code = {diagnostic.code: diagnostic for diagnostic in result.diagnostics}
    assert diagnostics_by_code["parser_errors"].count == 1
    assert diagnostics_by_code["metadata_only_no_local_source_artifact"].count == 2
