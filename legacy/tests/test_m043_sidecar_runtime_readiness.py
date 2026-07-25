from __future__ import annotations

from scripts import probe_m043_sidecar_runtime_readiness as probe


def _target() -> dict:
    return {
        "article_count": 6,
        "article_keys": ["a", "b", "c", "d", "e", "f"],
    }


def _reuse(all_present: bool = True) -> dict:
    return {
        "systems": {
            name: {"all_prior_artifacts_present": all_present}
            for name in [
                "current_baseline",
                "grobid",
                "opendataloader_pdf",
                "adaptix",
                "quant_mind_patterns",
                "combined_architecture",
            ]
        }
    }


def test_build_runtime_readiness_reports_replayable_and_blockers_when_live_tools_absent():
    readiness = probe.build_runtime_readiness(
        target_subset=_target(),
        reuse_matrix=_reuse(),
        find_spec=lambda _name: None,
        which=lambda _name: None,
        env={},
    )

    assert readiness["target_article_count"] == 6
    assert readiness["checks"]["grobid"]["status"] == "replayable_prior_evidence"
    assert readiness["checks"]["grobid"]["blockers"] == ["grobid_service_url_not_configured"]
    assert readiness["checks"]["opendataloader_pdf"]["status"] == "replayable_prior_evidence"
    assert readiness["checks"]["adaptix"]["status"] == "replayable_prior_evidence"
    assert readiness["checks"]["quant_mind_patterns"]["status"] == "ready_pattern_only"
    assert readiness["graph_write_allowed"] is False
    assert readiness["production_import_attempted"] is False


def test_build_runtime_readiness_marks_live_hints_ready_without_network_call():
    importable = {"docling", "adaptix"}
    readiness = probe.build_runtime_readiness(
        target_subset=_target(),
        reuse_matrix=_reuse(),
        find_spec=lambda name: object() if name in importable else None,
        which=lambda name: "/usr/bin/docker" if name == "docker" else None,
        env={"GROBID_URL": "http://localhost:8070"},
    )

    assert readiness["checks"]["grobid"]["status"] == "live_ready"
    assert readiness["checks"]["grobid"]["local_hints"] == {
        "GROBID_URL_set": True,
        "docker_available": True,
    }
    assert readiness["checks"]["opendataloader_pdf"]["status"] == "live_ready"
    assert readiness["checks"]["adaptix"]["status"] == "live_ready"
    assert (
        readiness["checks"]["quant_mind_patterns"]["runtime_dependency_adoption_authorized"]
        is False
    )


def test_build_runtime_readiness_rejects_wrong_target_size():
    target = _target()
    target["article_count"] = 5

    try:
        probe.build_runtime_readiness(target_subset=target, reuse_matrix=_reuse())
    except ValueError as exc:
        assert "6-node connected component" in str(exc)
    else:  # pragma: no cover - defensive failure branch
        raise AssertionError("expected ValueError")


def test_render_markdown_includes_system_statuses():
    readiness = probe.build_runtime_readiness(
        target_subset=_target(),
        reuse_matrix=_reuse(),
        find_spec=lambda _name: None,
        which=lambda _name: None,
        env={},
    )

    markdown = probe.render_markdown(readiness)

    assert "grobid" in markdown
    assert "opendataloader_pdf" in markdown
    assert "quant_mind_patterns" in markdown
    assert "Graph writes: disabled" in markdown
