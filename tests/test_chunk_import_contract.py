from __future__ import annotations

from copy import deepcopy

from research_graph.repair.chunk_import_contract import validate_import_ready_package, validation_to_dict


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
    diagnostics = deepcopy(package["diagnostics"])
    diagnostics["import_eligible_chunk_count"] = 0
    diagnostics["refused_chunk_count"] = 1
    package["diagnostics"] = diagnostics

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


def test_nested_required_fields_are_validated() -> None:
    package = _valid_package()
    package["paper"] = {"paper_id": "wrong", "source_artifacts": ["normalized_markdown:p1"]}
    conversion = deepcopy(package["conversion"])
    conversion.pop("converter")
    package["conversion"] = conversion
    element = deepcopy(package["elements"])[0]
    element.pop("element_type")
    package["elements"] = [element]
    chunk = deepcopy(package["chunks"])[0]
    chunk.pop("order_index")
    package["chunks"] = [chunk]
    annotation = deepcopy(package["annotations"])[0]
    annotation.pop("method")
    package["annotations"] = [annotation]
    evidence_path = deepcopy(package["evidence_paths"])[0]
    evidence_path.pop("provenance_chain")
    package["evidence_paths"] = [evidence_path]

    reasons = _single_reason(package)

    assert "paper_id_mismatch" in reasons
    assert "missing_converter" in reasons
    assert "missing_element_type" in reasons
    assert "missing_order_index" in reasons
    assert "missing_method" in reasons
    assert "missing_provenance_chain" in reasons


def test_conversion_and_diagnostics_redaction_flags_are_validated() -> None:
    package = _valid_package()
    conversion = deepcopy(package["conversion"])
    conversion["raw_text_included"] = True
    conversion["embeddings_included"] = True
    package["conversion"] = conversion
    diagnostics = deepcopy(package["diagnostics"])
    diagnostics["raw_text_included"] = True
    diagnostics["embeddings_included"] = True
    diagnostics["ladybugdb_written"] = True
    diagnostics["production_import_attempted"] = True
    package["diagnostics"] = diagnostics

    reasons = _single_reason(package)

    assert "raw_text_leakage" in reasons
    assert "embedding_leakage" in reasons
    assert "production_write_attempted" in reasons
    assert "production_import_attempted" in reasons


def test_diagnostics_counts_must_match_computed_counts() -> None:
    package = _valid_package()
    diagnostics = deepcopy(package["diagnostics"])
    diagnostics["import_eligible_chunk_count"] = 999
    diagnostics["refused_chunk_count"] = 999
    package["diagnostics"] = diagnostics

    reasons = _single_reason(package)

    assert "diagnostics_import_count_mismatch" in reasons
    assert "diagnostics_refusal_count_mismatch" in reasons


def test_unresolved_annotation_chunk_has_precise_reason() -> None:
    package = _valid_package()
    annotation = deepcopy(package["annotations"])[0]
    annotation["chunk_id"] = "missing-chunk"
    package["annotations"] = [annotation]

    assert "unresolved_annotation_chunk" in _single_reason(package)


def test_graph_ready_retrieval_only_route_is_not_import_eligible() -> None:
    package = _valid_package()
    chunk = deepcopy(package["chunks"])[0]
    chunk["route"] = "retrieval_only"
    chunk["allowed_uses"] = ["trusted_kg_import", "retrieval_diagnostics"]
    package["chunks"] = [chunk]

    result = validate_import_ready_package(package)

    assert "retrieval_only_not_importable" in result.refusal_counts
    assert result.import_eligible_chunk_count == 0


def test_invalid_route_is_rejected_and_not_counted_import_eligible() -> None:
    package = _valid_package()
    chunk = deepcopy(package["chunks"])[0]
    chunk["route"] = "not_a_route"
    package["chunks"] = [chunk]

    result = validate_import_ready_package(package)

    assert "invalid_route" in result.refusal_counts
    assert result.import_eligible_chunk_count == 0


def test_incompatible_route_and_chunk_type_is_rejected() -> None:
    package = _valid_package()
    chunk = deepcopy(package["chunks"])[0]
    chunk["route"] = "table_extraction"
    chunk["chunk_type"] = "claim_candidate"
    package["chunks"] = [chunk]

    result = validate_import_ready_package(package)

    assert "route_chunk_type_mismatch" in result.refusal_counts
    assert result.import_eligible_chunk_count == 0


def test_evidence_path_span_must_contain_chunk_span() -> None:
    package = _valid_package()
    evidence_path = deepcopy(package["evidence_paths"])[0]
    evidence_path["source_span"] = {
        "coordinate_space": "canonical_normalized_markdown",
        "char_start": 1000,
        "char_end": 1100,
        "page_start": None,
        "page_end": None,
    }
    package["evidence_paths"] = [evidence_path]

    result = validate_import_ready_package(package)

    assert "invalid_source_span" in result.refusal_counts
    assert result.import_eligible_chunk_count == 0
