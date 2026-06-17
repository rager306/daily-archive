"""Property hardening for the M024 article evidence bridge contract."""

from __future__ import annotations

import json
from copy import deepcopy
from pathlib import Path
from typing import Any

from adaptix import Retort
from hypothesis import given, settings
from hypothesis import strategies as st

from research_graph.papers.evidence import (
    ALLOWED_LOAD_OUTCOMES,
    ALLOWED_SUBTREE_STATUSES,
    ARTICLE_EVIDENCE_BUNDLE_SCHEMA_VERSION,
    ARTICLE_EVIDENCE_DIAGNOSTICS_SCHEMA_VERSION,
    ARTICLE_EVIDENCE_RUN_SCHEMA_VERSION,
    ArticleEvidenceBundle,
    ArticleEvidenceDiagnostic,
    ArticleEvidenceRunSummary,
    ArticleEvidenceSourceReference,
    attach_retrieval_table_benchmark_summary,
    build_article_evidence_run_summary,
    default_safety_flags,
    summarize_source_refs,
    to_json,
    to_redacted_dict,
    validate_article_evidence_bundle,
)

BRIDGE_RETORT = Retort()

HEX64 = st.text(alphabet="0123456789abcdef", min_size=64, max_size=64)
SAFE_TEXT = st.text(
    alphabet=st.characters(blacklist_categories=("Cs",), blacklist_characters="\x00"),
    min_size=1,
    max_size=40,
).filter(lambda value: value.strip() != "")
PAPER_IDS = st.from_regex(r"\d{4}\.\d{4,5}", fullmatch=True)
SOURCE_IDS = st.from_regex(r"article-source:[a-f0-9]{12}", fullmatch=True)
RUN_IDS = st.from_regex(r"m024-property-run-[a-f0-9]{8}", fullmatch=True)
FORBIDDEN_PAYLOAD_KEYS = st.sampled_from(
    [
        "text",
        "raw_text",
        "chunk_text",
        "paper_text",
        "payload",
        "embedding",
        "embeddings",
        "vector",
        "vectors",
        "api_key",
        "credentials",
        "optimizer_trace",
    ]
)
REQUIRED_SUBTREES = ("raw", "normalized", "page_index", "assets", "links_dedup", "retrieval", "staging", "metrics")
RETRIEVAL_TABLES_FIXTURES_DIR = Path(__file__).parent / "fixtures" / "article_retrieval_tables"


def source_ref_strategy() -> st.SearchStrategy[ArticleEvidenceSourceReference]:
    """Generate metadata-only source references with outcome-compatible fields."""

    def build_source(values: tuple[str, str, str, str, str | None, int, int, int]) -> ArticleEvidenceSourceReference:
        paper_id, source_id, source_type, load_outcome, sha256, byte_size, warning_count, duration_ms = values
        is_pdf = source_type == "pdf"
        failure_reason = "no_substantive_body" if load_outcome == "failed" else None
        checksum = sha256 if load_outcome != "failed" else None
        return ArticleEvidenceSourceReference(
            source_id=source_id,
            paper_id=paper_id,
            source_path=f"fixtures/{source_id.removeprefix('article-source:')}.{source_type if not is_pdf else 'pdf'}",
            source_type=source_type,
            media_type="application/pdf" if is_pdf else "text/markdown",
            sha256=checksum,
            byte_size=byte_size,
            parser_name="pdf_metadata_probe" if is_pdf else "markdown_loader",
            loader_name="local_article_loader",
            load_outcome=load_outcome,
            failure_reason=failure_reason,
            warning_count=warning_count,
            duration_ms=duration_ms,
        )

    return st.builds(
        build_source,
        st.tuples(
            PAPER_IDS,
            SOURCE_IDS,
            st.sampled_from(["markdown", "pdf"]),
            st.sampled_from(sorted(ALLOWED_LOAD_OUTCOMES)),
            HEX64,
            st.integers(min_value=0, max_value=10_000_000),
            st.integers(min_value=0, max_value=20),
            st.integers(min_value=0, max_value=120_000),
        ),
    )


def unique_source_refs_strategy(
    *, min_size: int = 0, max_size: int = 5
) -> st.SearchStrategy[tuple[ArticleEvidenceSourceReference, ...]]:
    return st.lists(
        source_ref_strategy(),
        min_size=min_size,
        max_size=max_size,
        unique_by=lambda source: source.source_id,
    ).map(lambda values: tuple(sorted(values, key=lambda source: source.source_id)))


