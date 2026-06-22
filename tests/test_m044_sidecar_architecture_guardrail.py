from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MODULE_PATH = ROOT / "scripts" / "verify_m044_sidecar_architecture_guardrail.py"
spec = importlib.util.spec_from_file_location(
    "verify_m044_sidecar_architecture_guardrail", MODULE_PATH
)
assert spec is not None
guard = importlib.util.module_from_spec(spec)
sys.modules["verify_m044_sidecar_architecture_guardrail"] = guard
assert spec.loader is not None
spec.loader.exec_module(guard)


def _pack(tmp_path: Path) -> dict:
    source_refs = {
        "m033_summary": "m033.md",
        "adr_003": "adr003.md",
        "adr_004": "adr004.md",
        "adr_005": "adr005.md",
        "adr_007": "adr007.md",
        "decisions": "decisions.md",
        "m043_fit": "m043.md",
    }
    for rel in source_refs.values():
        (tmp_path / rel).write_text("ok", encoding="utf-8")
    return {
        "pack_id": "m044-sidecar-architecture-context-v1",
        "source_refs": source_refs,
        "mandatory_decisions": [
            {"id": decision, "rule": "rule"} for decision in sorted(guard.REQUIRED_DECISIONS)
        ],
        "required_systems": sorted(guard.REQUIRED_SYSTEMS),
        "prohibited_claims": sorted(guard.REQUIRED_PROHIBITED_CLAIMS),
        "required_packet_flags": dict(guard.REQUIRED_PACKET_FLAGS),
        "required_preflight_commands": [
            "uv run python scripts/verify_m044_sidecar_architecture_guardrail.py"
        ],
        "graph_write_allowed": False,
        "promotion_allowed": False,
        "production_import_attempted": False,
        "import_eligible": False,
    }


def test_verify_context_pack_accepts_complete_pack(tmp_path):
    errors = guard.verify_context_pack(_pack(tmp_path), root=tmp_path)

    assert errors == []


def test_verify_context_pack_rejects_missing_decision(tmp_path):
    pack = _pack(tmp_path)
    pack["mandatory_decisions"] = [
        item for item in pack["mandatory_decisions"] if item["id"] != "ADR-005"
    ]

    errors = guard.verify_context_pack(pack, root=tmp_path)

    assert any("ADR-005" in error for error in errors)


def test_verify_context_pack_rejects_enabled_import_flag(tmp_path):
    pack = _pack(tmp_path)
    pack["required_packet_flags"]["import_eligible"] = True
    pack["import_eligible"] = True

    errors = guard.verify_context_pack(pack, root=tmp_path)

    assert any("import_eligible" in error for error in errors)


def test_verify_context_pack_rejects_missing_source_ref_file(tmp_path):
    pack = _pack(tmp_path)
    (tmp_path / pack["source_refs"]["m043_fit"]).unlink()

    errors = guard.verify_context_pack(pack, root=tmp_path)

    assert any("m043_fit" in error for error in errors)


def test_verify_context_pack_rejects_missing_preflight_command(tmp_path):
    pack = _pack(tmp_path)
    pack["required_preflight_commands"] = []

    errors = guard.verify_context_pack(pack, root=tmp_path)

    assert any("preflight command" in error for error in errors)
