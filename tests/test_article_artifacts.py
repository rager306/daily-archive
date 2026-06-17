from __future__ import annotations

import json
from copy import deepcopy
from pathlib import Path

from research_graph.corpus.ingestion.loader import load_article_source
from research_graph.papers.artifacts.models import (
    ARTICLE_ARTIFACT_DIAGNOSTICS_SCHEMA_VERSION,
    ARTICLE_ARTIFACT_RUN_SCHEMA_VERSION,
    ARTICLE_ARTIFACT_SCHEMA_VERSION,
    ArticleArtifactDiagnostic,
    ArticleArtifactManifest,
    ArticleArtifactRecord,
    ArticleArtifactRunSummary,
    CandidateLink,
    SectionLineage,
    SourceReference,
    SourceSpan,
    build_article_artifact_manifest_from_structure,
    build_article_artifact_run_diagnostics_artifact,
    default_safety_flags,
    to_json,
    validate_article_artifact_manifest,
)

VALID_SHA = "a" * 64
FIXTURES_DIR = Path(__file__).parent / "fixtures" / "article_artifacts"


def _load_fixture(name: str) -> dict:
    return json.loads((FIXTURES_DIR / name).read_text(encoding="utf-8"))


def _source_ref() -> SourceReference:
    return SourceReference(
        source_id="source-p1-md",
        paper_id="p1",
        source_role="normalized_markdown",
        source_path="papers/p1/source/normalized.md",
        sha256=VALID_SHA,
        media_type="text/markdown",
    )


def _span() -> SourceSpan:
    return SourceSpan(
        span_id="span-1",
        source_id="source-p1-md",
        coordinate_space="normalized_markdown_char",
        char_start=10,
        char_end=42,
        span_hash="b" * 64,
    )


def _artifact() -> ArticleArtifactRecord:
    span = _span()
    return ArticleArtifactRecord(
        artifact_id="p1:artifact:figure:0001",
        paper_id="p1",
        artifact_type="figure",
        review_state="review_required",
        source_refs=(_source_ref(),),
        source_spans=(span,),
        section_lineage=SectionLineage(
            section_id="p1:section:2",
            parent_section_id="p1:section:root",
            section_type="results",
            ordinal_path=(2,),
            source_span=span,
        ),
        candidate_links=(
            CandidateLink(
                link_id="p1:link:0001",
                source_artifact_id="p1:artifact:figure:0001",
                target_ref="p1:artifact:claim:0002",
                link_type="supports",
                review_state="review_required",
                source_spans=(span,),
                diagnostic_codes=("needs_semantic_review",),
            ),
        ),
        confidence_label="medium",
        diagnostic_codes=("review_required",),
        metadata={"detector_version": "test"},
    )


def _manifest() -> dict:
    return ArticleArtifactManifest(
        paper_id="p1",
        run_id="test-run",
        source_refs=(_source_ref(),),
        artifacts=(_artifact(),),
        diagnostics=(
            ArticleArtifactDiagnostic(
                code="review_required",
                json_path="/artifacts[0]",
                object_id="p1:artifact:figure:0001",
            ),
        ),
    ).to_redacted_dict()


def test_redacted_article_structure_fixture_validates_expected_manifest() -> None:
    structure = _load_fixture("basic_article_structure.json")
    manifest = _load_fixture("basic_expected_manifest.json")

    assert structure["schema_version"] == "m023-redacted-article-structure.v1"
    assert structure["paper_id"] == manifest["paper_id"]
    assert {source["source_id"] for source in structure["source_refs"]} == {
        source["source_id"] for source in manifest["source_refs"]
    }
    assert {placeholder["artifact_id"] for placeholder in structure["artifact_placeholders"]}.issubset(
        {artifact["artifact_id"] for artifact in manifest["artifacts"]}
    )
    assert {span["span_id"] for span in structure["safe_spans"]}.issuperset(
        {
            "fixture-paper-0001:span:caption-figure-0001",
            "fixture-paper-0001:span:citation-0001",
            "fixture-paper-0001:span:equation-0001",
        }
    )
    assert validate_article_artifact_manifest(manifest) == []
    assert manifest["summary"]["artifact_counts_by_type"] == {
        "equation": 1,
        "figure": 1,
        "reference": 1,
        "section": 1,
    }
    assert manifest["summary"]["candidate_link_type_counts"] == {
        "cites": 1,
        "contains": 2,
        "located_in": 1,
        "supports": 1,
    }
    assert manifest["import_eligible_count"] == 0
    assert manifest["promoted_to_fact_count"] == 0

    serialized = json.dumps({"structure": structure, "manifest": manifest})
    forbidden_fragments = (
        "raw paper text",
        "raw_model_output",
        "raw_minimax_response",
        "\"text\":",
        "\"caption_text\":",
        "\"embedding\":",
        "\"vector\":",
        "\"secret\":",
        "\"optimizer_trace\":",
        "\"source_of_truth\":",
        "trusted_kg_import_allowed\": true",
        "ladybugdb_written\": true",
        "production_import_attempted\": true",
        "model_outputs_included\": true",
    )
    for fragment in forbidden_fragments:
        assert fragment not in serialized




