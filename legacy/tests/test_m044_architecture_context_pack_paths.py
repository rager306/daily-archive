"""M044 architecture context pack: source_refs must resolve on disk (flat-phase)."""

from __future__ import annotations

import json
from pathlib import Path

from scripts.verify_m044_sidecar_architecture_guardrail import (
    DEFAULT_CONTEXT_PACK,
    verify_context_pack,
)

ROOT = Path(__file__).resolve().parents[1]


def test_m044_context_pack_source_refs_exist() -> None:
    pack = json.loads(DEFAULT_CONTEXT_PACK.read_text(encoding="utf-8"))
    errors = verify_context_pack(pack, root=ROOT)
    assert errors == [], errors


def test_m033_summary_is_flat_phase_path() -> None:
    pack = json.loads(DEFAULT_CONTEXT_PACK.read_text(encoding="utf-8"))
    m033 = pack["source_refs"]["m033_summary"]
    assert m033.startswith(".gsd/phases/"), m033
    assert not m033.startswith(".gsd/milestones/"), m033
    assert (ROOT / m033).is_file()
