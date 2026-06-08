from __future__ import annotations

import pytest

from arxiv_archive.universal_kb_contracts import CandidatePacket
from arxiv_archive.universal_kb_sidecar_boundary import (
    BoundaryDomainInvariantError,
    BoundaryMappingError,
    candidate_packet_from_sidecar_json,
)


def _valid_payload() -> dict:
    return {
        "candidate": {
            "id": "sidecar-candidate-1",
            "type": "section_summary",
            "evidence_refs": ["artifact:fixture-paper:section:1"],
        },
        "review": {"state": "pending"},
        "source": {"sidecar": "opendataloader-fixture", "version": "v1"},
    }


def test_valid_sidecar_json_maps_to_candidate_packet() -> None:
    packet = candidate_packet_from_sidecar_json(_valid_payload())

    assert isinstance(packet, CandidatePacket)
    assert packet.candidate_id == "sidecar-candidate-1"
    assert packet.candidate_type == "section_summary"
    assert packet.evidence_refs == ("artifact:fixture-paper:section:1",)
    assert packet.review_state == "pending"
    assert packet.safety_flags.graphdb_written is False
    assert packet.safety_flags.production_import_attempted is False


def test_boundary_mapping_error_is_separate_from_domain_invariant_error() -> None:
    payload = _valid_payload()
    del payload["candidate"]["id"]

    with pytest.raises(BoundaryMappingError) as exc_info:
        candidate_packet_from_sidecar_json(payload)

    assert exc_info.value.diagnostics[0].code == "sidecar_mapping_error"
    assert exc_info.value.diagnostics[0].path == "/"


def test_domain_invariant_error_reports_candidate_contract_failure() -> None:
    payload = _valid_payload()
    payload["review"]["state"] = "approved"

    with pytest.raises(BoundaryDomainInvariantError) as exc_info:
        candidate_packet_from_sidecar_json(payload)

    assert exc_info.value.diagnostics[0].code == "candidate_domain_invariant_error"
    assert "approved" not in exc_info.value.diagnostics[0].message


@pytest.mark.parametrize(
    ("field", "value"),
    [
        ("import_eligible", True),
        ("graph_import_allowed", True),
        ("graphdb_written", True),
        ("ladybugdb_written", True),
        ("production_import_attempted", True),
        ("trusted_kg_import_allowed", True),
    ],
)
def test_external_authority_flags_cannot_widen_candidate_safety(field: str, value: bool) -> None:
    payload = _valid_payload()
    payload["candidate"][field] = value
    payload[field] = value

    packet = candidate_packet_from_sidecar_json(payload)

    packet.assert_no_write()
    assert packet.safety_flags.import_eligible is False
    assert packet.safety_flags.graph_import_allowed is False
    assert packet.safety_flags.graphdb_written is False
    assert packet.safety_flags.ladybugdb_written is False
    assert packet.safety_flags.production_import_attempted is False


def test_malformed_external_json_does_not_echo_payload_values() -> None:
    payload = _valid_payload()
    payload["candidate"]["evidence_refs"] = "raw_text: secret payload"

    with pytest.raises(BoundaryMappingError) as exc_info:
        candidate_packet_from_sidecar_json(payload)

    diagnostic = exc_info.value.diagnostics[0]
    assert diagnostic.code == "sidecar_mapping_error"
    assert "secret payload" not in diagnostic.message
    assert "raw_text" not in diagnostic.message
