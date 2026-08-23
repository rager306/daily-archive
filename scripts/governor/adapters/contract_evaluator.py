"""Generic contract evaluator — replaces 9 bespoke _check methods.

Reads a contract YAML (fixtures + source markers + severity + dimension)
and produces DriftFlags. This is the single code path for all
"load YAML → validate fixtures → check markers → emit flags" checks,
eliminating ~737 lines of duplicated boilerplate.

Contract YAML schema (unified):
    schema_version: str
    policy_id: str
    dimension: str
    severity: str
    anchor: str
    fixtures:
        positive: { path: str, required_fields: [str, ...], ... }
        negative: { path: str, expect: {require_violations: [str, ...]} }
    source_markers:
        <name>: { path: str, required_patterns: [str, ...], forbidden_patterns: [str, ...] }

The evaluator handles three concerns per contract:
  1. Fixture validation: positive fixtures must exist and be readable;
     negative fixtures must trigger their labelled violations.
  2. Source markers: required patterns must be present; forbidden must be absent.
  3. Flag emission: uniform DriftFlag construction from contract metadata.
"""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import yaml

from governor.domain.trajectory import DriftFlag


class ContractEvaluator:
    """Evaluates a DATA-driven contract against the repository."""

    def __init__(self, root: Path, contract_path: Path):
        self.root = root
        self.contract = self._load(contract_path)
        self.dimension = str(self.contract.get("dimension", "unknown"))
        self.severity = str(self.contract.get("severity", "medium"))
        self.policy_id = str(self.contract.get("policy_id", contract_path.stem))

    def _load(self, path: Path) -> dict[str, Any]:
        if not path.exists():
            return {}
        try:
            return yaml.safe_load(path.read_text(encoding="utf-8")) or {}
        except (OSError, yaml.YAMLError) as error:
            raise ContractLoadError(f"cannot load {path}: {error}") from error

    def _flag(self, code: str, evidence: str) -> DriftFlag:
        return DriftFlag(
            flag=code,
            severity=self.severity,
            evidence=evidence,
            dimension=self.dimension,
        )

    def evaluate(self) -> list[DriftFlag]:
        """Run the full contract evaluation."""
        flags: list[DriftFlag] = []
        if not self.contract:
            return [self._flag(
                f"{self.policy_id}_contract_missing",
                f"contract file not found or empty",
            )]

        flags.extend(self._check_fixtures())
        flags.extend(self._check_source_markers())
        return flags

    def _check_fixtures(self) -> list[DriftFlag]:
        flags: list[DriftFlag] = []
        for kind, spec in (self.contract.get("fixtures") or {}).items():
            fixture_path = self.root / str(spec.get("path", ""))
            if not fixture_path.is_file():
                flags.append(self._flag(
                    f"{self.policy_id}_fixture_missing",
                    f"{kind} fixture missing: {fixture_path}",
                ))
                continue

            try:
                data = json.loads(fixture_path.read_text(encoding="utf-8"))
            except (ValueError, OSError) as error:
                flags.append(self._flag(
                    f"{self.policy_id}_fixture_invalid",
                    f"{kind} fixture unreadable: {error}",
                ))
                continue

            # Validate expect constraints: field=value pairs must match
            expect = spec.get("expect") or {}
            for field, expected in expect.items():
                if field == "require_violations":
                    # Negative fixture must contain labelled violations
                    observed = self._extract_violations(data)
                    for v in (expected if isinstance(expected, list) else [expected]):
                        if v not in observed:
                            flags.append(self._flag(
                                f"{self.policy_id}_fixture_broken",
                                f"{kind} fixture missing required violation: {v}",
                            ))
                    continue

                actual = self._get_nested(data, field)
                if actual is None:
                    flags.append(self._flag(
                        f"{self.policy_id}_fixture_invalid",
                        f"{kind} fixture missing expected field: {field}",
                    ))
                elif isinstance(expected, bool) and actual != expected:
                    flags.append(self._flag(
                        f"{self.policy_id}_fixture_broken",
                        f"{kind} fixture {kind} must have {field}={expected}, got {actual}",
                    ))
                elif isinstance(expected, (int, float)) and actual != expected:
                    flags.append(self._flag(
                        f"{self.policy_id}_fixture_broken",
                        f"{kind} fixture must have {field}={expected}, got {actual}",
                    ))

            # Positive fixtures must satisfy required_fields
            if kind == "positive":
                required = spec.get("required_fields", [])
                rc = self.contract.get("report_contract", {}).get("required_fields", [])
                for field in (required + rc):
                    if field and field not in (data if isinstance(data, dict) else {}):
                        flags.append(self._flag(
                            f"{self.policy_id}_fixture_invalid",
                            f"positive fixture missing required field: {field}",
                        ))

        return flags

    def _get_nested(self, data: Any, dotpath: str) -> Any:
        """Get a value by dot-notation path (e.g. 'a.b.c')."""
        parts = dotpath.split(".")
        current = data
        for part in parts:
            if isinstance(current, dict):
                current = current.get(part)
            else:
                return None
        return current

    def _extract_violations(self, data: Any) -> list[str]:
        """Extract violation labels from a negative fixture."""
        if isinstance(data, list):
            return [str(item.get("violation", item.get("code", ""))) for item in data if isinstance(item, dict)]
        if isinstance(data, dict):
            return [str(data.get("violation", data.get("code", "")))]
        return []

    def _check_source_markers(self) -> list[DriftFlag]:
        flags: list[DriftFlag] = []
        for marker_key, entry in (self.contract.get("source_markers") or {}).items():
            marker_path = self.root / str(entry.get("path", ""))
            if not marker_path.is_file():
                flags.append(self._flag(
                    f"{self.policy_id}_source_marker_missing",
                    f"{marker_key} source missing: {marker_path}",
                ))
                continue

            text = marker_path.read_text(errors="replace")

            for pattern in entry.get("required_patterns", []):
                if pattern not in text:
                    flags.append(self._flag(
                        f"{self.policy_id}_source_marker_missing",
                        f"{marker_key} source missing enforcement marker: {pattern}",
                    ))

            for pattern in entry.get("forbidden_patterns", []):
                if pattern in text:
                    flags.append(self._flag(
                        f"{self.policy_id}_forbidden_pattern",
                        f"{marker_key} source contains forbidden pattern: {pattern}",
                    ))

        return flags


class ContractLoadError(Exception):
    pass