def test_deterministic_fixture_detector_generates_candidates_from_structure() -> None:
    structure = _load_fixture("basic_article_structure.json")
    manifest = build_article_artifact_manifest_from_structure(structure)

    assert validate_article_artifact_manifest(manifest) == []
    assert manifest["paper_id"] == "fixture-paper-0001"
    assert manifest["summary"]["artifact_counts_by_type"] == {
        "equation": 1,
        "figure": 1,
        "reference": 1,
        "section": 2,
    }
    assert manifest["summary"]["candidate_link_type_counts"] == {
        "cites": 1,
        "contains": 3,
        "located_in": 1,
        "supports": 1,
    }
    assert manifest["summary"]["missing_span_count"] == 0
    assert manifest["summary"]["diagnostic_summary"]["review_state_counts"] == {"review_required": 5}
    artifact_ids = {artifact["artifact_id"] for artifact in manifest["artifacts"]}
    assert "fixture-paper-0001:artifact:section:methods" in artifact_ids
    assert "fixture-paper-0001:artifact:section:results" in artifact_ids

    serialized = json.dumps(manifest)
    for forbidden_fragment in (
        '"text":',
        '"caption_text":',
        '"raw_model_output":',
        '"embedding":',
        '"vector":',
        '"secret":',
        '"source_of_truth":',
    ):
        assert forbidden_fragment not in serialized


def test_deterministic_fixture_detector_uses_explicit_structured_markers_only() -> None:
    structure = _load_fixture("basic_article_structure.json")
    structure["structured_markers"] = [
        {
            "artifact_id": "fixture-paper-0001:artifact:dataset:0001",
            "artifact_type": "dataset",
            "section_id": "fixture-paper-0001:section:methods",
            "span_id": "fixture-paper-0001:span:section-methods",
        },
        {
            "artifact_id": "fixture-paper-0001:artifact:method:0001",
            "artifact_type": "method",
            "section_id": "fixture-paper-0001:section:methods",
            "span_id": "fixture-paper-0001:span:section-methods",
        },
        {
            "artifact_id": "fixture-paper-0001:artifact:metric:0001",
            "artifact_type": "metric",
            "section_id": "fixture-paper-0001:section:results",
            "span_id": "fixture-paper-0001:span:section-results",
        },
        {
            "artifact_id": "fixture-paper-0001:artifact:experiment:0001",
            "artifact_type": "experiment",
            "section_id": "fixture-paper-0001:section:results",
            "span_id": "fixture-paper-0001:span:section-results",
        },
    ]

    manifest = build_article_artifact_manifest_from_structure(structure)

    assert validate_article_artifact_manifest(manifest) == []
    assert manifest["summary"]["artifact_counts_by_type"] == {
        "dataset": 1,
        "equation": 1,
        "experiment": 1,
        "figure": 1,
        "method": 1,
        "metric": 1,
        "reference": 1,
        "section": 2,
    }
    assert manifest["summary"]["candidate_link_type_counts"]["contains"] == 7


def test_deterministic_fixture_detector_reports_missing_spans_without_raw_payloads() -> None:
    structure = _load_fixture("basic_article_structure.json")
    structure["artifact_placeholders"][0]["span_id"] = "fixture-paper-0001:span:missing"

    manifest = build_article_artifact_manifest_from_structure(structure)

    assert validate_article_artifact_manifest(manifest) == []
    assert manifest["summary"]["missing_span_count"] == 1
    assert manifest["summary"]["diagnostic_summary"]["diagnostic_counts_by_code"] == {"missing_span": 1}
    assert manifest["diagnostics"][0]["code"] == "missing_span"
    assert "raw paper text" not in json.dumps(manifest["diagnostics"])


