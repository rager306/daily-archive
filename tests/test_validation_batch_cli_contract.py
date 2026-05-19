from __future__ import annotations

import json
import subprocess


def _run_cli(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["uv", "run", "python", "-m", "arxiv_archive", *args],
        check=False,
        capture_output=True,
        text=True,
    )


def test_validation_batch_help_lists_contract_commands() -> None:
    result = _run_cli("validation-batch", "--help")

    assert result.returncode == 0
    assert "contract" in result.stdout
    assert "init" in result.stdout
    assert "preflight" in result.stdout
    assert "scan" in result.stdout
    assert "review" in result.stdout
    assert "resume" in result.stdout


def test_validation_batch_contract_json_is_safe() -> None:
    result = _run_cli("validation-batch", "contract", "--json")

    assert result.returncode == 0
    payload = json.loads(result.stdout)
    assert payload["schema_version"] == "m007-validation-batch-contract.v1"
    assert payload["status"] == "contract_only"
    assert payload["real_source_acquisition_performed"] is False
    assert payload["real_scan_performed"] is False
    assert payload["production_import_attempted"] is False
    assert payload["ladybugdb_written"] is False
    assert "No production KG import" in payload["boundary"]


def test_validation_batch_review_stub_is_nonzero_and_safe() -> None:
    result = _run_cli("validation-batch", "review", "--batch-id", "fixture-b001", "--json")

    assert result.returncode == 1
    payload = json.loads(result.stdout)
    assert payload["batch_id"] == "fixture-b001"
    assert payload["status"] == "not_implemented"
    assert payload["real_source_acquisition_performed"] is False
    assert payload["real_scan_performed"] is False
    assert payload["production_import_attempted"] is False
    assert payload["ladybugdb_written"] is False


def test_validation_batch_scan_stub_does_not_claim_work() -> None:
    result = _run_cli("validation-batch", "scan", "--batch-id", "fixture-b001", "--json")

    assert result.returncode == 1
    payload = json.loads(result.stdout)
    assert payload["status"] == "not_implemented"
    assert payload["real_scan_performed"] is False
    assert payload["raw_text_included"] is False
    assert payload["chunk_text_included"] is False
