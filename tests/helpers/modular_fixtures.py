"""Canonical Adaptix fixture helpers for refactored article modules.

The helpers in this module intentionally build small, representative contract
objects through the same dataclasses and public builders used by the production
modules.  Tests can reuse these shapes instead of carrying local one-off samples
for loader, parser, PageIndex, chunking, asset, identity, and staging contracts.
"""

from __future__ import annotations

import json
from dataclasses import replace
from pathlib import Path
from typing import Any, TypeVar

from adaptix import Retort

from research_graph.papers.artifacts.models import build_article_artifact_manifest_from_structure
from research_graph.papers.indexing.page_index import build_article_page_index_from_structure
from research_graph.papers.source_assets.registry import AssetRecord, PreservedSourceFile, SourceSpan as AssetSourceSpan
from research_graph.papers.chunking.chunker import StructureAwarePackage, parse_markdown_structure
from arxiv_archive.identity.canonicalization import canonical_source_id, stable_json_hash
from research_graph.papers.indexing.navigation import PageIndexDocument
from research_graph.papers.indexing.parsed_page_index import build_page_index_from_parsed
from research_graph.corpus.ingestion.loader import (
    ArticleLoadResult,
    FullTextIngestionResult,
    FullTextQualityReport,
)
from research_graph.corpus.parsing.parser import parse_article
from research_graph.corpus.parsing.structure import ParsedArticle
from arxiv_archive.staging.import_boundary import ImportBoundaryRehearsal, ImportCandidate

T = TypeVar("T")

MODULAR_FIXTURE_DIR = Path(__file__).resolve().parents[1] / "fixtures" / "modular"
MODULAR_FIXTURE_PATH = MODULAR_FIXTURE_DIR / "canonical_contract_samples.json"
MODULAR_RETORT = Retort()
FIXTURE_PAPER_ID = "fixture-paper-modular-0001"
FIXTURE_SOURCE_PATH = Path("tests/fixtures/modular/fixture-paper-modular-0001.md")
FIXTURE_MARKDOWN = """# Fixture Paper

## Methods
A deterministic parser boundary builds stable identifiers.

## Results
Figure and table references stay metadata-only for review.
"""


def adaptix_dump(value: object) -> Any:
    """Dump a fixture object through the shared Adaptix retort."""
    return _jsonable(MODULAR_RETORT.dump(value))


def adaptix_load(payload: Any, target_type: type[T]) -> T:
    """Load a fixture object through the shared Adaptix retort."""
    loaded = MODULAR_RETORT.load(payload, target_type)
    if isinstance(loaded, PageIndexDocument):
        root = loaded.node_by_id(loaded.root.id)
        if root is not None:
            loaded.root = root
    return loaded


def sample_quality_report() -> FullTextQualityReport:
    """Return a representative loader quality report."""
    return FullTextQualityReport(
        status="ok",
        char_count=len(FIXTURE_MARKDOWN),
        line_count=len(FIXTURE_MARKDOWN.splitlines()),
        heading_count=3,
        non_heading_nonempty_line_count=2,
        warnings=[],
        fallback_reason=None,
    )


def sample_article_load_result() -> ArticleLoadResult:
    """Return the canonical loader result shape used by modular tests."""
    source_id = canonical_source_id(FIXTURE_PAPER_ID, "normalized-md")
    return ArticleLoadResult(
        source_path=FIXTURE_SOURCE_PATH,
        source_type="markdown",
        media_type="text/markdown",
        sha256=stable_json_hash({"markdown": FIXTURE_MARKDOWN}),
        byte_size=len(FIXTURE_MARKDOWN.encode("utf-8")),
        source_id=source_id,
        parser_name="fixture_markdown_parser",
        loader_name="fixture_loader",
        outcome="loaded",
        failure_reason=None,
        warnings=[],
        duration_ms=7,
        paper_id=FIXTURE_PAPER_ID,
        text=FIXTURE_MARKDOWN,
        quality=sample_quality_report(),
        provenance={
            "source_id": source_id,
            "fixture": "modular_contract_samples",
        },
    )


def sample_parsed_article() -> ParsedArticle:
    """Return parsed fixture article output through the parser boundary."""
    ingestion = FullTextIngestionResult(
        paper_id=FIXTURE_PAPER_ID,
        source_type="markdown",
        source_path=FIXTURE_SOURCE_PATH,
        text=FIXTURE_MARKDOWN,
        extraction_mode="structured_markdown",
        warnings=[],
        fallback_reason=None,
        quality=sample_quality_report(),
        provenance={"fixture": "modular_contract_samples"},
    )
    return parse_article(ingestion)


def sample_page_index_document():
    """Return the canonical PageIndex fixture document."""
    return build_page_index_from_parsed(sample_parsed_article())