def test_deterministic_fixture_detector_rejects_raw_payload_markers() -> None:
    structure = _load_fixture("basic_article_structure.json")
    structure["artifact_placeholders"][0]["caption_text"] = "forbidden raw caption"

    try:
        build_article_artifact_manifest_from_structure(structure)
    except ValueError as exc:
        assert "forbidden raw payload keys" in str(exc)
    else:  # pragma: no cover - defensive assertion branch
        raise AssertionError("raw payload marker should be rejected")


def test_source_reference_can_be_built_from_loader_result(tmp_path: Path) -> None:
    source_path = tmp_path / "paper.md"
    source_path.write_text("# Abstract\n\nExplicit loader provenance.\n", encoding="utf-8")
    result = load_article_source(source_path, source_type="markdown", paper_id="p-loader")

    source = SourceReference.from_loader_result(result, source_role="normalized_markdown")
    payload = source.to_redacted_dict()

    assert payload["source_id"] == result.source_id
    assert payload["paper_id"] == "p-loader"
    assert payload["source_role"] == "normalized_markdown"
    assert payload["source_path"] == str(source_path)
    assert payload["sha256"] == result.sha256
    assert payload["media_type"] == "text/markdown"
    assert payload["conversion_status"] == "loaded"
    assert payload["raw_text_embedded"] is False
    assert payload["raw_binary_embedded"] is False

def test_artifact_manifest_serializes_redacted_contract() -> None:
    manifest = _manifest()

    assert manifest["schema_version"] == ARTICLE_ARTIFACT_SCHEMA_VERSION
    assert manifest["paper_id"] == "p1"
    assert manifest["summary"]["artifact_count"] == 1
    assert manifest["summary"]["candidate_link_count"] == 1
    assert manifest["summary"]["artifact_counts_by_type"] == {"figure": 1}
    assert manifest["summary"]["candidate_link_type_counts"] == {"supports": 1}
    assert manifest["summary"]["promoted_to_fact_count"] == 0
    assert manifest["import_eligible_count"] == 0
    assert manifest["production_import_attempted"] is False
    assert manifest["ladybugdb_written"] is False
    assert manifest["safety_flags"] == default_safety_flags()
    assert validate_article_artifact_manifest(manifest) == []


def test_artifact_record_blocks_import_and_kg_promotion() -> None:
    record = _artifact().to_redacted_dict()

    assert record["promoted_to_fact"] is False
    assert record["import_eligible"] is False
    assert "trusted_kg_import" not in record["allowed_uses"]
    assert "trusted_kg_import" in record["excluded_uses"]
    assert "production_ladybugdb_write" in record["excluded_uses"]
    assert record["safety_flags"]["raw_text_included"] is False
    assert record["safety_flags"]["model_outputs_included"] is False
    assert record["source_refs"][0]["raw_text_embedded"] is False
    assert record["source_spans"][0]["raw_text_embedded"] is False


def test_section_lineage_uses_ids_and_ordinals_not_titles_or_text() -> None:
    lineage = _artifact().to_redacted_dict()["section_lineage"]

    assert lineage == {
        "section_id": "p1:section:2",
        "parent_section_id": "p1:section:root",
        "section_type": "results",
        "ordinal_path": [2],
        "source_span": _span().to_redacted_dict(),
    }
    serialized = json.dumps(lineage)
    assert "section_title" not in serialized
    assert "section_text" not in serialized
    assert "raw_text\":" not in serialized
    assert lineage["source_span"]["raw_text_embedded"] is False


def test_candidate_link_uses_explicit_review_vocabulary() -> None:
    link = _artifact().to_redacted_dict()["candidate_links"][0]

    assert link["link_type"] == "supports"
    assert link["review_state"] == "review_required"
    assert link["promoted_to_fact"] is False
    assert link["import_eligible"] is False
    assert link["source_spans"][0]["coordinate_space"] == "normalized_markdown_char"


def test_validation_reports_stable_codes_and_json_paths_without_leaked_values() -> None:
    leaked = "do not echo this raw paper text"
    manifest = _manifest()
    manifest["artifacts"][0]["metadata"]["raw_text"] = leaked
    manifest["artifacts"][0]["safety_flags"]["raw_text_included"] = True
    manifest["artifacts"][0]["candidate_links"][0]["link_type"] = "bad-link"
    manifest["artifacts"][0]["source_spans"][0]["char_end"] = 1

    diagnostics = validate_article_artifact_manifest(manifest)
    codes = {diagnostic["code"] for diagnostic in diagnostics}
    paths = {diagnostic["json_path"] for diagnostic in diagnostics}
    serialized = json.dumps(diagnostics)

    assert "forbidden_payload_key" in codes
    assert "safety_flag_true:raw_text_included" in codes
    assert "invalid_candidate_link_type" in codes
    assert "invalid_source_span_coordinates" in codes
    assert "/artifacts[0]/metadata/raw_text" in paths
    assert "/artifacts[0]/safety_flags/raw_text_included" in paths
    assert "/artifacts[0]/candidate_links[0]/link_type" in paths
    assert leaked not in serialized


