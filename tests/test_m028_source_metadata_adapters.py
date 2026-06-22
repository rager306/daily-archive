"""Contract tests for the M028 source metadata adapter and verifier scripts."""

from __future__ import annotations

import importlib.util
import json
from copy import deepcopy
from pathlib import Path
from types import ModuleType

import pytest

REPO_ROOT = Path(__file__).parents[1]
BUILD_SCRIPT_PATH = REPO_ROOT / "scripts" / "build_m028_source_metadata_adapters.py"
VERIFY_SCRIPT_PATH = REPO_ROOT / "scripts" / "verify_m028_source_metadata_adapters.py"
REAL_CORPUS_DIR = REPO_ROOT / "data" / "article_corpora" / "m028-universal-loader-runtime-smoke-v1"


def _load_script(path: Path, module_name: str) -> ModuleType:
    spec = importlib.util.spec_from_file_location(module_name, path)
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _load_build_script() -> ModuleType:
    return _load_script(BUILD_SCRIPT_PATH, "build_m028_source_metadata_adapters")


def _load_verify_script() -> ModuleType:
    return _load_script(VERIFY_SCRIPT_PATH, "verify_m028_source_metadata_adapters")


def _write_json(path: Path, payload: dict[str, object]) -> None:
    path.write_text(json.dumps(payload, indent=2, sort_keys=True), encoding="utf-8")


def _write_jsonl(path: Path, rows: list[dict[str, object]]) -> None:
    path.write_text(
        "\n".join(json.dumps(row, sort_keys=True) for row in rows) + "\n", encoding="utf-8"
    )


def _read_json(path: Path) -> dict[str, object]:
    return json.loads(path.read_text(encoding="utf-8"))


def _read_jsonl(path: Path) -> list[dict[str, object]]:
    return [
        json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()
    ]


def _sha256(path: Path) -> str:
    import hashlib

    return hashlib.sha256(path.read_bytes()).hexdigest()


def _selection() -> dict[str, object]:
    return {
        "refs": [
            {
                "ref_id": "R01",
                "url": "https://arxiv.org/pdf/2605.20897.pdf",
                "canonical_url": "https://arxiv.org/abs/2605.20897",
                "source_kind": "arxiv_pdf_url",
                "normalized_identity": "arxiv:2605.20897",
                "arxiv_id": "2605.20897",
                "arxiv_unversioned_id": "2605.20897",
            },
            {
                "ref_id": "R02",
                "url": "https://arxiv.org/abs/2605.20897",
                "canonical_url": "https://arxiv.org/abs/2605.20897",
                "source_kind": "arxiv_abs_url",
                "normalized_identity": "arxiv:2605.20897",
                "arxiv_id": "2605.20897",
                "arxiv_unversioned_id": "2605.20897",
            },
            {
                "ref_id": "R03",
                "url": "https://developer.nvidia.com/blog/example/?linkId=123",
                "canonical_url": "https://developer.nvidia.com/blog/example/",
                "source_kind": "company_blog_url",
                "normalized_identity": "company_blog:nvidia:example",
            },
        ]
    }


def _artifact_files(root: Path) -> dict[str, Path]:
    sources = root / "sources"
    sources.mkdir()
    pdf = sources / "R01.pdf"
    pdf.write_bytes(b"%PDF-1.4\nfixture bytes are local only\n")
    arxiv_html = sources / "R02.html"
    arxiv_html.write_text(
        """
        <html><head>
          <title>Fallback title</title>
          <meta name="citation_title" content="Metadata Adapter Paper">
          <meta name="citation_author" content="Ada Lovelace">
          <meta name="citation_author" content="Grace Hopper">
          <meta name="citation_date" content="2026-05-20">
          <meta name="citation_arxiv_id" content="2605.20897">
          <meta name="citation_pdf_url" content="https://arxiv.org/pdf/2605.20897.pdf">
        </head><body>body must not be serialized</body></html>
        """,
        encoding="utf-8",
    )
    blog_html = sources / "R03.html"
    blog_html.write_text(
        """
        <html><head>
          <meta property="og:title" content="NVIDIA Blog Metadata">
          <meta name="author" content="NVIDIA">
          <meta property="article:published_time" content="2026-05-21T00:00:00Z">
        </head><body>blog body must not be serialized</body></html>
        """,
        encoding="utf-8",
    )
    return {"R01": pdf, "R02": arxiv_html, "R03": blog_html}


