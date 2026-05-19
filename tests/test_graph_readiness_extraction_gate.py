from __future__ import annotations

import json
from pathlib import Path

from arxiv_archive.graph_readiness_extraction_gate import decide_extraction, run_extraction_gate


def _manifest(entries: list[dict[str, object]]) -> dict[str, object]:
    return {
        "schema_version": "s05-eligibility-manifest.v1",
        "scope": "prose_claims_only",
        "entries": entries,
    }


def _entry(paper_id: str, route: str, final_eligibility: str) -> dict[str, object]:
    return {
        "paper_id": paper_id,
        "route": route,
        "final_eligibility": final_eligibility,
        "independent_review_verdict": "FLAG",
        "finding_codes": ["paper_level_trust_overclaim"],
        "caveats": ["route-level caveat"],
        "required_repairs": [],
    }


def test_decide_extraction_blocks_when_only_caveated_routes_exist() -> None:
    manifest = _manifest(
        [
            _entry("p1", "claim_extraction", "eligible_with_caveat"),
            _entry("p1", "method_extraction", "eligible_with_caveat"),
            _entry("p1", "retrieval_only", "eligible_with_caveat"),
        ]
    )

    result = decide_extraction(manifest)

    assert result.extraction_attempted is False
    assert result.blocked_reason == "no_trusted_extraction_routes_after_review"
    assert len(result.caveated_entries) == 2
    assert len(result.excluded_entries) == 1


def test_decide_extraction_allows_trusted_routes_and_skips_excluded() -> None:
    manifest = _manifest(
        [
            _entry("p1", "claim_extraction", "eligible"),
            _entry("p1", "table_extraction", "route_excluded"),
            _entry("p2", "paper", "review_required"),
        ]
    )

    result = decide_extraction(manifest)

    assert result.extraction_attempted is True
    assert result.blocked_reason is None
    assert [entry["route"] for entry in result.trusted_entries] == ["claim_extraction"]
    assert [entry["route"] for entry in result.excluded_entries] == ["table_extraction"]
    assert [entry["route"] for entry in result.skipped_entries] == ["paper"]


def test_allow_caveated_extraction_is_explicit() -> None:
    manifest = _manifest([_entry("p1", "entity_candidate_extraction", "eligible_with_caveat")])

    result = decide_extraction(manifest, require_trusted=False)

    assert result.extraction_attempted is True
    assert result.caveated_entries[0]["route"] == "entity_candidate_extraction"


def test_run_extraction_gate_writes_redacted_summary_and_events(tmp_path: Path) -> None:
    manifest_path = tmp_path / "manifest.json"
    output_dir = tmp_path / "out"
    manifest_path.write_text(
        json.dumps(
            _manifest(
                [
                    _entry("p1", "claim_extraction", "eligible_with_caveat"),
                    _entry("p1", "citation_graph", "route_excluded"),
                    _entry("p2", "paper", "review_required"),
                ]
            )
        ),
        encoding="utf-8",
    )

    summary = run_extraction_gate(manifest_path=manifest_path, output_dir=output_dir)

    summary_path = output_dir / "extraction-route-summary.json"
    events_path = output_dir / "extraction-events.jsonl"
    assert summary_path.exists()
    assert events_path.exists()
    assert summary["extraction_attempted"] is False
    assert summary["counts"] == {
        "trusted_routes": 0,
        "caveated_routes": 1,
        "excluded_routes": 1,
        "skipped_routes": 1,
        "claims": 0,
        "entities": 0,
        "relations": 0,
    }
    events_text = events_path.read_text(encoding="utf-8")
    assert "raw_text_included" in events_text
    assert "route-level caveat" in events_text