def test_validation_rejects_missing_ids_and_invalid_vocabularies() -> None:
    manifest = _manifest()
    del manifest["artifacts"][0]["artifact_id"]
    manifest["artifacts"][0]["artifact_type"] = "paragraph"
    manifest["artifacts"][0]["review_state"] = "approved"
    manifest["artifacts"][0]["candidate_links"][0]["link_id"] = ""
    manifest["artifacts"][0]["candidate_links"][0]["review_state"] = "needs-human"

    diagnostics = validate_article_artifact_manifest(manifest)
    codes = {diagnostic["code"] for diagnostic in diagnostics}
    paths = {diagnostic["json_path"] for diagnostic in diagnostics}

    assert "missing_artifact_id" in codes
    assert "invalid_artifact_type" in codes
    assert "invalid_review_state" in codes
    assert "empty_link_id" in codes
    assert "invalid_candidate_link_review_state" in codes
    assert "/artifacts[0]/artifact_id" in paths
    assert "/artifacts[0]/artifact_type" in paths
    assert "/artifacts[0]/review_state" in paths
    assert "/artifacts[0]/candidate_links[0]/link_id" in paths
    assert "/artifacts[0]/candidate_links[0]/review_state" in paths


def test_validation_rejects_minimax_source_of_truth_and_broken_source_refs() -> None:
    manifest = _manifest()
    manifest["artifacts"][0]["metadata"]["source_of_truth"] = "MiniMax extraction"
    manifest["artifacts"][0]["source_spans"][0]["source_id"] = "missing-source"
    manifest["artifacts"][0]["section_lineage"]["source_span"]["coordinate_space"] = "normalized_markdown_char"
    manifest["artifacts"][0]["section_lineage"]["source_span"]["char_start"] = None
    manifest["artifacts"][0]["section_lineage"]["source_span"]["char_end"] = None

    diagnostics = validate_article_artifact_manifest(manifest)
    codes = {diagnostic["code"] for diagnostic in diagnostics}
    paths = {diagnostic["json_path"] for diagnostic in diagnostics}
    serialized = json.dumps(diagnostics)

    assert "source_of_truth_claim" in codes
    assert "minimax_source_of_truth" in codes
    assert "unknown_source_id" in codes
    assert "invalid_source_span_coordinates" in codes
    assert "/artifacts[0]/metadata/source_of_truth" in paths
    assert "/artifacts[0]/source_spans[0]/source_id" in paths
    assert "/artifacts[0]/section_lineage/source_span" in paths
    assert "MiniMax extraction" not in serialized


def test_validation_rejects_duplicate_ids() -> None:
    manifest = _manifest()
    duplicate_artifact = deepcopy(manifest["artifacts"][0])
    duplicate_source = deepcopy(manifest["source_refs"][0])
    duplicate_span = deepcopy(manifest["artifacts"][0]["source_spans"][0])
    duplicate_link = deepcopy(manifest["artifacts"][0]["candidate_links"][0])
    manifest["artifacts"].append(duplicate_artifact)
    manifest["source_refs"].append(duplicate_source)
    manifest["artifacts"][0]["source_spans"].append(duplicate_span)
    manifest["artifacts"][0]["candidate_links"].append(duplicate_link)
    manifest["summary"]["artifact_count"] = 2
    manifest["summary"]["candidate_link_count"] = 3

    diagnostics = validate_article_artifact_manifest(manifest)
    codes = {diagnostic["code"] for diagnostic in diagnostics}
    paths = {diagnostic["json_path"] for diagnostic in diagnostics}

    assert "duplicate_artifact_id" in codes
    assert "duplicate_source_id" in codes
    assert "duplicate_source_span_id" in codes
    assert "duplicate_candidate_link_id" in codes
    assert "/artifacts[1]/artifact_id" in paths
    assert "/source_refs[1]/source_id" in paths
    assert "/artifacts[0]/source_spans[1]/span_id" in paths
    assert "/artifacts[0]/candidate_links[1]/link_id" in paths


