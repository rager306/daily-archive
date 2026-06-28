from __future__ import annotations

import json
import os
from pathlib import Path

import pytest

from research_graph.application.corpus.manifest_io import write_manifest_json_atomic


def test_write_manifest_json_atomic_writes_json_object(tmp_path: Path) -> None:
    output = tmp_path / "manifest.json"

    write_manifest_json_atomic(output, {"schema_version": "test.v1", "items": [1, 2]})

    assert json.loads(output.read_text(encoding="utf-8")) == {
        "schema_version": "test.v1",
        "items": [1, 2],
    }
    assert output.read_text(encoding="utf-8").endswith("\n")


def test_write_manifest_json_atomic_preserves_existing_file_when_replace_fails(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    output = tmp_path / "manifest.json"
    output.write_text('{"old": true}\n', encoding="utf-8")

    def fail_replace(source: str, target: Path) -> None:
        raise OSError(f"blocked replace: {source} -> {target}")

    monkeypatch.setattr(os, "replace", fail_replace)

    with pytest.raises(OSError, match="blocked replace"):
        write_manifest_json_atomic(output, {"new": True})

    assert output.read_text(encoding="utf-8") == '{"old": true}\n'
    assert list(tmp_path.glob(".manifest.json.*.tmp")) == []


def test_write_manifest_json_atomic_rejects_non_object_payload(tmp_path: Path) -> None:
    output = tmp_path / "manifest.json"

    with pytest.raises(TypeError, match="JSON object"):
        write_manifest_json_atomic(output, ["not", "an", "object"])  # type: ignore[arg-type]

    assert not output.exists()