def bundle_strategy(*, min_sources: int = 0) -> st.SearchStrategy[ArticleEvidenceBundle]:
    def build_bundle(
        paper_id: str, run_id: str, source_refs: tuple[ArticleEvidenceSourceReference, ...]
    ) -> ArticleEvidenceBundle:
        normalized_sources = tuple(
            ArticleEvidenceSourceReference(
                source_id=source.source_id,
                paper_id=paper_id,
                source_path=source.source_path,
                source_type=source.source_type,
                media_type=source.media_type,
                sha256=source.sha256,
                byte_size=source.byte_size,
                parser_name=source.parser_name,
                loader_name=source.loader_name,
                load_outcome=source.load_outcome,
                failure_reason=source.failure_reason,
                warning_count=source.warning_count,
                duration_ms=source.duration_ms,
            )
            for source in source_refs
        )
        source_dicts = [source.to_redacted_dict() for source in normalized_sources]
        digest_seed = "-".join(source.source_id for source in normalized_sources) or "empty"
        return ArticleEvidenceBundle(
            paper_id=paper_id,
            run_id=run_id,
            bundle_id=f"article-evidence-bundle:property-{abs(hash((paper_id, run_id, digest_seed))) % 10**12:012d}",
            source_refs=normalized_sources,
            bundle_root=None,
            diagnostics=(),
            subtrees={},
            summary=summarize_source_refs(source_dicts),
        )

    return st.builds(build_bundle, PAPER_IDS, RUN_IDS, unique_source_refs_strategy(min_size=min_sources))


def assert_redacted_bridge_payload(payload: dict[str, Any]) -> None:
    serialized = json.dumps(payload, sort_keys=True)
    assert "Graph-Guided Retrieval for Scientific Agents" not in serialized
    assert "OPENAI_API_KEY" not in serialized
    assert "sk-test-secret" not in serialized
    assert "embedding=[" not in serialized
    assert "vector=[" not in serialized


# --- Property: Adaptix and deterministic serialization roundtrips ---


@settings(max_examples=80)
@given(source=source_ref_strategy())
def test_source_reference_adaptix_roundtrip_preserves_metadata_only_contract(
    source: ArticleEvidenceSourceReference,
) -> None:
    dumped = BRIDGE_RETORT.dump(source)
    restored = BRIDGE_RETORT.load(dumped, ArticleEvidenceSourceReference)

    assert restored == source
    assert restored.to_redacted_dict() == source.to_redacted_dict()
    assert validate_article_evidence_bundle(
        ArticleEvidenceBundle(
            paper_id=source.paper_id,
            run_id="m024-property-source",
            bundle_id="article-evidence-bundle:property-source",
            source_refs=(restored,),
            summary=summarize_source_refs([restored.to_redacted_dict()]),
        )
    ) == []


@settings(max_examples=80)
@given(diagnostic_code=SAFE_TEXT, json_path=SAFE_TEXT.map(lambda value: f"/{value.strip('/')}"))
def test_diagnostic_adaptix_roundtrip_preserves_path_and_blocks_import(
    diagnostic_code: str, json_path: str
) -> None:
    original = ArticleEvidenceDiagnostic(
        code=diagnostic_code,
        json_path=json_path,
        severity="repair_required",
        object_id="article-source:diagnostic",
        blocks_import=True,
    )

    dumped = BRIDGE_RETORT.dump(original)
    restored = BRIDGE_RETORT.load(dumped, ArticleEvidenceDiagnostic)

    assert restored == original
    assert restored.to_redacted_dict() == original.to_redacted_dict()
    assert restored.to_redacted_dict()["blocks_import"] is True


@settings(max_examples=80)
@given(bundle=bundle_strategy())
def test_bundle_to_json_and_adaptix_roundtrips_are_deterministic(bundle: ArticleEvidenceBundle) -> None:
    payload = bundle.to_redacted_dict()
    json_roundtrip = json.loads(to_json(bundle))
    adaptix_roundtrip = BRIDGE_RETORT.load(BRIDGE_RETORT.dump(bundle), ArticleEvidenceBundle)

    assert payload["schema_version"] == ARTICLE_EVIDENCE_BUNDLE_SCHEMA_VERSION
    assert payload == json_roundtrip
    assert adaptix_roundtrip.to_redacted_dict() == payload
    assert validate_article_evidence_bundle(json_roundtrip) == []
    assert_redacted_bridge_payload(json_roundtrip)


