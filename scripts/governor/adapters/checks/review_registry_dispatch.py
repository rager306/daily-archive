"""DATA-driven dispatch for the Governor `review` command.

`governor review --registry <contract.yaml>` selects the matching validator from
the contract's declared `format`, so adding a review registry stays a DATA
change rather than a new CLI surface or shell guard. Unknown or malformed
contracts fall back to the original GAR JSONL validator, which fails closed and
reports the contract as invalid.
"""
from __future__ import annotations

from collections.abc import Mapping
from pathlib import Path

import yaml

from governor.adapters.checks.grobid_horizon_review import check_grobid_horizon_review
from governor.adapters.checks.grobid_review_registry import check_grobid_review_registry
from governor.domain.trajectory import DriftFlag

HORIZON_FORMAT = "horizon-json-v1"
_DEFAULT_GAR_CONTRACT = "scripts/governor/checks/grobid_absorption_review.yaml"


def _contract_mapping(root: Path, contract_rel: str) -> dict | None:
    candidate = Path(contract_rel)
    path = candidate if candidate.is_absolute() else root / candidate
    if not path.is_file():
        return None
    try:
        loaded = yaml.safe_load(path.read_text(encoding="utf-8"))
    except (OSError, yaml.YAMLError):
        return None
    return loaded if isinstance(loaded, Mapping) else None


def check_review_registry(root: Path, contract_rel: str | None = None) -> list[DriftFlag]:
    """Validate whichever controlled review registry the contract selects.

    Dispatch is declared by DATA: a contract carrying `format: horizon-json-v1`
    routes to the long-horizon stage validator; anything else (including a
    missing or malformed contract) routes to the GAR JSONL validator so the
    existing fail-closed behaviour is preserved.
    """
    effective_rel = contract_rel or _DEFAULT_GAR_CONTRACT
    contract = _contract_mapping(root, effective_rel)
    if contract is not None and str(contract.get("format", "")) == HORIZON_FORMAT:
        return check_grobid_horizon_review(root, effective_rel)
    return check_grobid_review_registry(root, contract_rel)
