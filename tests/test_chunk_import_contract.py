from __future__ import annotations

from copy import deepcopy

from arxiv_archive.chunk_import_contract import validate_import_ready_package, validation_to_dict


def _source_span() -> dict[str, object]:
    return {
        "coordinate_space": "canonical_normalized_markdown",
        "char_start": 10,
        "char_end": 120,
        "page_start": None,
        "page_end": None,
    }


def _valid_package(**overrides: object) -> dict[str, object]:
    package: dict[str, object] = {
        "schema_version": "m005-import-ready-chunk-package.v1",
        "contract_version": "import-ready-chunk-contract.v1",
        "run_id": "run-1",
        "created_at": "2026-05-19T00:00:00Z",
        "paper_id": "p1",
        "paper": {
            "paper_id": "p1",
            "title": "Example",
            "categories": ["cs.AI"],
            "source_artifacts": ["normalized_markdown:p1"],
        },
        "conversion": {
            "conversion_id": "conversion:p1",
            "converter": "manual_fixture",
            "converter_version": None,
            "source_artifact": "normalized_markdown:p1",
            "quality_state": "ok_for_graph",
            "warnings": [],
            "raw_text_included": False,
            "embeddings_included": False,
        },
        "elements": [
            {
                "element_id": "element:p1:intro:paragraph-1",
                "paper_id": "p1",
                "element_type": "paragraph",
                "parent_element_id": None,
                "section_path": ["Introduction"],
                "order_index": 1,
                "source_span": _source_span(),
                "quality_state": "ok_for_graph",
                "warnings": [],
            }
        ],
        "chunks": [
            {
                "chunk_id": "chunk:p1:intro:claim-1",
                "paper_id": "p1",
                "parent_chunk_id": None,
                "parent_element_ids": ["element:p1:intro:paragraph-1"],
                "section_path": ["Introduction"],
                "chunk_type": "claim_candidate",
                "route": "claim_extraction",
                "state": "ok_for_graph",
                "allowed_uses": ["trusted_kg_import", "claim_extraction", "retrieval_diagnostics"],
                "excluded_uses": [],
                "order_index": 1,
                "source_span": _source_span(),
                "source_artifact": "normalized_markdown:p1",
                "evidence_path_id": "evidence:p1:claim-1",
                "quality_warnings": [],
                "redaction": {
                    "raw_text_included": False,
                    "chunk_text_included": False,
                    "embeddings_included": False,
                    "vectors_included": False,
                    "secrets_included": False,
                },
            }
        ],
        "annotations": [
            {
                "annotation_id": "annotation:p1:claim-1:rules",
                "paper_id": "p1",
                "chunk_id": "chunk:p1:intro:claim-1",
                "method": "rules",
                "method_version": "fixture",
                "annotation_type": "route_hint",
                "values": [{"code": "contains_citation_marker"}],
                "confidence_class": "diagnostic",
                "promoted_to_fact": False,
                "warnings": [],
            }
        ],
        "evidence_paths": [
            {
                "evidence_path_id": "evidence:p1:claim-1",
                "paper_id": "p1",
                "chunk_id": "chunk:p1:intro:claim-1",
                "source_element_ids": ["element:p1:intro:paragraph-1"],
                "source_artifact": "normalized_markdown:p1",
                "source_span": _source_span(),
                "provenance_chain": ["conversion:p1", "element:p1:intro:paragraph-1", "chunk:p1:intro:claim-1"],
            }
        ],
        "diagnostics": {
            "package_state": "ok_for_graph",
            "valid_package": True,
            "import_eligible_chunk_count": 1,
            "refused_chunk_count": 0,
            "counts_by_state": {"ok_for_graph": 1},
            "counts_by_route": {"claim_extraction": 1},
            "counts_by_chunk_type": {"claim_candidate": 1},
            "refusal_counts": {},
            "source_span_coverage": 1.0,
            "parent_reference_resolution_rate": 1.0,
            "evidence_path_resolution_rate": 1.0,
            "raw_text_included": False,
            "embeddings_included": False,
            "ladybugdb_written": False,
            "production_import_attempted": False,
        },
    }
    package.update(overrides)
    return package


def _single_reason(package: dict[str, object]) -> set[str]:
    return set(validate_import_ready_package(package).refusal_counts)