@settings(max_examples=80)
@given(bundle=bundle_strategy())
def test_placeholder_subtrees_and_metrics_survive_json_roundtrip(bundle: ArticleEvidenceBundle) -> None:
    payload = bundle.to_redacted_dict()
    restored = json.loads(json.dumps(payload, sort_keys=True))

    assert set(restored["subtrees"]) == set(REQUIRED_SUBTREES)
    assert restored["summary"] == payload["summary"]
    assert restored["subtrees"] == payload["subtrees"]
    assert restored["summary"]["import_eligible_count"] == 0
    assert restored["summary"]["promoted_to_fact_count"] == 0
    assert restored["summary"]["safety_flags"] == default_safety_flags()
    assert validate_article_evidence_bundle(restored) == []


@settings(max_examples=40, deadline=None)
@given(bundle=bundle_strategy(min_sources=1), include_manifest_provenance=st.booleans())
def test_retrieval_table_attachment_preserves_required_subtrees_and_aggregate_only_contract(
    bundle: ArticleEvidenceBundle, include_manifest_provenance: bool
) -> None:
    from research_graph.papers.indexing.retrieval_tables import build_article_retrieval_table_manifest

    fixture = json.loads((RETRIEVAL_TABLES_FIXTURES_DIR / "minimal_manifest.json").read_text(encoding="utf-8"))
    source_refs = [bundle.source_refs[0].to_redacted_dict()]
    source_id = source_refs[0]["source_id"]
    fixture["paper_id"] = bundle.paper_id
    fixture["run_id"] = bundle.run_id
    fixture["source_refs"] = source_refs
    for unit in fixture["retrieval_units"]:
        unit["source_ref_ids"] = [source_id]
    for candidate in fixture["table_candidates"]:
        candidate["source_ref_ids"] = [source_id]
    manifest = build_article_retrieval_table_manifest(
        paper_id=fixture["paper_id"],
        run_id=fixture["run_id"],
        source_refs=fixture["source_refs"],
        page_index_refs=fixture["page_index_refs"],
        asset_refs=fixture["asset_refs"],
        links_dedup_refs=fixture["links_dedup_refs"],
        retrieval_units=fixture["retrieval_units"],
        table_candidates=fixture["table_candidates"],
        manifest_path=fixture["manifest_path"],
    )

    if not include_manifest_provenance:
        manifest.pop("manifest_path", None)
        manifest.pop("manifest_sha256", None)

    attached = attach_retrieval_table_benchmark_summary(
        bundle,
        manifest,
        manifest_path=manifest.get("manifest_path") if include_manifest_provenance else None,
        manifest_sha256=manifest.get("manifest_sha256") if include_manifest_provenance else None,
    )
    retrieval = attached["subtrees"]["retrieval"]
    metrics = attached["subtrees"]["metrics"]["retrieval_table_benchmark"]

    assert set(attached["subtrees"]) == set(REQUIRED_SUBTREES)
    assert retrieval["record_count"] == 4
    assert retrieval["retrieval_unit_count"] == 3
    assert retrieval["table_candidate_count"] == 1
    assert retrieval["ranking_tie_count"] == 1
    assert retrieval["import_eligible_count"] == 0
    assert retrieval["promoted_to_fact_count"] == 0
    assert retrieval["production_import_attempted"] is False
    assert retrieval["ladybugdb_written"] is False
    assert retrieval["trusted_kg_import_allowed"] is False
    assert metrics["record_count"] == retrieval["record_count"]
    assert metrics["import_eligible_count"] == 0
    assert metrics["production_import_attempted"] is False
    assert "retrieval_units" not in retrieval
    assert "table_candidates" not in retrieval
    assert attached["import_eligible_count"] == 0
    assert attached["promoted_to_fact_count"] == 0
    if include_manifest_provenance and retrieval["diagnostic_count"] == 0:
        assert retrieval["status"] == "review_only"
        assert metrics["status"] == "review_only"
        assert validate_article_evidence_bundle(attached) == []
    else:
        assert retrieval["status"] == "blocked"
        assert metrics["status"] == "blocked"
        if not include_manifest_provenance:
            assert retrieval["diagnostic_counts_by_code"]["retrieval_table_missing_manifest_path"] == 1
            assert retrieval["diagnostic_counts_by_code"]["retrieval_table_missing_manifest_sha256"] == 1
        assert validate_article_evidence_bundle(attached) == []
    assert_redacted_bridge_payload(attached)