def _acquisition_rows(paths: dict[str, Path], root: Path) -> list[dict[str, object]]:
    rows = []
    kinds = {"R01": "arxiv_pdf_url", "R02": "arxiv_abs_url", "R03": "company_blog_url"}
    identities = {
        "R01": "arxiv:2605.20897",
        "R02": "arxiv:2605.20897",
        "R03": "company_blog:nvidia:example",
    }
    urls = {
        "R01": "https://arxiv.org/pdf/2605.20897.pdf",
        "R02": "https://arxiv.org/abs/2605.20897",
        "R03": "https://developer.nvidia.com/blog/example/?linkId=123",
    }
    canonicals = {
        "R01": "https://arxiv.org/abs/2605.20897",
        "R02": "https://arxiv.org/abs/2605.20897",
        "R03": "https://developer.nvidia.com/blog/example/",
    }
    for ref_id, artifact_path in paths.items():
        rows.append(
            {
                "ref_id": ref_id,
                "url": urls[ref_id],
                "canonical_url": canonicals[ref_id],
                "source_kind": kinds[ref_id],
                "normalized_identity": identities[ref_id],
                "artifact_path": str(artifact_path.relative_to(root)),
                "content_type": "application/pdf"
                if artifact_path.suffix == ".pdf"
                else "text/html; charset=utf-8",
                "byte_count": artifact_path.stat().st_size,
                "sha256": _sha256(artifact_path),
                "http_status": 200,
                "status": "captured",
                "terminal": True,
                "failure_code": None,
                "graph_write_attempted": False,
                "kg_readiness_claimed": False,
                "production_persistence_attempted": False,
            }
        )
    # pyrefly: ignore [bad-return]
    return rows  # ty:ignore[invalid-return-type]


def _expanded_refs() -> list[dict[str, object]]:
    ref_specs = [
        ("R01", "arxiv_pdf_url", "arxiv:2605.20897"),
        ("R02", "arxiv_abs_url", "arxiv:2605.11111"),
        ("R03", "arxiv_pdf_url", "arxiv:2605.11112"),
        ("R04", "arxiv_abs_url", "arxiv:2605.11113"),
        ("R05", "arxiv_pdf_url", "arxiv:2605.11114"),
        ("R06", "arxiv_abs_url", "arxiv:2605.11115"),
        ("R07", "arxiv_pdf_url", "arxiv:2605.11116"),
        ("R08", "arxiv_abs_url", "arxiv:2605.11117"),
        ("R09", "arxiv_abs_url", "arxiv:2605.11118"),
        ("R10", "arxiv_abs_url", "arxiv:2605.20897"),
        ("R11", "arxiv_abs_url", "arxiv:2605.11119"),
        ("R12", "arxiv_abs_url", "arxiv:2605.11120"),
        ("R13", "nature_article_url", "nature:10.1038/example"),
        ("R14", "company_blog_url", "company_blog:nvidia:example"),
        ("R15", "arxiv_abs_url", "arxiv:2605.23904"),
        ("R16", "arxiv_abs_url", "arxiv:2605.22502"),
        ("R17", "arxiv_abs_url", "arxiv:2605.28655"),
        ("R18", "arxiv_abs_url", "arxiv:2605.26099"),
        ("R19", "arxiv_abs_url", "arxiv:2605.22166"),
        ("R20", "arxiv_abs_url", "arxiv:2605.22681"),
        ("R21", "arxiv_abs_url", "arxiv:2605.26302"),
    ]
    refs: list[dict[str, object]] = []
    for ref_id, source_kind, identity in ref_specs:
        ref: dict[str, object] = {
            "ref_id": ref_id,
            "source_kind": source_kind,
            "normalized_identity": identity,
        }
        if source_kind.startswith("arxiv_"):
            arxiv_id = identity.removeprefix("arxiv:")
            variant = "pdf" if source_kind == "arxiv_pdf_url" else "abs"
            suffix = ".pdf" if variant == "pdf" else ""
            ref.update(
                {
                    "url": f"https://arxiv.org/{variant}/{arxiv_id}{suffix}",
                    "canonical_url": f"https://arxiv.org/abs/{arxiv_id}",
                    "arxiv_id": arxiv_id,
                    "arxiv_unversioned_id": arxiv_id,
                }
            )
        elif source_kind == "company_blog_url":
            ref.update(
                {
                    "url": "https://developer.nvidia.com/blog/example/?linkId=fixture",
                    "canonical_url": "https://developer.nvidia.com/blog/example/",
                }
            )
        else:
            ref.update(
                {
                    "url": "https://www.nature.com/articles/example",
                    "canonical_url": "https://www.nature.com/articles/example",
                }
            )
        refs.append(ref)
    return refs


