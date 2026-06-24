from __future__ import annotations

import json
from pathlib import Path

import pytest

from research_graph.infrastructure.corpus.sources.markdown_converter import ConversionResult
from research_graph.infrastructure.corpus.sources.thirty_paper_source_scan import (
    AcquisitionPaths,
    acquire_sources_for_manifest,
    acquire_sources_for_manifest_sync,
    missing_markdown_paper_ids,
)


class FakeConverter:
    def __init__(self, results: dict[str, ConversionResult]) -> None:
        self.results = results
        self.closed = False
        self.calls: list[str] = []

    async def convert(self, arxiv_id: str) -> ConversionResult:
        self.calls.append(arxiv_id)
        return self.results[arxiv_id]

    async def close(self) -> None:
        self.closed = True


def _manifest(tmp_path: Path) -> Path:
    manifest = {
        "m005_overlap_count": 1,
        "expansion_count": 2,
        "papers": [
            {
                "rank": 1,
                "paper_id": "2605.14259v1",
                "selection_role": "m005_baseline_overlap",
                "availability": {"available_markdown": True},
            },
            {
                "rank": 2,
                "paper_id": "2001.00116v2",
                "selection_role": "deterministic_expansion",
                "availability": {"available_markdown": False},
            },
            {
                "rank": 3,
                "paper_id": "2001.00119v2",
                "selection_role": "deterministic_expansion",
                "availability": {"available_markdown": False},
            },
        ],
    }
    path = tmp_path / "manifest.json"
    path.write_text(json.dumps(manifest), encoding="utf-8")
    return path


def test_missing_markdown_paper_ids_reads_manifest_flags(tmp_path: Path) -> None:
    manifest = json.loads(_manifest(tmp_path).read_text(encoding="utf-8"))

    assert missing_markdown_paper_ids(manifest) == ["2001.00116v2", "2001.00119v2"]


@pytest.mark.asyncio
async def test_acquire_sources_for_manifest_sync_fails_inside_running_loop() -> None:
    with pytest.raises(RuntimeError, match="await acquire_sources_for_manifest"):
        acquire_sources_for_manifest_sync()


async def test_acquire_sources_for_manifest_writes_redacted_summary_and_diagnostics(
    tmp_path: Path,
) -> None:
    manifest = _manifest(tmp_path)
    research = tmp_path / "papers"
    cache = tmp_path / "cache"
    (research / "2605.14259v1").mkdir(parents=True)
    (research / "2605.14259v1" / "full_text.md").write_text("# Existing\n\nBody", encoding="utf-8")
    (research / "2001.00116v2").mkdir(parents=True)
    (research / "2001.00116v2" / "paper.json").write_text("{}", encoding="utf-8")
    (research / "2001.00119v2").mkdir(parents=True)
    (research / "2001.00119v2" / "paper.json").write_text("{}", encoding="utf-8")
    converter = FakeConverter(
        {
            "2001.00116v2": ConversionResult(
                markdown="# Converted\n\nSubstantive body", method="arxiv2md", error=None
            ),
            "2001.00119v2": ConversionResult(
                markdown=None,
                method="docling",
                error="network token raw text should not be present",
            ),
        }
    )

    paths = await acquire_sources_for_manifest(
        manifest_path=manifest,
        output_dir=tmp_path / "out",
        paths=AcquisitionPaths(research_papers_dir=research, arxiv_cache_dir=cache),
        converter=converter,
    )

    summary = json.loads(paths["summary_path"].read_text(encoding="utf-8"))
    diagnostics = [
        json.loads(line)
        for line in paths["diagnostics_path"].read_text(encoding="utf-8").splitlines()
    ]

    assert converter.calls == ["2001.00116v2", "2001.00119v2"]
    assert converter.closed is False
    assert summary["paper_count"] == 3
    assert summary["attempted_missing_markdown_count"] == 2
    assert summary["originally_missing_markdown_count"] == 2
    assert summary["preexisting_markdown_ready_from_original_missing_count"] == 0
    assert summary["acquired_markdown_count"] == 1
    assert summary["ready_for_markdown_scan_count"] == 2
    assert summary["still_missing_markdown_count"] == 1
    assert summary["raw_text_included"] is False
    assert summary["production_import_attempted"] is False
    assert (research / "2001.00116v2" / "full_text.md").exists()
    assert len(diagnostics) == 3
    assert all("Substantive body" not in json.dumps(record) for record in diagnostics)
    assert diagnostics[1]["outcome"] == "acquired_markdown"
    assert diagnostics[2]["outcome"] == "conversion_failed"
    assert diagnostics[2]["conversion_error"] == "network token raw text should not be present"


async def test_acquire_sources_for_manifest_rejects_low_quality_markdown(tmp_path: Path) -> None:
    manifest = _manifest(tmp_path)
    research = tmp_path / "papers"
    cache = tmp_path / "cache"
    (research / "2605.14259v1").mkdir(parents=True)
    (research / "2605.14259v1" / "full_text.md").write_text("# Existing\n\nBody", encoding="utf-8")
    (research / "2001.00116v2").mkdir(parents=True)
    (research / "2001.00119v2").mkdir(parents=True)
    converter = FakeConverter(
        {
            "2001.00116v2": ConversionResult(
                markdown="# Only headings", method="arxiv2md", error=None
            ),
            "2001.00119v2": ConversionResult(
                markdown="# Converted\n\nBody", method="docling", error=None
            ),
        }
    )

    paths = await acquire_sources_for_manifest(
        manifest_path=manifest,
        output_dir=tmp_path / "out",
        paths=AcquisitionPaths(research_papers_dir=research, arxiv_cache_dir=cache),
        converter=converter,
    )

    summary = json.loads(paths["summary_path"].read_text(encoding="utf-8"))
    diagnostics = [
        json.loads(line)
        for line in paths["diagnostics_path"].read_text(encoding="utf-8").splitlines()
    ]

    assert summary["acquired_markdown_count"] == 1
    assert diagnostics[1]["outcome"] == "conversion_failed"
    assert diagnostics[1]["conversion_error"] == "low_quality_markdown:no_substantive_body"
    assert not (research / "2001.00116v2" / "full_text.md").exists()
