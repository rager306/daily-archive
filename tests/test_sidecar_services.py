"""Parser sidecar probe / ensure (offline-safe unit tests)."""

from __future__ import annotations

from research_graph.infrastructure.corpus.parsing.sidecar_services import (
    grobid_isalive_url,
    probe_grobid,
    probe_opendataloader,
    probe_parser_sidecars,
)


def test_grobid_isalive_url_join() -> None:
    assert grobid_isalive_url("http://127.0.0.1:8070").endswith("/api/isalive")
    assert (
        grobid_isalive_url("http://127.0.0.1:8070/api/isalive")
        == "http://127.0.0.1:8070/api/isalive"
    )


def test_probe_grobid_when_down_is_fail_closed() -> None:
    # Port 9 is almost never GROBID; expect unavailable without raising.
    probe = probe_grobid("http://127.0.0.1:9", timeout_s=0.5)
    assert probe.name == "grobid"
    assert probe.available is False
    assert "connection_failed" in probe.diagnostics or probe.detail


def test_probe_opendataloader_returns_structured_status() -> None:
    probe = probe_opendataloader()
    assert probe.name == "opendataloader"
    assert isinstance(probe.available, bool)
    assert probe.detail


def test_probe_parser_sidecars_without_ensure() -> None:
    status = probe_parser_sidecars(ensure=False)
    payload = status.to_dict()
    assert "grobid" in payload
    assert "opendataloader" in payload
    assert payload["compose_file"].endswith("docker-compose.yml")
