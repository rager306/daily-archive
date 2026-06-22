from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

from jsonschema import Draft7Validator, validate

ROOT = Path(__file__).resolve().parents[1]

SCHEMA_PATHS = [
    ROOT / "schemas/daily-archive.pdf-batch-manifest.v1.json",
    ROOT / "schemas/daily-archive.parser-op.v1.json",
    ROOT / "schemas/grobid-tei.v1.json",
    ROOT / "schemas/opendataloader-pdf.v1.json",
    ROOT / "schemas/m057-fd-table-similarity.v1.json",
    ROOT / "schemas/m058-plotextractor-figure-caption.v1.json",
]

MANIFEST_COUNTS = {
    ROOT / "artifacts/m054-pdf-acquisition/manifest.json": 5,
    ROOT / "artifacts/m055-parser-benchmark/manifest.json": 5,
    ROOT / "artifacts/m055deep-parser-benchmark/manifest.json": 20,
    ROOT / "artifacts/m056-bfs-graph/manifest.json": 166,
    ROOT / "artifacts/m057-fd-marker/manifest.json": 166,
    ROOT / "artifacts/m058-plotextractor/manifest.json": 5,
}

SAFETY_DEFAULTS = {
    "external_network_authorized": False,
    "graph_writes_authorized": False,
    "production_import_authorized": False,
    "fact_promotion_authorized": False,
    "llm_calls_authorized": False,
}


def load_json(path: Path) -> dict:
    return json.loads(path.read_text())


def test_schemas_exist() -> None:
    for path in SCHEMA_PATHS:
        assert path.exists(), path
        line_count = len(path.read_text().splitlines())
        assert 200 <= line_count <= 500, f"{path} has {line_count} lines"


def test_schemas_valid_json_schema() -> None:
    for path in SCHEMA_PATHS:
        schema = load_json(path)
        assert schema["$schema"] == "http://json-schema.org/draft-07/schema#"
        assert schema["additionalProperties"] is True
        assert schema["$id"].endswith(path.name)
        Draft7Validator.check_schema(schema)


def test_manifest_schema_validates_against_self() -> None:
    schema = load_json(ROOT / "schemas/daily-archive.pdf-batch-manifest.v1.json")
    example = schema["examples"][0]
    validate(instance=example, schema=schema)


def test_retroactive_manifests_built() -> None:
    schema = load_json(ROOT / "schemas/daily-archive.pdf-batch-manifest.v1.json")
    for path, expected_count in MANIFEST_COUNTS.items():
        manifest = load_json(path)
        validate(instance=manifest, schema=schema)
        assert manifest["schema_version"] == "daily-archive.pdf-batch-manifest.v1"
        assert manifest["aggregate"]["pdf_count"] == expected_count
        assert len(manifest["pdfs"]) == expected_count
        assert manifest["safety_defaults"] == SAFETY_DEFAULTS
        for pdf in manifest["pdfs"]:
            pdf_path = ROOT / pdf["path"]
            assert pdf_path.exists(), pdf_path
            assert pdf["storage_provider"] == "local"
            assert pdf["size_bytes"] == pdf_path.stat().st_size
            assert pdf["expected_parsers"]


def test_jsonschema_validation_works_on_m054() -> None:
    completed = subprocess.run(
        [
            sys.executable,
            "scripts/m059_jsonschema_validate.py",
            "--manifest=artifacts/m054-pdf-acquisition/manifest.json",
            "--parser=grobid",
        ],
        cwd=ROOT,
        check=False,
        text=True,
        capture_output=True,
    )
    assert completed.returncode == 0, completed.stdout + completed.stderr
    assert "aggregate total=5 passed=5 failed=0 missing=0" in completed.stdout


def test_5_safety_defaults() -> None:
    script_text = (ROOT / "scripts/m059_build_manifest.py").read_text()
    validator_text = (ROOT / "scripts/m059_jsonschema_validate.py").read_text()
    for key in SAFETY_DEFAULTS:
        assert key in script_text
        assert key in validator_text
    for path in MANIFEST_COUNTS:
        manifest = load_json(path)
        assert manifest["safety_defaults"] == SAFETY_DEFAULTS
        assert set(manifest["safety_defaults"]) == set(SAFETY_DEFAULTS)
        assert all(value is False for value in manifest["safety_defaults"].values())


def test_m050_m058_regression_artifacts_still_present() -> None:
    regression_paths = [
        ROOT / "artifacts/m054-pdf-acquisition/target-subset.json",
        ROOT / "artifacts/m054-pdf-acquisition/acquisition-log.json",
        ROOT / "artifacts/m055-parser-benchmark/corpus-manifest.json",
        ROOT / "artifacts/m055deep-parser-benchmark/corpus-manifest-20.json",
        ROOT / "artifacts/m056-bfs-graph/cumulative-corpus.json",
        ROOT / "artifacts/m056-bfs-graph/candidate-edges.json",
        ROOT / "artifacts/m057-fd-marker/table-similarity/summary.json",
        ROOT / "artifacts/m057-fd-marker/figure-links/summary.json",
        ROOT / "artifacts/m058-plotextractor/summary.json",
    ]
    for path in regression_paths:
        assert path.exists(), path

    assert (
        len(load_json(ROOT / "artifacts/m054-pdf-acquisition/target-subset.json")["records"]) == 5
    )
    assert (
        len(load_json(ROOT / "artifacts/m055-parser-benchmark/corpus-manifest.json")["pdfs"]) == 5
    )
    assert (
        len(load_json(ROOT / "artifacts/m055deep-parser-benchmark/corpus-manifest-20.json")["pdfs"])
        == 20
    )
    assert len(load_json(ROOT / "artifacts/m056-bfs-graph/cumulative-corpus.json")["pdfs"]) == 166
    assert (
        load_json(ROOT / "artifacts/m057-fd-marker/table-similarity/summary.json")["edges_total"]
        == 4934
    )
    assert len(load_json(ROOT / "artifacts/m058-plotextractor/summary.json")["per_pdf"]) == 5


def test_new_source_and_markdown_use_127001_not_loopback_alias() -> None:
    forbidden_loopback_alias = "local" + "host"
    checked_paths = [
        ROOT / "scripts/m059_build_manifest.py",
        ROOT / "scripts/m059_jsonschema_validate.py",
        ROOT / "doc/adr/ADR-013-manifest-driven-pdf-ingest.md",
        *SCHEMA_PATHS,
    ]
    for path in checked_paths:
        assert forbidden_loopback_alias not in path.read_text()