def _expanded_selection() -> dict[str, object]:
    return {"schema_version": "m028.source-selection.v1", "refs": _expanded_refs()}


def _expanded_artifacts(root: Path, refs: list[dict[str, object]]) -> dict[str, Path]:
    sources = root / "sources"
    sources.mkdir()
    paths: dict[str, Path] = {}
    for ref in refs:
        ref_id = str(ref["ref_id"])
        source_kind = str(ref["source_kind"])
        if source_kind == "arxiv_pdf_url":
            path = sources / f"{ref_id}.pdf"
            path.write_bytes(f"%PDF-1.4\nlocal fixture for {ref_id}\n".encode())
        else:
            path = sources / f"{ref_id}.html"
            title = f"Fixture title {ref_id}"
            path.write_text(
                f"""
                <html><head>
                  <meta name="citation_title" content="{title}">
                  <meta name="citation_author" content="Fixture Author">
                  <meta name="citation_date" content="2026-05-20">
                  <meta name="citation_arxiv_id" content="{str(ref.get("arxiv_id", "")).removeprefix("arxiv:")}">
                </head><body>source body for {ref_id} must not be serialized</body></html>
                """,
                encoding="utf-8",
            )
        paths[ref_id] = path
    return paths


def _expanded_acquisition_rows(
    refs: list[dict[str, object]], paths: dict[str, Path], root: Path
) -> list[dict[str, object]]:
    rows: list[dict[str, object]] = []
    for ref in refs:
        ref_id = str(ref["ref_id"])
        artifact_path = paths[ref_id]
        rows.append(
            {
                "ref_id": ref_id,
                "url": ref["url"],
                "canonical_url": ref["canonical_url"],
                "source_kind": ref["source_kind"],
                "normalized_identity": ref["normalized_identity"],
                "artifact_path": str(artifact_path.relative_to(root)),
                "content_type": "application/pdf"
                if artifact_path.suffix == ".pdf"
                else "text/html; charset=utf-8",
                "byte_count": artifact_path.stat().st_size,
                "sha256": _sha256(artifact_path),
                "http_status": 200,
                "status": "captured",
                "terminal": True,
                "failure_code": None,
                "graph_write_attempted": False,
                "kg_readiness_claimed": False,
                "production_persistence_attempted": False,
            }
        )
    return rows