def sample_structure_aware_package() -> StructureAwarePackage:
    """Return a representative structure-aware chunking package."""
    package = parse_markdown_structure(
        FIXTURE_MARKDOWN,
        paper_id=FIXTURE_PAPER_ID,
        title="Fixture Paper",
        source_artifact=str(FIXTURE_SOURCE_PATH),
        categories=("cs.AI", "cs.IR"),
        run_id="modular-fixture-generation",
    )
    return replace(package, created_at="2026-05-31T00:00:00+00:00")


def sample_preserved_source_file() -> PreservedSourceFile:
    """Return a metadata-only preserved source file record."""
    digest = stable_json_hash({"source": str(FIXTURE_SOURCE_PATH), "paper_id": FIXTURE_PAPER_ID})
    return PreservedSourceFile(
        source_file_id=f"{FIXTURE_PAPER_ID}:source-file:normalized-md",
        paper_id=FIXTURE_PAPER_ID,
        source_role="normalized_markdown",
        original_path=str(FIXTURE_SOURCE_PATH),
        workspace_path="workspace/modular/fixture-paper-modular-0001.md",
        sha256=digest,
        byte_size=len(FIXTURE_MARKDOWN.encode("utf-8")),
        media_type="text/markdown",
        provenance={"fixture": "modular_contract_samples"},
        copied=False,
    )


def sample_asset_record() -> AssetRecord:
    """Return a representative metadata-only asset link."""
    return AssetRecord(
        asset_id=f"{FIXTURE_PAPER_ID}:asset:figure:0001",
        paper_id=FIXTURE_PAPER_ID,
        asset_type="figure",
        extraction_state="linked_not_extracted",
        source_file_id=sample_preserved_source_file().source_file_id,
        chunk_id=f"{FIXTURE_PAPER_ID}:chunk:results:0001",
        source_artifact=str(FIXTURE_SOURCE_PATH),
        source_span=AssetSourceSpan(
            coordinate_space="normalized_markdown_char",
            char_start=90,
            char_end=130,
        ),
        workspace_path=None,
        sha256=None,
        byte_size=None,
        media_type=None,
        provenance={"fixture": "modular_contract_samples"},
        warning_codes=("review_required",),
    )


def sample_redacted_article_structure() -> dict[str, Any]:
    """Return the canonical redacted structure dict shared across modules."""
    source_id = canonical_source_id(FIXTURE_PAPER_ID, "normalized-md")
    return {
        "schema_version": "m023-redacted-article-structure.v1",
        "paper_id": FIXTURE_PAPER_ID,
        "source_refs": [
            {
                "source_id": source_id,
                "paper_id": FIXTURE_PAPER_ID,
                "source_role": "normalized_markdown",
                "source_path": str(FIXTURE_SOURCE_PATH),
                "sha256": stable_json_hash({"markdown": FIXTURE_MARKDOWN}),
                "media_type": "text/markdown",
                "raw_text_embedded": False,
                "raw_binary_embedded": False,
            }
        ],
        "sections": [
            {
                "section_id": f"{FIXTURE_PAPER_ID}:section:root",
                "parent_section_id": None,
                "section_type": "root",
                "ordinal_path": [],
                "span_id": f"{FIXTURE_PAPER_ID}:span:root",
            },
            {
                "section_id": f"{FIXTURE_PAPER_ID}:section:methods",
                "parent_section_id": f"{FIXTURE_PAPER_ID}:section:root",
                "section_type": "methods",
                "ordinal_path": [1],
                "span_id": f"{FIXTURE_PAPER_ID}:span:methods",
            },
            {
                "section_id": f"{FIXTURE_PAPER_ID}:section:results",
                "parent_section_id": f"{FIXTURE_PAPER_ID}:section:root",
                "section_type": "results",
                "ordinal_path": [2],
                "span_id": f"{FIXTURE_PAPER_ID}:span:results",
            },
        ],
        "artifact_placeholders": [
            {
                "artifact_id": f"{FIXTURE_PAPER_ID}:artifact:figure:0001",
                "artifact_type": "figure",
                "section_id": f"{FIXTURE_PAPER_ID}:section:results",
                "span_id": f"{FIXTURE_PAPER_ID}:span:figure:0001",
                "caption_span_id": f"{FIXTURE_PAPER_ID}:span:caption:figure:0001",
                "candidate_link_targets": [f"{FIXTURE_PAPER_ID}:artifact:claim:0001"],
            }
        ],
        "safe_spans": [
            _safe_span(f"{FIXTURE_PAPER_ID}:span:root", source_id, 0, 15),
            _safe_span(f"{FIXTURE_PAPER_ID}:span:methods", source_id, 16, 80),
            _safe_span(f"{FIXTURE_PAPER_ID}:span:results", source_id, 81, 145),
            _safe_span(f"{FIXTURE_PAPER_ID}:span:caption:figure:0001", source_id, 105, 130),
            {
                "span_id": f"{FIXTURE_PAPER_ID}:span:figure:0001",
                "source_id": source_id,
                "coordinate_space": "page_bbox",
                "page_start": 1,
                "page_end": 1,
                "bbox": [0.1, 0.2, 0.8, 0.6],
                "span_hash": stable_json_hash({"span": "figure"}),
                "raw_text_embedded": False,
            },
        ],
        "safety_flags": {
            "raw_text_included": False,
            "raw_binary_included": False,
            "base64_included": False,
            "model_outputs_included": False,
            "embeddings_included": False,
            "vectors_included": False,
            "secrets_included": False,
            "optimizer_traces_included": False,
            "trusted_kg_import_allowed": False,
            "ladybugdb_written": False,
            "production_import_attempted": False,
        },
    }


