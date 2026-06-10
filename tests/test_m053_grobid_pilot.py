"""Tests for scripts/probe_m053_grobid_pilot.py (M053 S01).

Covers bounded GROBID probing, fail-closed diagnostics, dry-run behavior,
low-quality classification, atomic writes, safety defaults, and idempotency.
"""

from __future__ import annotations

import copy
import json
import subprocess
import sys
import urllib.error
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

import probe_m053_grobid_pilot as probe  # noqa: E402


class FakeResponse:
    def __init__(self, body: bytes = b"", status: int = 200) -> None:
        self._body = body
        self.status = status

    def __enter__(self) -> "FakeResponse":
        return self

    def __exit__(self, *args: object) -> None:
        return None

    def read(self) -> bytes:
        return self._body


def tei_payload(*, refs: int = 1, bodies: int = 1, padding: int = 1200) -> bytes:
    ref_xml = "".join("<ref>r</ref>" for _ in range(refs))
    body_xml = "".join(f"<body>{ref_xml}<p>{'x' * padding}</p></body>" for _ in range(bodies))
    return f"<TEI xmlns='http://www.tei-c.org/ns/1.0'><text>{body_xml}</text></TEI>".encode()


def make_pdf(tmp_path: Path, name: str = "paper.pdf") -> Path:
    pdf_path = tmp_path / name
    pdf_path.write_bytes(b"%PDF-1.4 fake")
    return pdf_path


def make_target(tmp_path: Path, paper_id: str = "paper") -> probe.PdfTarget:
    return probe.PdfTarget(paper_id, make_pdf(tmp_path, f"{paper_id}.pdf"))


def strip_generated_at(value: object) -> object:
    if isinstance(value, dict):
        return {k: strip_generated_at(v) for k, v in value.items() if k != "generated_at"}
    if isinstance(value, list):
        return [strip_generated_at(v) for v in value]
    return value


def test_service_available_uses_isalive_endpoint(monkeypatch: pytest.MonkeyPatch) -> None:
    seen: dict[str, object] = {}

    def fake_urlopen(request: urllib.request.Request, timeout: int) -> FakeResponse:
        seen["url"] = request.full_url
        seen["timeout"] = timeout
        return FakeResponse(status=200)

    monkeypatch.setattr(probe.urllib.request, "urlopen", fake_urlopen)

    assert probe.check_grobid_available("http://grobid.test/") is True
    assert seen == {"url": "http://grobid.test/api/isalive", "timeout": 5}


def test_service_unavailable_on_connection_error(monkeypatch: pytest.MonkeyPatch) -> None:
    def fake_urlopen(*_args: object, **_kwargs: object) -> FakeResponse:
        raise ConnectionError("service down")

    monkeypatch.setattr(probe.urllib.request, "urlopen", fake_urlopen)

    assert probe.check_grobid_available("http://grobid.test") is False