@settings(max_examples=50)
@given(bundles=st.lists(bundle_strategy(), min_size=0, max_size=4), run_id=RUN_IDS)
def test_run_summary_to_redacted_dict_and_json_roundtrips(
    bundles: list[ArticleEvidenceBundle], run_id: str
) -> None:
    run_summary = build_article_evidence_run_summary(
        run_id=run_id,
        bundles=bundles,
        output_paths={"bundle_dir": "redacted-bundles"},
        input_source_ids=[source.source_id for bundle in bundles for source in bundle.source_refs],
        input_hashes=["source-load-event:" + "a" * 64],
    )
    dataclass_summary = ArticleEvidenceRunSummary(
        run_id=run_id,
        bundles=tuple(to_redacted_dict(bundle) for bundle in bundles),
        output_paths={"bundle_dir": "redacted-bundles"},
        input_source_ids=tuple(sorted({source.source_id for bundle in bundles for source in bundle.source_refs})),
        input_hashes=("source-load-event:" + "a" * 64,),
    )

    restored_dataclass = BRIDGE_RETORT.load(BRIDGE_RETORT.dump(dataclass_summary), ArticleEvidenceRunSummary)
    json_roundtrip = json.loads(json.dumps(run_summary, sort_keys=True))

    assert run_summary["schema_version"] == ARTICLE_EVIDENCE_RUN_SCHEMA_VERSION
    assert run_summary["bundle_schema_version"] == ARTICLE_EVIDENCE_BUNDLE_SCHEMA_VERSION
    assert run_summary["diagnostics_schema_version"] == ARTICLE_EVIDENCE_DIAGNOSTICS_SCHEMA_VERSION
    assert restored_dataclass.to_redacted_dict() == dataclass_summary.to_redacted_dict()
    assert json_roundtrip == run_summary
    assert run_summary["import_eligible_count"] == 0
    assert run_summary["promoted_to_fact_count"] == 0
    assert run_summary["production_import_attempted"] is False
    assert run_summary["ladybugdb_written"] is False
    assert_redacted_bridge_payload(run_summary)


# --- Property: fail-closed validation under graph-readiness mutations ---


def mutate_payload(payload: dict[str, Any], mutation: str, forbidden_key: str) -> dict[str, Any]:
    mutated = deepcopy(payload)
    if mutation == "duplicate_source_id":
        mutated["source_refs"].append(dict(mutated["source_refs"][0]))
    elif mutation == "malformed_sha256":
        mutated["source_refs"][0]["sha256"] = "not-a-sha256"
    elif mutation == "unsupported_load_outcome":
        mutated["source_refs"][0]["load_outcome"] = "ready_for_graph"
    elif mutation == "missing_required_subtree":
        mutated["subtrees"].pop("page_index", None)
    elif mutation == "unsupported_subtree_status":
        mutated["subtrees"]["retrieval"]["status"] = "ready_for_graph"
    elif mutation == "unsafe_import_flag":
        mutated["safety_flags"]["trusted_kg_import_allowed"] = True
    elif mutation == "positive_import_count":
        mutated["import_eligible_count"] = 1
    elif mutation == "positive_promoted_count":
        mutated["summary"]["promoted_to_fact_count"] = 1
    elif mutation == "forbidden_payload_key":
        mutated[forbidden_key] = "redacted payload marker"
    else:  # pragma: no cover - keeps exhaustive mutation list honest
        raise AssertionError(f"unknown mutation: {mutation}")
    return mutated


@settings(max_examples=120)
@given(
    bundle=bundle_strategy(min_sources=1),
    mutation=st.sampled_from(
        [
            "duplicate_source_id",
            "malformed_sha256",
            "unsupported_load_outcome",
            "missing_required_subtree",
            "unsupported_subtree_status",
            "unsafe_import_flag",
            "positive_import_count",
            "positive_promoted_count",
            "forbidden_payload_key",
        ]
    ),
    forbidden_key=FORBIDDEN_PAYLOAD_KEYS,
)
def test_mutated_bundles_fail_closed_with_redacted_diagnostics(
    bundle: ArticleEvidenceBundle, mutation: str, forbidden_key: str
) -> None:
    payload = bundle.to_redacted_dict()
    mutated = mutate_payload(payload, mutation, forbidden_key)

    diagnostics = validate_article_evidence_bundle(mutated)

    assert diagnostics, f"{mutation} validated silently"
    assert all(diagnostic["blocks_import"] is True for diagnostic in diagnostics)
    assert all(str(diagnostic["json_path"]).startswith("/") for diagnostic in diagnostics)
    assert_redacted_bridge_payload({"diagnostics": diagnostics})