def sample_article_artifact_manifest() -> dict[str, Any]:
    """Return a review-only article artifact manifest built from the shared structure."""
    return build_article_artifact_manifest_from_structure(
        sample_redacted_article_structure(),
        run_id="modular-fixture-generation",
    )


def sample_article_page_index_manifest() -> dict[str, Any]:
    """Return a metadata-only article PageIndex manifest built from the shared structure."""
    return build_article_page_index_from_structure(sample_redacted_article_structure())


def sample_import_boundary_rehearsal() -> ImportBoundaryRehearsal:
    """Return a representative rejected staging/import candidate."""
    candidate = ImportCandidate(
        candidate_id=f"{FIXTURE_PAPER_ID}:candidate:0001",
        method_id=f"{FIXTURE_PAPER_ID}:method:0001",
        package_id=f"benchmark-method:{FIXTURE_PAPER_ID}:method:0001",
        candidate_type="method_candidate",
        route="method_extraction",
        state="ok_for_retrieval_only",
        import_eligible=False,
        refusal_reasons=("review_not_completed",),
        remediation_hints=("complete_human_review_before_import",),
    )
    return ImportBoundaryRehearsal(
        rehearsal_id="modular-fixture-negative-import-boundary",
        source_benchmark_id="modular-fixture-generation",
        candidates=(candidate,),
        remediation_hints=("complete_human_review_before_import",),
    )


def canonical_contract_samples() -> dict[str, Any]:
    """Return JSON-native canonical samples for the refactored module contracts."""
    parsed = sample_parsed_article()
    page_index = sample_page_index_document()
    chunk_package = sample_structure_aware_package()
    preserved_source = sample_preserved_source_file()
    asset = sample_asset_record()
    rehearsal = sample_import_boundary_rehearsal()
    return {
        "schema_version": "modular-fixture-samples.v1",
        "paper_id": FIXTURE_PAPER_ID,
        "loader": adaptix_dump(sample_article_load_result()),
        "parser": adaptix_dump(parsed),
        "page_index": adaptix_dump(page_index),
        "chunking_contract": chunk_package.to_contract(),
        "asset_source": preserved_source.to_contract(),
        "asset_record": asset.to_contract(),
        "identity": {
            "canonical_source_id": canonical_source_id(FIXTURE_PAPER_ID, "normalized-md"),
            "sample_hash": stable_json_hash({"paper_id": FIXTURE_PAPER_ID, "fixture": "modular"}),
        },
        "article_artifacts": sample_article_artifact_manifest(),
        "article_page_index": sample_article_page_index_manifest(),
        "staging": rehearsal.to_contract(),
    }


def write_canonical_contract_samples(path: Path = MODULAR_FIXTURE_PATH) -> Path:
    """Persist canonical JSON fixture samples with stable formatting."""
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(canonical_contract_samples(), indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return path


def load_canonical_contract_samples(path: Path = MODULAR_FIXTURE_PATH) -> dict[str, Any]:
    """Load persisted canonical JSON fixture samples."""
    return json.loads(path.read_text(encoding="utf-8"))


def _safe_span(span_id: str, source_id: str, char_start: int, char_end: int) -> dict[str, Any]:
    return {
        "span_id": span_id,
        "source_id": source_id,
        "coordinate_space": "normalized_markdown_char",
        "char_start": char_start,
        "char_end": char_end,
        "span_hash": stable_json_hash({"span_id": span_id, "char_start": char_start, "char_end": char_end}),
        "raw_text_embedded": False,
    }


def _jsonable(value: Any) -> Any:
    if isinstance(value, Path):
        return str(value)
    if isinstance(value, tuple):
        return [_jsonable(item) for item in value]
    if isinstance(value, list):
        return [_jsonable(item) for item in value]
    if isinstance(value, dict):
        return {str(key): _jsonable(item) for key, item in value.items()}
    return value
