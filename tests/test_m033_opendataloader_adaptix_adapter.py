from __future__ import annotations

import json
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))

from probe_m033_opendataloader_adaptix_adapter import load_odl_document, run_probe
from verify_m033_opendataloader_adaptix_adapter import verify


def _write_json(path: Path, payload: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2), encoding="utf-8")


def _sample_odl_json() -> dict:
    return {
        "file name": "sample.pdf",
        "number of pages": 2,
        "author": None,
        "title": "Sample",
        "creation date": None,
        "modification date": None,
        "kids": [
            {
                "type": "heading",
                "id": 1,
                "page number": 1,
                "bounding box": [1.0, 2.0, 3.0, 4.0],
                "content": "Introduction",
                "heading level": 1,
            },
            {
                "type": "table",
                "id": 2,
                "page number": 2,
                "bounding box": [5.0, 6.0, 7.0, 8.0],
                "rows": [],
            },
        ],
    }


def test_adaptix_maps_space_named_fields_to_typed_model() -> None:
    doc = load_odl_document(_sample_odl_json())

    assert doc.file_name == "sample.pdf"
    assert doc.number_of_pages == 2
    assert doc.kids[0].page_number == 1
    assert doc.kids[0].bounding_box == (1.0, 2.0, 3.0, 4.0)
    assert doc.kids[0].extra["heading level"] == 1


def test_probe_writes_candidate_only_summary_and_false_safety_flags(tmp_path: Path) -> None:
    probe_root = tmp_path / "probe"
    adapter_dir = tmp_path / "adapter"
    _write_json(probe_root / "per-paper" / "paper-a" / "hybrid" / "original.json", _sample_odl_json())

    assert run_probe(probe_root, adapter_dir) == 0
    summary = json.loads((adapter_dir / "adaptix-adapter-summary.json").read_text(encoding="utf-8"))

    assert summary["status"] == "adaptix-adapter-candidate"
    assert summary["paper_count"] == 1
    result = summary["results"][0]
    assert result["status"] == "mapped_candidate_only"
    assert result["candidate_summary"]["source_ref_candidate"]["candidate_only"] is True
    assert all(value is False for value in summary["safety_flags"].values())
    assert all(value is False for value in result["safety_flags"].values())


def test_probe_fails_closed_on_malformed_document(tmp_path: Path) -> None:
    probe_root = tmp_path / "probe"
    adapter_dir = tmp_path / "adapter"
    _write_json(
        probe_root / "per-paper" / "paper-b" / "hybrid" / "original.json",
        {"file name": "broken.pdf", "kids": [{"page number": "bad"}]},
    )

    assert run_probe(probe_root, adapter_dir) == 1
    summary = json.loads((adapter_dir / "adaptix-adapter-summary.json").read_text(encoding="utf-8"))
    diagnostics = [
        json.loads(line)
        for line in (adapter_dir / "adaptix-adapter-diagnostics.jsonl").read_text(encoding="utf-8").splitlines()
    ]

    assert summary["status"] == "needs-attention"
    assert summary["error_count"] == 1
    assert diagnostics[0]["severity"] == "error"
    assert diagnostics[0]["code"] == "adaptix_mapping_failed"
    assert all(value is False for value in diagnostics[0]["safety_flags"].values())


def test_verifier_accepts_valid_probe_artifacts(tmp_path: Path) -> None:
    probe_root = tmp_path / "probe"
    adapter_dir = tmp_path / "adapter"
    _write_json(probe_root / "per-paper" / "paper-c" / "hybrid" / "original.json", _sample_odl_json())
    assert run_probe(probe_root, adapter_dir) == 0

    assert verify(probe_root, adapter_dir) == 0
    closeout = json.loads(
        (adapter_dir / "adaptix-adapter-closeout-summary.json").read_text(encoding="utf-8")
    )
    assert closeout["status"] == "passed"
    assert closeout["failure_count"] == 0


def test_verifier_rejects_permissive_import_flag(tmp_path: Path) -> None:
    probe_root = tmp_path / "probe"
    adapter_dir = tmp_path / "adapter"
    _write_json(probe_root / "per-paper" / "paper-d" / "hybrid" / "original.json", _sample_odl_json())
    assert run_probe(probe_root, adapter_dir) == 0
    summary_path = adapter_dir / "adaptix-adapter-summary.json"
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    summary["safety_flags"]["graph_import_allowed"] = True
    summary_path.write_text(json.dumps(summary, indent=2), encoding="utf-8")

    assert verify(probe_root, adapter_dir) == 1
    closeout = json.loads(
        (adapter_dir / "adaptix-adapter-closeout-summary.json").read_text(encoding="utf-8")
    )
    assert closeout["status"] == "failed"
    assert any(failure["code"] == "unsafe_flag" for failure in closeout["failures"])


def test_probe_requires_existing_json_outputs(tmp_path: Path) -> None:
    with pytest.raises(FileNotFoundError):
        run_probe(tmp_path / "missing", tmp_path / "adapter")
