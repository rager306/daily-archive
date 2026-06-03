import json
from pathlib import Path

import pytest

from scripts.replay_m028_smoke_closeout import CloseoutError, read_jsonl, validate_source_acquisition


def _write_minimal_corpus(base: Path, *, artifact_path: str, unsafe_event: bool = False) -> None:
    base.mkdir(parents=True)
    source = base / "sources" / "R01.txt"
    source.parent.mkdir()
    source.write_text("metadata fixture only\n", encoding="utf-8")
    if artifact_path == "data/corpus/sources/R01.txt":
        artifact = base.parent.parent / "corpus" / "sources" / "R01.txt"
        artifact.parent.mkdir(parents=True)
        artifact.write_text("metadata fixture only\n", encoding="utf-8")
        byte_count = artifact.stat().st_size
        import hashlib

        sha256 = hashlib.sha256(artifact.read_bytes()).hexdigest()
    else:
        byte_count = source.stat().st_size
        sha256 = "0" * 64

    refs = []
    for index in range(1, 22):
        ref_id = f"R{index:02d}"
        normalized_identity = "arxiv:2605.20897" if ref_id in {"R01", "R10"} else f"arxiv:2605.{20000 + index}"
        refs.append(
            {
                "ref_id": ref_id,
                "url": f"https://arxiv.org/abs/2605.{20000 + index}",
                "canonical_url": f"https://arxiv.org/abs/2605.{20000 + index}",
                "source_kind": "arxiv_abs_url",
                "normalized_identity": normalized_identity,
            }
        )
    (base / "selection.json").write_text(json.dumps({"refs": refs, "safety_flags": {"graph_write_attempted": False}}), encoding="utf-8")

    events = []
    for ref in refs:
        events.append(
            {
                "ref_id": ref["ref_id"],
                "status": "captured",
                "terminal": True,
                "artifact_path": artifact_path,
                "byte_count": byte_count,
                "sha256": sha256,
                "graph_write_attempted": unsafe_event if ref["ref_id"] == "R01" else False,
                "kg_readiness_claimed": False,
                "production_persistence_attempted": False,
            }
        )
    (base / "source-acquisition-events.jsonl").write_text("\n".join(json.dumps(event) for event in events) + "\n", encoding="utf-8")
    (base / "source-acquisition-summary.json").write_text(json.dumps({"safety_flags": {"graph_write_attempted": False}}), encoding="utf-8")
    (base / "acquisition-report.md").write_text("# Metadata-only acquisition report\n", encoding="utf-8")


def test_read_jsonl_rejects_malformed_row(tmp_path):
    path = tmp_path / "events.jsonl"
    path.write_text('{"ok": true}\n{bad json}\n', encoding="utf-8")

    with pytest.raises(CloseoutError) as exc_info:
        read_jsonl(path)

    assert exc_info.value.diagnostics[0].code == "JSONL_MALFORMED"


def test_validate_source_acquisition_rejects_missing_artifact(tmp_path):
    root = tmp_path
    corpus = root / "data" / "article_corpora" / "m028-universal-loader-runtime-smoke-v1"
    _write_minimal_corpus(corpus, artifact_path="data/article_corpora/m028-universal-loader-runtime-smoke-v1/sources/missing.txt")

    preflight, diagnostics = validate_source_acquisition(corpus, root)

    assert preflight["status"] == "fail"
    assert any(diagnostic.code == "ARTIFACT_PATH_MISSING_ON_DISK" for diagnostic in diagnostics)


def test_validate_source_acquisition_rejects_unsafe_flag(tmp_path):
    root = tmp_path
    corpus = root / "data" / "article_corpora" / "m028-universal-loader-runtime-smoke-v1"
    artifact_rel = "data/corpus/sources/R01.txt"
    _write_minimal_corpus(corpus, artifact_path=artifact_rel, unsafe_event=True)

    preflight, diagnostics = validate_source_acquisition(corpus, root)

    assert preflight["status"] == "fail"
    assert any(diagnostic.code == "UNSAFE_FLAG_TRUE" for diagnostic in diagnostics)
