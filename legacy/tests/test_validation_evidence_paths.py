from __future__ import annotations

from pathlib import Path

import pytest

from research_graph.application.validation.evidence_paths import (
    ValidationEvidencePathError,
    json_path,
    repo_relative_path,
    safe_output_path,
)


def test_json_path_formats_keys_and_indexes() -> None:
    assert json_path("$.rows", 0) == "$.rows[0]"
    assert json_path("$", "safety_flags") == "$.safety_flags"


def test_repo_relative_path_accepts_only_paths_under_root(tmp_path: Path) -> None:
    existing = tmp_path / "inputs" / "evidence.json"
    existing.parent.mkdir(parents=True)
    existing.write_text("{}", encoding="utf-8")

    assert repo_relative_path(
        "inputs/evidence.json", repo_root=tmp_path, label="input"
    ) == existing.resolve()

    for unsafe in (
        "",
        " inputs/evidence.json",
        "../evidence.json",
        "/tmp/evidence.json",
        "https://example.test/evidence.json",
    ):
        with pytest.raises(ValidationEvidencePathError):
            repo_relative_path(unsafe, repo_root=tmp_path, label="input")


def test_safe_output_path_stays_under_configured_output_dir(tmp_path: Path) -> None:
    output_dir = Path("data/validation-remediation")

    assert safe_output_path(
        output_dir / "evidence.json",
        repo_root=tmp_path,
        label="output",
        output_dir=output_dir,
    ) == (tmp_path / output_dir / "evidence.json").resolve()

    with pytest.raises(ValidationEvidencePathError):
        safe_output_path(
            "tmp/evidence.json",
            repo_root=tmp_path,
            label="output",
            output_dir=output_dir,
        )