def test_valid_import_ready_package_passes() -> None:
    result = validate_import_ready_package(_valid_package())

    assert result.passed is True
    assert result.valid_package is True
    assert result.import_eligible_chunk_count == 1
    assert result.refused_chunk_count == 0
    assert result.diagnostics == []


def test_missing_chunk_id_is_rejected() -> None:
    package = _valid_package()
    chunk = deepcopy(package["chunks"])[0]
    chunk.pop("chunk_id")
    package["chunks"] = [chunk]

    assert "missing_chunk_id" in _single_reason(package)


def test_graph_ready_chunk_missing_source_span_is_rejected() -> None:
    package = _valid_package()
    chunk = deepcopy(package["chunks"])[0]
    chunk["source_span"] = None
    package["chunks"] = [chunk]

    assert "missing_source_span" in _single_reason(package)


def test_unresolved_parent_element_is_rejected() -> None:
    package = _valid_package()
    chunk = deepcopy(package["chunks"])[0]
    chunk["parent_element_ids"] = ["missing-element"]
    package["chunks"] = [chunk]

    assert "unresolved_parent_element" in _single_reason(package)


def test_missing_or_unresolved_evidence_path_is_rejected() -> None:
    missing = _valid_package()
    chunk = deepcopy(missing["chunks"])[0]
    chunk["evidence_path_id"] = None
    missing["chunks"] = [chunk]

    unresolved = _valid_package(evidence_paths=[])

    assert "missing_evidence_path" in _single_reason(missing)
    assert "unresolved_evidence_path" in _single_reason(unresolved)


def test_retrieval_only_chunk_cannot_request_trusted_import() -> None:
    package = _valid_package()
    chunk = deepcopy(package["chunks"])[0]
    chunk["state"] = "ok_for_retrieval_only"
    chunk["route"] = "retrieval_only"
    package["chunks"] = [chunk]

    result = validate_import_ready_package(package)

    assert "retrieval_only_not_importable" in result.refusal_counts
    assert result.import_eligible_chunk_count == 0
    assert result.refused_chunk_count == 1


def test_raw_text_embeddings_and_vectors_are_rejected() -> None:
    package = _valid_package()
    chunk = deepcopy(package["chunks"])[0]
    chunk["chunk_text"] = "raw text must not be in machine artifacts"
    chunk["embeddings"] = [0.1]
    chunk["vector"] = [0.2]
    package["chunks"] = [chunk]

    reasons = _single_reason(package)

    assert "raw_text_leakage" in reasons
    assert "embedding_leakage" in reasons
    assert "vector_leakage" in reasons


def test_annotation_promoted_to_fact_is_rejected() -> None:
    package = _valid_package()
    annotation = deepcopy(package["annotations"])[0]
    annotation["promoted_to_fact"] = True
    package["annotations"] = [annotation]

    assert "annotation_promoted_to_fact" in _single_reason(package)


def test_reference_chunk_polluting_claim_route_is_rejected() -> None:
    package = _valid_package()
    chunk = deepcopy(package["chunks"])[0]
    chunk["chunk_type"] = "reference_entry"
    package["chunks"] = [chunk]

    assert "reference_pollutes_claim_route" in _single_reason(package)


def test_retrieval_only_package_can_be_valid_but_not_import_ready() -> None:
    package = _valid_package()
    chunk = deepcopy(package["chunks"])[0]
    chunk["state"] = "ok_for_retrieval_only"
    chunk["route"] = "retrieval_only"
    chunk["allowed_uses"] = ["retrieval_diagnostics"]
    chunk["excluded_uses"] = ["trusted_kg_import"]
    chunk["evidence_path_id"] = None
    package["chunks"] = [chunk]

    result = validate_import_ready_package(package)

    assert result.valid_package is True
    assert result.passed is True
    assert result.import_eligible_chunk_count == 0
    assert result.refused_chunk_count == 1


def test_validation_serialization_is_redacted() -> None:
    payload = validation_to_dict(validate_import_ready_package(_valid_package()))

    assert payload["schema_version"] == "m005-import-contract-validation.v1"
    assert payload["raw_text_included"] is False
    assert payload["embeddings_included"] is False
    assert payload["ladybugdb_written"] is False
    assert payload["production_import_attempted"] is False