def test_validation_rejects_import_eligible_and_promoted_records() -> None:
    manifest = _manifest()
    manifest["artifacts"][0]["promoted_to_fact"] = True
    manifest["artifacts"][0]["import_eligible"] = True
    manifest["artifacts"][0]["allowed_uses"].append("trusted_kg_import")
    manifest["artifacts"][0]["excluded_uses"].remove("trusted_kg_import")
    manifest["summary"]["promoted_to_fact_count"] = 1

    diagnostics = validate_article_artifact_manifest(manifest)
    codes = [diagnostic["code"] for diagnostic in diagnostics]

    assert "artifact_promoted_to_fact" in codes
    assert "artifact_import_eligible" in codes
    assert "trusted_import_allowed" in codes
    assert "missing_excluded_use" in codes
    assert "summary_promoted_to_fact_count_nonzero" in codes


def test_run_summary_merges_manifest_counts_without_payloads() -> None:
    manifest = _manifest()
    summary = ArticleArtifactRunSummary(
        run_id="test-run",
        manifests=(manifest,),
        input_hashes={"input_structure_sha256": VALID_SHA},
        output_paths={"manifest": "out/p1-article-artifacts.json"},
    ).to_redacted_dict()

    assert summary["schema_version"] == ARTICLE_ARTIFACT_RUN_SCHEMA_VERSION
    assert summary["diagnostics_schema_version"] == ARTICLE_ARTIFACT_DIAGNOSTICS_SCHEMA_VERSION
    assert summary["manifest_schema_version"] == ARTICLE_ARTIFACT_SCHEMA_VERSION
    assert summary["paper_count"] == 1
    assert summary["paper_ids"] == ["p1"]
    assert summary["artifact_count"] == 1
    assert summary["diagnostic_count"] == 1
    assert summary["diagnostic_codes"] == ["review_required"]
    assert summary["artifact_counts_by_type"] == {"figure": 1}
    assert summary["review_state_counts"] == {"review_required": 1}
    assert summary["candidate_link_type_counts"] == {"supports": 1}
    assert summary["promoted_to_fact_count"] == 0
    assert summary["import_eligible_count"] == 0
    assert summary["production_import_attempted"] is False
    assert summary["ladybugdb_written"] is False
    assert summary["trusted_kg_import_allowed"] is False
    assert summary["input_hashes"] == {"input_structure_sha256": VALID_SHA}
    assert summary["output_paths"] == {"manifest": "out/p1-article-artifacts.json"}
    assert summary["safety_flags"]["raw_binary_included"] is False


def test_run_diagnostics_artifact_carries_stable_codes_and_no_import_flags() -> None:
    manifest = _manifest()
    diagnostics = build_article_artifact_run_diagnostics_artifact(
        run_id="test-run",
        manifests=(manifest,),
        input_hashes={"input_structure_sha256": VALID_SHA},
        output_paths={"diagnostics": "out/article-artifacts-diagnostics.json"},
    )

    assert diagnostics["schema_version"] == ARTICLE_ARTIFACT_DIAGNOSTICS_SCHEMA_VERSION
    assert diagnostics["run_schema_version"] == ARTICLE_ARTIFACT_RUN_SCHEMA_VERSION
    assert diagnostics["manifest_schema_version"] == ARTICLE_ARTIFACT_SCHEMA_VERSION
    assert diagnostics["paper_ids"] == ["p1"]
    assert diagnostics["diagnostic_count"] == 1
    assert diagnostics["diagnostic_codes"] == ["review_required"]
    assert diagnostics["diagnostic_counts_by_code"] == {"review_required": 1}
    assert diagnostics["manifest_diagnostic_summaries"]["p1"]["diagnostic_counts_by_code"] == {"review_required": 1}
    assert diagnostics["production_import_attempted"] is False
    assert diagnostics["ladybugdb_written"] is False
    assert diagnostics["trusted_kg_import_allowed"] is False
    assert diagnostics["promoted_to_fact_count"] == 0
    assert diagnostics["import_eligible_count"] == 0
    assert diagnostics["safety_flags"] == default_safety_flags()

    serialized = json.dumps(diagnostics)
    for forbidden_fragment in (
        '"text":',
        '"caption_text":',
        '"raw_model_output":',
        '"embedding":',
        '"vector":',
        '"secret":',
        '"source_of_truth":',
        '"trusted_kg_import_allowed": true',
        '"ladybugdb_written": true',
        '"production_import_attempted": true',
    ):
        assert forbidden_fragment not in serialized


def test_to_json_is_sorted_and_contract_contains_no_forbidden_payload_keys() -> None:
    manifest = _manifest()
    payload = to_json(manifest)

    assert payload.endswith("\n")
    assert "raw paper text" not in payload
    assert validate_article_artifact_manifest(json.loads(payload)) == []