@pytest.fixture()
def expanded_outputs(tmp_path: Path) -> dict[str, object]:
    builder = _load_build_script()
    selection = _expanded_selection()
    # pyrefly: ignore [bad-argument-type]
    refs = list(selection["refs"])  # ty:ignore[invalid-argument-type]
    paths = _expanded_artifacts(tmp_path, refs)  # ty:ignore[invalid-argument-type]
    acquisition_events = _expanded_acquisition_rows(refs, paths, tmp_path)  # ty:ignore[invalid-argument-type]
    selection_path = tmp_path / "selection.json"
    acquisition_path = tmp_path / "source-acquisition-events.jsonl"
    out_dir = tmp_path / "out"
    _write_json(selection_path, selection)
    _write_jsonl(acquisition_path, acquisition_events)

    metadata_events, summary = builder.build_metadata_outputs(
        selection_path, acquisition_path, out_dir, repo_root=tmp_path
    )

    return {
        "selection": selection,
        "acquisition_events": acquisition_events,
        "metadata_events": metadata_events,
        "summary": summary,
        "repo_root": tmp_path,
        "out_dir": out_dir,
    }


def _diagnostic_codes(outputs: dict[str, object], *, reject_unsafe_claims: bool = True) -> set[str]:
    verifier = _load_verify_script()
    diagnostics = verifier.verify_contract(
        selection=outputs["selection"],
        acquisition_events=outputs["acquisition_events"],
        metadata_events=outputs["metadata_events"],
        summary=outputs["summary"],
        repo_root=outputs["repo_root"],
        reject_unsafe_claims=reject_unsafe_claims,
    )
    return {item["code"] for item in diagnostics}


def test_build_outputs_preserve_refs_identities_and_metadata_only_payloads(tmp_path: Path) -> None:
    script = _load_build_script()
    selection_path = tmp_path / "selection.json"
    acquisition_path = tmp_path / "source-acquisition-events.jsonl"
    out_dir = tmp_path / "out"
    paths = _artifact_files(tmp_path)
    _write_json(selection_path, _selection())
    _write_jsonl(acquisition_path, _acquisition_rows(paths, tmp_path))

    events, summary = script.build_metadata_outputs(
        selection_path, acquisition_path, out_dir, repo_root=tmp_path
    )

    assert summary["url_ref_count"] == 3
    assert summary["normalized_identity_count"] == 2
    assert summary["duplicate_identity_group_count"] == 1
    assert summary["source_kind_counts"] == {
        "arxiv_abs_url": 1,
        "arxiv_pdf_url": 1,
        "company_blog_url": 1,
    }
    assert all(flag is False for flag in summary["safety_flags"].values())
    assert [event["ref_id"] for event in events] == ["R01", "R02", "R03"]
    assert events[0]["url_variant"] == "pdf_url"
    assert events[1]["identity_group"]["ref_ids"] == ["R01", "R02"]
    assert events[1]["optional_metadata"]["title"]["value"] == "Metadata Adapter Paper"
    assert events[2]["source_family"] == "company_blog"
    serialized = (out_dir / "source-metadata-summary.json").read_text() + (
        out_dir / "source-metadata-events.jsonl"
    ).read_text()
    for forbidden in [
        "<html",
        "</html>",
        "%PDF-",
        "raw_text",
        "chunk_text",
        "trusted_fact",
        "body must not be serialized",
    ]:
        assert forbidden.lower() not in serialized.lower()


def test_missing_acquisition_event_is_blocked_not_silent(tmp_path: Path) -> None:
    script = _load_build_script()
    selection_path = tmp_path / "selection.json"
    acquisition_path = tmp_path / "source-acquisition-events.jsonl"
    paths = _artifact_files(tmp_path)
    _write_json(selection_path, _selection())
    _write_jsonl(acquisition_path, _acquisition_rows(paths, tmp_path)[:2])

    events, summary = script.build_metadata_outputs(
        selection_path, acquisition_path, tmp_path / "out", repo_root=tmp_path
    )

    blocked = events[-1]
    assert blocked["ref_id"] == "R03"
    assert blocked["metadata_status"] == "blocked"
    assert blocked["acquisition"]["failure_code"] == "missing_acquisition_event"
    assert summary["diagnostic_counts"]["missing_acquisition_event"] == 1