# --- Focused reviewability and non-importability examples ---


def test_empty_source_list_remains_valid_reviewable_and_non_importable() -> None:
    bundle = ArticleEvidenceBundle(
        paper_id="2605.00001",
        run_id="m024-empty-source-test",
        bundle_id="article-evidence-bundle:empty-source",
        source_refs=(),
    )
    payload = bundle.to_redacted_dict()

    assert payload["summary"]["source_count"] == 0
    assert payload["summary"]["checksum_coverage_rate"] == 0.0
    assert payload["subtrees"]["raw"]["status"] == "absent"
    assert payload["subtrees"]["metrics"]["status"] == "absent"
    assert payload["import_eligible_count"] == 0
    assert payload["promoted_to_fact_count"] == 0
    assert validate_article_evidence_bundle(payload) == []


def test_pdf_metadata_only_source_is_reviewable_but_non_importable() -> None:
    source = ArticleEvidenceSourceReference(
        source_id="article-source:pdfmeta000001",
        paper_id="2605.00002",
        source_path="fixtures/minimal.pdf",
        source_type="pdf",
        media_type="application/pdf",
        sha256="b" * 64,
        byte_size=512,
        parser_name="pdf_metadata_probe",
        loader_name="local_article_loader",
        load_outcome="loaded_metadata_only",
        failure_reason=None,
        warning_count=1,
        duration_ms=25,
    )
    bundle = ArticleEvidenceBundle(
        paper_id="2605.00002",
        run_id="m024-pdf-metadata-test",
        bundle_id="article-evidence-bundle:pdf-metadata",
        source_refs=(source,),
    )
    payload = bundle.to_redacted_dict()

    assert payload["summary"]["metadata_only_count"] == 1
    assert payload["subtrees"]["raw"]["status"] == "metadata_only"
    assert payload["subtrees"]["normalized"]["status"] == "blocked"
    assert payload["subtrees"]["metrics"]["metadata_only_present"] is True
    assert payload["import_eligible_count"] == 0
    assert payload["production_import_attempted"] is False
    assert validate_article_evidence_bundle(payload) == []


def test_failed_low_quality_source_is_reviewable_but_non_importable() -> None:
    source = ArticleEvidenceSourceReference(
        source_id="article-source:lowquality001",
        paper_id="2605.00003",
        source_path="fixtures/arxiv_landing_only.md",
        source_type="markdown",
        media_type="text/markdown",
        sha256=None,
        byte_size=128,
        parser_name="markdown_loader",
        loader_name="local_article_loader",
        load_outcome="failed",
        failure_reason="no_substantive_body",
        warning_count=2,
        duration_ms=12,
    )
    bundle = ArticleEvidenceBundle(
        paper_id="2605.00003",
        run_id="m024-failed-low-quality-test",
        bundle_id="article-evidence-bundle:failed-low-quality",
        source_refs=(source,),
    )
    payload = bundle.to_redacted_dict()

    assert payload["summary"]["failure_count"] == 1
    assert payload["summary"]["failure_counts"] == {"no_substantive_body": 1}
    assert payload["subtrees"]["staging"]["status"] == "blocked"
    assert payload["subtrees"]["metrics"]["failed_source_present"] is True
    assert payload["import_eligible_count"] == 0
    assert payload["ladybugdb_written"] is False
    assert validate_article_evidence_bundle(payload) == []


@settings(max_examples=80)
@given(status=st.text(min_size=1, max_size=30).filter(lambda value: value not in ALLOWED_SUBTREE_STATUSES))
def test_unsupported_subtree_vocabularies_are_diagnostics_not_silent(status: str) -> None:
    bundle = ArticleEvidenceBundle(
        paper_id="2605.00004",
        run_id="m024-subtree-vocab-test",
        bundle_id="article-evidence-bundle:subtree-vocab",
        source_refs=(),
    )
    payload = bundle.to_redacted_dict()
    payload["subtrees"]["page_index"]["status"] = status

    diagnostics = validate_article_evidence_bundle(payload)

    assert "invalid_subtree_status" in {diagnostic["code"] for diagnostic in diagnostics}
    assert all(diagnostic["blocks_import"] is True for diagnostic in diagnostics)
