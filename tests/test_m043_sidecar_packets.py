from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MODULE_PATH = ROOT / "scripts" / "build_m043_sidecar_packets.py"
spec = importlib.util.spec_from_file_location("build_m043_sidecar_packets", MODULE_PATH)
assert spec is not None
packets_mod = importlib.util.module_from_spec(spec)
sys.modules["build_m043_sidecar_packets"] = packets_mod
assert spec.loader is not None
spec.loader.exec_module(packets_mod)


def _target() -> dict:
    return {
        "articles": [
            {
                "article_key": "a1",
                "m041_category": "baseline",
                "article_ref": "artifact:data/a1/article.json",
                "catalog_path": "arxiv/cs-ai/a1",
            }
        ],
        "graph_write_allowed": False,
        "promotion_allowed": False,
        "production_import_attempted": False,
        "import_eligible": False,
    }


def _source(*, local_pdf_count: int = 0) -> dict:
    return {
        "records": [
            {
                "article_key": "a1",
                "identity_metadata_status": "fetched",
                "pdf_url_present": True,
                "local_pdf_count": local_pdf_count,
                "local_source_file_count": 1,
                "local_loader_file_count": 0,
                "linked_from_count": 0,
            }
        ],
        "graph_write_allowed": False,
        "promotion_allowed": False,
        "production_import_attempted": False,
        "import_eligible": False,
    }


def _runtime(
    *,
    grobid: str = "replayable_prior_evidence",
    opendataloader: str = "live_ready",
    adaptix: str = "live_ready",
) -> dict:
    return {
        "checks": {
            "current_baseline": {"status": "ready"},
            "grobid": {"status": grobid},
            "opendataloader_pdf": {"status": opendataloader},
            "adaptix": {"status": adaptix},
            "quant_mind_patterns": {"status": "ready_pattern_only"},
            "combined_architecture": {"status": "ready_recommendation"},
        },
        "graph_write_allowed": False,
        "promotion_allowed": False,
        "production_import_attempted": False,
        "import_eligible": False,
    }


def _reuse() -> dict:
    return {
        "systems": {
            system: {"prior_artifacts": [f"artifact/{system}.json"]}
            for system in packets_mod.SYSTEMS
        },
        "graph_write_allowed": False,
        "promotion_allowed": False,
        "production_import_attempted": False,
        "import_eligible": False,
    }


def test_build_packets_blocks_pdf_sidecars_without_local_pdf():
    packet = packets_mod.build_packets(
        target=_target(),
        source_readiness=_source(local_pdf_count=0),
        runtime=_runtime(),
        reuse=_reuse(),
    )

    article = packet["packets"][0]
    assert article["sidecars"]["current_baseline"]["status"] == "ready_contract_reference"
    assert (
        article["sidecars"]["grobid"]["status"]
        == "blocked_target_specific_run_replayable_prior_evidence"
    )
    assert (
        article["sidecars"]["opendataloader_pdf"]["status"]
        == "blocked_target_specific_run_replayable_prior_evidence"
    )
    assert (
        article["sidecars"]["adaptix"]["status"]
        == "blocked_waiting_for_target_opendataloader_fixed_json"
    )
    assert "local_pdf_missing" in article["sidecars"]["opendataloader_pdf"]["blockers"]
    assert packet["forbidden_payload_fields_absent"] is True
    assert packet["graph_write_allowed"] is False


def test_build_packets_marks_pdf_sidecars_ready_when_live_and_pdf_present():
    packet = packets_mod.build_packets(
        target=_target(),
        source_readiness=_source(local_pdf_count=1),
        runtime=_runtime(grobid="live_ready", opendataloader="live_ready", adaptix="live_ready"),
        reuse=_reuse(),
    )

    sidecars = packet["packets"][0]["sidecars"]
    assert sidecars["grobid"]["status"] == "ready_for_bounded_live_pdf_probe"
    assert sidecars["opendataloader_pdf"]["status"] == "ready_for_bounded_live_pdf_probe"
    assert sidecars["adaptix"]["status"] == "ready_after_opendataloader_fixed_json"
    assert sidecars["quant_mind_patterns"]["status"] == "ready_pattern_mapping_only"


def test_build_packets_rejects_enabled_safety_flag():
    target = _target()
    target["graph_write_allowed"] = True

    try:
        packets_mod.build_packets(
            target=target, source_readiness=_source(), runtime=_runtime(), reuse=_reuse()
        )
    except ValueError as exc:
        assert "safety flag" in str(exc)
    else:  # pragma: no cover - defensive failure branch
        raise AssertionError("expected ValueError")


def test_assert_no_forbidden_fields_rejects_raw_text():
    try:
        packets_mod.assert_no_forbidden_fields({"raw_text": "secret payload"})
    except ValueError as exc:
        assert "forbidden payload fields" in str(exc)
    else:  # pragma: no cover - defensive failure branch
        raise AssertionError("expected ValueError")


def test_render_markdown_includes_status_counts():
    packet = packets_mod.build_packets(
        target=_target(), source_readiness=_source(), runtime=_runtime(), reuse=_reuse()
    )
    markdown = packets_mod.render_markdown(packet)

    assert "Graph writes: disabled" in markdown
    assert "blocked_target_specific_run_replayable_prior_evidence" in markdown
    assert "ready_pattern_mapping_only" in markdown