def test_checksum_mismatch_records_diagnostic(tmp_path: Path) -> None:
    script = _load_build_script()
    selection_path = tmp_path / "selection.json"
    acquisition_path = tmp_path / "source-acquisition-events.jsonl"
    paths = _artifact_files(tmp_path)
    rows = _acquisition_rows(paths, tmp_path)
    rows[1]["sha256"] = "0" * 64
    _write_json(selection_path, _selection())
    _write_jsonl(acquisition_path, rows)

    events, summary = script.build_metadata_outputs(
        selection_path, acquisition_path, tmp_path / "out", repo_root=tmp_path
    )

    arxiv_abs = events[1]
    assert arxiv_abs["metadata_status"] == "metadata_available_with_diagnostics"
    assert any(item["code"] == "artifact_checksum_mismatch" for item in arxiv_abs["diagnostics"])
    assert summary["diagnostic_counts"]["artifact_checksum_mismatch"] == 1


def test_rejects_malformed_selection(tmp_path: Path) -> None:
    script = _load_build_script()
    selection_path = tmp_path / "selection.json"
    acquisition_path = tmp_path / "source-acquisition-events.jsonl"
    _write_json(selection_path, {"refs": [{"ref_id": "R01"}]})
    _write_jsonl(acquisition_path, [])

    with pytest.raises(script.AdapterInputError, match="selection_ref_required_fields"):
        script.build_metadata_outputs(
            selection_path, acquisition_path, tmp_path / "out", repo_root=tmp_path
        )


def test_verifier_accepts_expanded_fixture_outputs(expanded_outputs: dict[str, object]) -> None:
    assert _diagnostic_codes(expanded_outputs) == set()


def test_verifier_accepts_real_expanded_artifacts() -> None:
    verifier = _load_verify_script()
    diagnostics = verifier.verify_contract(
        selection=_read_json(REAL_CORPUS_DIR / "selection.json"),
        acquisition_events=_read_jsonl(REAL_CORPUS_DIR / "source-acquisition-events.jsonl"),
        metadata_events=_read_jsonl(REAL_CORPUS_DIR / "source-metadata-events.jsonl"),
        summary=_read_json(REAL_CORPUS_DIR / "source-metadata-summary.json"),
        repo_root=REPO_ROOT,
        reject_unsafe_claims=True,
    )

    assert diagnostics == []


def test_verifier_rejects_stale_fourteen_ref_assumptions(
    expanded_outputs: dict[str, object],
) -> None:
    outputs = deepcopy(expanded_outputs)
    keep_ref_ids = {f"R{index:02d}" for index in range(1, 15)}
    # pyrefly: ignore [unsupported-operation]
    outputs["selection"]["refs"] = [  # ty:ignore[invalid-assignment]
        ref
        # pyrefly: ignore [bad-index]
        for ref in outputs["selection"]["refs"]
        if ref["ref_id"] in keep_ref_ids  # pyrefly: ignore[bad-assignment]
    ]
    outputs["acquisition_events"] = [
        # pyrefly: ignore [not-iterable]
        row
        # pyrefly: ignore [not-iterable]
        for row in outputs["acquisition_events"]  # ty:ignore[not-iterable]
        if row["ref_id"] in keep_ref_ids
    ]
    outputs["metadata_events"] = [
        # pyrefly: ignore [not-iterable]
        event
        # pyrefly: ignore [not-iterable]
        for event in outputs["metadata_events"]  # ty:ignore[not-iterable]
        if event["ref_id"] in keep_ref_ids
    ]
    # pyrefly: ignore [unsupported-operation]
    outputs["summary"]["url_ref_count"] = 14  # ty:ignore[invalid-assignment]
    # pyrefly: ignore [unsupported-operation]
    outputs["summary"]["ref_count"] = 14  # ty:ignore[invalid-assignment]
    # pyrefly: ignore [unsupported-operation]
    outputs["summary"]["ref_ids"] = [event["ref_id"] for event in outputs["metadata_events"]]  # ty:ignore[invalid-assignment]

    codes = _diagnostic_codes(outputs)

    assert "corpus_scope_stale" in codes
    assert "missing_new_refs" in codes


