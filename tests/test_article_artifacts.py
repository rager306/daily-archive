from __future__ import annotations

import json
from copy import deepcopy
from pathlib import Path

from arxiv_archive.article_artifacts import (
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
    summary = ArticleArtifactRunSummary(run_id="test-run", manifests=(manifest,)).to_redacted_dict()

    assert summary["schema_version"] == ARTICLE_ARTIFACT_RUN_SCHEMA_VERSION
    assert summary["paper_count"] == 1
    assert summary["artifact_count"] == 1
    assert summary["diagnostic_count"] == 1
    assert summary["artifact_counts_by_type"] == {"figure": 1}
    assert summary["review_state_counts"] == {"review_required": 1}
    assert summary["candidate_link_type_counts"] == {"supports": 1}
    assert summary["promoted_to_fact_count"] == 0
    assert summary["import_eligible_count"] == 0
    assert summary["safety_flags"]["raw_binary_included"] is False


def test_to_json_is_sorted_and_contract_contains_no_forbidden_payload_keys() -> None:
    manifest = _manifest()
    payload = to_json(manifest)

    assert payload.endswith("\n")
    assert "raw paper text" not in payload
    assert validate_article_artifact_manifest(json.loads(payload)) == []