def test_post_grobid_header_sends_multipart_to_header_endpoint(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    pdf_path = make_pdf(tmp_path)
    seen: dict[str, object] = {}

    def fake_urlopen(request: urllib.request.Request, timeout: int) -> FakeResponse:
        seen["url"] = request.full_url
        seen["timeout"] = timeout
        seen["content_type"] = request.headers["Content-type"]
        data = request.data or b""
        seen["has_pdf"] = b"%PDF-1.4 fake" in data
        seen["has_flags"] = b"consolidateHeader" in data and b"consolidateCitations" in data
        return FakeResponse(tei_payload(), status=200)

    monkeypatch.setattr(probe.urllib.request, "urlopen", fake_urlopen)

    body, status = probe.post_grobid_header(pdf_path, "http://grobid.test", timeout=17)

    assert status == 200
    assert body == tei_payload()
    assert seen["url"] == "http://grobid.test/api/processHeaderDocument"
    assert seen["timeout"] == 17
    assert str(seen["content_type"]).startswith("multipart/form-data; boundary=")
    assert seen["has_pdf"] is True
    assert seen["has_flags"] is True


def test_probe_success_writes_tei_and_packet(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    target = make_target(tmp_path, "success")
    tei = tei_payload(refs=2, bodies=1, padding=1200)
    monkeypatch.setattr(probe, "post_grobid_header", lambda *_args, **_kwargs: (tei, 200))

    packet = probe.probe_pdf(target, grobid_url="http://grobid.test", output_dir=tmp_path / "out")

    assert packet["status"] == "success"
    assert packet["tei_size_bytes"] == len(tei)
    assert packet["ref_count"] == 2
    assert packet["body_element_count"] == 1
    assert packet["attempts"][0]["outcome"] == "success"
    assert Path(packet["tei_path"]).read_bytes() == tei
    assert json.loads((tmp_path / "out" / "success.json").read_text())["status"] == "success"


def test_low_quality_source_when_tei_is_under_one_kib(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    target = make_target(tmp_path, "small")
    monkeypatch.setattr(probe, "post_grobid_header", lambda *_args, **_kwargs: (tei_payload(padding=10), 200))

    packet = probe.probe_pdf(target, grobid_url="http://grobid.test", output_dir=tmp_path / "out")

    assert packet["status"] == "low_quality_source"
    assert packet["low_quality_source"] is True
    assert packet["m022_repair_candidate"] is True


def test_low_quality_source_when_zero_ref_elements(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    target = make_target(tmp_path, "noref")
    monkeypatch.setattr(probe, "post_grobid_header", lambda *_args, **_kwargs: (tei_payload(refs=0, bodies=1, padding=1400), 200))

    packet = probe.probe_pdf(target, grobid_url="http://grobid.test", output_dir=tmp_path / "out")

    assert packet["status"] == "low_quality_source"
    assert packet["ref_count"] == 0
    assert packet["body_element_count"] == 1


def test_low_quality_source_when_zero_body_elements(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    target = make_target(tmp_path, "nobody")
    tei = b"<TEI><text>" + b"<ref>r</ref>" + (b"x" * 1200) + b"</text></TEI>"
    monkeypatch.setattr(probe, "post_grobid_header", lambda *_args, **_kwargs: (tei, 200))

    packet = probe.probe_pdf(target, grobid_url="http://grobid.test", output_dir=tmp_path / "out")

    assert packet["status"] == "low_quality_source"
    assert packet["ref_count"] == 1
    assert packet["body_element_count"] == 0


def test_bounded_retry_stops_after_three_attempts(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    target = make_target(tmp_path, "retry")
    calls = 0

    def fail(*_args: object, **_kwargs: object) -> tuple[bytes, int]:
        nonlocal calls
        calls += 1
        raise urllib.error.URLError("temporary network failure")

    monkeypatch.setattr(probe, "post_grobid_header", fail)
    monkeypatch.setattr(probe.time, "sleep", lambda *_args, **_kwargs: None)

    packet = probe.probe_pdf(target, grobid_url="http://grobid.test", output_dir=tmp_path / "out", max_retries=3)

    assert calls == 3
    assert packet["status"] == "network_error"
    assert len(packet["attempts"]) == 3


def test_success_on_first_attempt_does_not_retry(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    target = make_target(tmp_path, "first")
    mocked = mock.Mock(return_value=(tei_payload(), 200))
    monkeypatch.setattr(probe, "post_grobid_header", mocked)

    packet = probe.probe_pdf(target, grobid_url="http://grobid.test", output_dir=tmp_path / "out", max_retries=3)

    assert packet["status"] == "success"
    assert mocked.call_count == 1
    assert len(packet["attempts"]) == 1


def test_blocked_http_error_stops_without_retry(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    target = make_target(tmp_path, "blocked")
    error = urllib.error.HTTPError("http://grobid.test", 400, "bad", hdrs=None, fp=None)
    mocked = mock.Mock(side_effect=error)
    monkeypatch.setattr(probe, "post_grobid_header", mocked)

    packet = probe.probe_pdf(target, grobid_url="http://grobid.test", output_dir=tmp_path / "out", max_retries=3)

    assert packet["status"] == "blocked"
    assert packet["http_status"] == 400
    assert mocked.call_count == 1


def test_atomic_write_uses_tmp_then_replace(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    target = make_target(tmp_path, "atomic")
    tei = tei_payload()
    replacements: list[tuple[str, str]] = []
    original_replace = Path.replace

    def recording_replace(self: Path, target_path: Path) -> Path:
        replacements.append((self.name, Path(target_path).name))
        return original_replace(self, target_path)

    monkeypatch.setattr(probe, "post_grobid_header", lambda *_args, **_kwargs: (tei, 200))
    monkeypatch.setattr(Path, "replace", recording_replace)

    probe.probe_pdf(target, grobid_url="http://grobid.test", output_dir=tmp_path / "out")

    assert ("atomic.tei.xml.tmp", "atomic.tei.xml") in replacements
    assert ("atomic.json.tmp", "atomic.json") in replacements


def test_cli_dry_run_subprocess_does_not_invoke_urlopen(tmp_path: Path) -> None:
    pdf_path = make_pdf(tmp_path, "dry.pdf")
    out_dir = tmp_path / "out"
    code = f"""
import runpy, sys, urllib.request

def fail(*args, **kwargs):
    raise RuntimeError('urlopen invoked')

urllib.request.urlopen = fail
sys.argv = ['probe', '--dry-run', '--pdf-path', {str(pdf_path)!r}, '--output-dir', {str(out_dir)!r}]
runpy.run_path({str(ROOT / 'scripts' / 'probe_m053_grobid_pilot.py')!r}, run_name='__main__')
"""

    subprocess.run([sys.executable, "-c", code], cwd=ROOT, check=True, capture_output=True, text=True)

    packet = json.loads((out_dir / "dry.json").read_text(encoding="utf-8"))
    assert packet["status"] == "grobid_unavailable"
    assert packet["attempts"] == []


def test_packet_schema_required_fields_present(tmp_path: Path) -> None:
    target = make_target(tmp_path, "schema")
    packet = probe.probe_pdf(target, grobid_url="http://grobid.test", output_dir=tmp_path / "out", dry_run=True)

    required = {
        "schema_version",
        "generated_at",
        "paper_id",
        "status",
        "tei_size_bytes",
        "ref_count",
        "body_element_count",
        "low_quality_source",
        "http_status",
        "attempts",
        "m022_repair_candidate",
        "safety_defaults",
    }
    assert required <= packet.keys()
    assert packet["schema_version"] == probe.SCHEMA_VERSION
    assert packet["status"] in probe.VALID_STATUSES


def test_safety_defaults_all_false_on_every_output(tmp_path: Path) -> None:
    target = make_target(tmp_path, "safe")
    summary = probe.run_probe([target], output_dir=tmp_path / "out", grobid_url="http://grobid.test", dry_run=True)
    packet = json.loads((tmp_path / "out" / "safe.json").read_text(encoding="utf-8"))

    assert summary["safety_defaults"] == probe.SAFETY_DEFAULTS
    assert packet["safety_defaults"] == probe.SAFETY_DEFAULTS
    assert all(value is False for value in summary["safety_defaults"].values())
    assert all(value is False for value in packet["safety_defaults"].values())


def test_m022_repair_candidate_true_for_low_quality_source(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    target = make_target(tmp_path, "repair")
    monkeypatch.setattr(probe, "post_grobid_header", lambda *_args, **_kwargs: (tei_payload(refs=0, padding=1400), 200))

    packet = probe.probe_pdf(target, grobid_url="http://grobid.test", output_dir=tmp_path / "out")

    assert packet["status"] == "low_quality_source"
    assert packet["m022_repair_candidate"] is True


def test_summary_idempotent_modulo_generated_at(tmp_path: Path) -> None:
    targets = [probe.PdfTarget("idem-a", make_pdf(tmp_path, "idem-a.pdf")), probe.PdfTarget("idem-b", make_pdf(tmp_path, "idem-b.pdf"))]

    first = copy.deepcopy(probe.run_probe(targets, output_dir=tmp_path / "out", grobid_url="http://grobid.test", dry_run=True))
    second = copy.deepcopy(probe.run_probe(targets, output_dir=tmp_path / "out", grobid_url="http://grobid.test", dry_run=True))

    assert strip_generated_at(first) == strip_generated_at(second)