def test_verifier_rejects_missing_new_refs(expanded_outputs: dict[str, object]) -> None:
    outputs = deepcopy(expanded_outputs)
    # pyrefly: ignore [unsupported-operation]
    outputs["selection"]["refs"] = [  # ty:ignore[invalid-assignment]
        ref
        # pyrefly: ignore [bad-index]
        for ref in outputs["selection"]["refs"]
        if ref["ref_id"] != "R21"  # pyrefly: ignore[bad-assignment]
    ]
    outputs["metadata_events"] = [
        # pyrefly: ignore [not-iterable]
        event
        # pyrefly: ignore [not-iterable]
        for event in outputs["metadata_events"]  # ty:ignore[not-iterable]
        if event["ref_id"] != "R21"
    ]
    outputs["acquisition_events"] = [
        # pyrefly: ignore [not-iterable]
        row
        # pyrefly: ignore [not-iterable]
        for row in outputs["acquisition_events"]  # ty:ignore[not-iterable]
        if row["ref_id"] != "R21"
    ]
    # pyrefly: ignore [unsupported-operation]
    outputs["summary"]["ref_ids"] = [event["ref_id"] for event in outputs["metadata_events"]]  # ty:ignore[invalid-assignment]

    codes = _diagnostic_codes(outputs)

    assert "missing_new_refs" in codes
    assert "corpus_scope_stale" in codes


def test_verifier_rejects_unsafe_readiness_flags(expanded_outputs: dict[str, object]) -> None:
    outputs = deepcopy(expanded_outputs)
    outputs["summary"]["safety_flags"]["kg_readiness_claimed"] = True  # pyrefly: ignore [bad-assignment, bad-index]  # ty:ignore[not-subscriptable]
    outputs["summary"]["unsafe_claim_counts"]["parser_readiness_claimed"] = 1  # pyrefly: ignore [bad-assignment, bad-index]  # ty:ignore[not-subscriptable]
    outputs["metadata_events"][0]["safety_flags"]["graph_write_attempted"] = True  # pyrefly: ignore [bad-assignment, bad-index]  # ty:ignore[not-subscriptable]

    assert "unsafe_claim_detected" in _diagnostic_codes(outputs)


def test_verifier_rejects_source_kind_drift(expanded_outputs: dict[str, object]) -> None:
    outputs = deepcopy(expanded_outputs)
    outputs["metadata_events"][0]["source_kind"] = "company_blog_url"  # pyrefly: ignore [bad-assignment, bad-index]  # ty:ignore[not-subscriptable]

    codes = _diagnostic_codes(outputs)

    assert "source_kind_drift" in codes
    assert "selection_metadata_mismatch" in codes


def test_verifier_rejects_broken_acquisition_references(
    expanded_outputs: dict[str, object],
) -> None:
    outputs = deepcopy(expanded_outputs)
    outputs["metadata_events"][0]["artifact"]["path"] = "sources/missing.pdf"  # pyrefly: ignore [bad-assignment, bad-index]  # ty:ignore[not-subscriptable]

    assert "artifact_reference_broken" in _diagnostic_codes(outputs)


def test_verifier_rejects_raw_payload_leakage(expanded_outputs: dict[str, object]) -> None:
    outputs = deepcopy(expanded_outputs)
    outputs["metadata_events"][0]["raw_text"] = "<html><body>raw source payload</body></html>"  # pyrefly: ignore [bad-assignment, bad-index]  # ty:ignore[not-subscriptable]

    assert "raw_payload_leakage" in _diagnostic_codes(outputs)


def test_verifier_requires_nullable_optional_gap_diagnostics(
    expanded_outputs: dict[str, object],
) -> None:
    outputs = deepcopy(expanded_outputs)
    first_event = outputs["metadata_events"][0]  # pyrefly: ignore [bad-assignment, bad-index]  # ty:ignore[not-subscriptable]
    first_event["optional_metadata"].pop("doi")

    assert "required_nullable_field_missing" in _diagnostic_codes(outputs)
