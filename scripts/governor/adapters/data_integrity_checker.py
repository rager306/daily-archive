"""Data integrity checker — four structural rules (M449 S02).

All rules are DATA-driven from data_integrity.yaml:
  1. no_hardcoded_service_urls: scan Rust src for hardcoded URLs
  2. no_nested_data_dirs: detect data/X/X nesting
  3. cache_invalidation_bound: verify stage contracts bind config sha
  4. artifact_lifecycle_gc: flag stale artifact dirs
"""

from __future__ import annotations

import os
import re
from datetime import datetime, timedelta
from pathlib import Path
from typing import Any

import yaml

from governor.domain.trajectory import DriftFlag


def check_data_integrity(root: Path) -> list[DriftFlag]:
    """Evaluate all data integrity rules from YAML contract."""
    contract_path = root / "scripts/governor/checks/data_integrity.yaml"
    if not contract_path.exists():
        return []

    try:
        contract = yaml.safe_load(contract_path.read_text()) or {}
    except (OSError, yaml.YAMLError):
        return []

    if not contract:
        return []

    dimension = str(contract.get("dimension", "data_integrity"))
    severity = str(contract.get("severity", "medium"))

    def _flag(code: str, evidence: str) -> DriftFlag:
        return DriftFlag(flag=code, severity=severity, evidence=evidence, dimension=dimension)

    flags: list[DriftFlag] = []

    for rule in contract.get("rules", []):
        rule_id = rule.get("id", "unknown")

        if rule_id == "no_hardcoded_service_urls":
            flags.extend(_check_hardcoded_urls(root, rule, _flag))
        elif rule_id == "no_nested_data_dirs":
            flags.extend(_check_nested_dirs(root, rule, _flag))
        elif rule_id == "cache_invalidation_bound":
            flags.extend(_check_cache_binding(root, rule, _flag))
        elif rule_id == "artifact_lifecycle_gc":
            flags.extend(_check_artifact_gc(root, rule, _flag))
        elif rule_id == "textbook_manifest_required":
            flags.extend(_check_textbook_manifests(root, rule, _flag))

    return flags


def _check_textbook_manifests(root: Path, rule: dict, _flag) -> list[DriftFlag]:
    """Every textbook under sources/textbook/<id>/ needs a textbook_manifest.json."""
    flags = []
    textbook_dir = root / rule.get("scan", "data/sources/textbook/")
    if not textbook_dir.exists():
        return flags
    for textbook in textbook_dir.iterdir():
        if not textbook.is_dir():
            continue
        manifest = textbook / "textbook_manifest.json"
        if not manifest.is_file():
            flags.append(_flag(
                "textbook_manifest_missing",
                f"textbook/{textbook.name}/ has no textbook_manifest.json — "
                f"add one describing format and parsing approach",
            ))
    return flags


def _check_hardcoded_urls(root: Path, rule: dict, _flag) -> list[DriftFlag]:
    """Scan Rust src for hardcoded service URLs."""
    flags = []
    pattern = re.compile(rule.get("pattern", r'https?://[a-z0-9.-]+'))
    exclude = re.compile(rule.get("exclude_pattern", ""), re.IGNORECASE) if rule.get("exclude_pattern") else None

    crates_dir = root / "crates"
    if not crates_dir.exists():
        return flags

    # All allowlists come from YAML (exclude_pattern + exclude_dirs)
    exclude_dirs = rule.get("exclude_dirs", ["tests", "examples", "benches"])
    exclude_suffixes = rule.get("suffix_exclude", [".md", ".txt"])

    for rs_file in crates_dir.rglob("*.rs"):
        rel_parts = rs_file.relative_to(root).parts
        # Skip excluded directories (from YAML)
        if any(part in exclude_dirs for part in rel_parts):
            continue
        rel = rs_file.relative_to(root)
        try:
            text = rs_file.read_text(errors="replace")
        except OSError:
            continue

        for match in pattern.finditer(text):
            url = match.group(0)
            # Skip if matches the YAML exclude_pattern regex
            if exclude and exclude.search(url):
                continue
            line_num = text[:match.start()].count('\n') + 1
            flags.append(_flag(
                "data_integrity_hardcoded_url",
                f"{rel}:{line_num} contains hardcoded URL '{url}' "
                f"— move to .env or DATA dictionary",
            ))
            break  # one hit per file is enough

    return flags


def _check_nested_dirs(root: Path, rule: dict, _flag) -> list[DriftFlag]:
    """Detect data/X/X nesting (same-name directory inside directory)."""
    flags = []
    data_dir = root / "data"
    if not data_dir.exists():
        return flags

    for entry in data_dir.iterdir():
        if not entry.is_dir():
            continue
        # Check for same-name subdirectory
        nested = entry / entry.name
        if nested.exists() and nested.is_dir():
            flags.append(_flag(
                "data_integrity_nested_dir",
                f"data/{entry.name}/{entry.name}/ — same-name nesting detected; flatten",
            ))

    return flags


def _check_cache_binding(root: Path, rule: dict, _flag) -> list[DriftFlag]:
    """Verify stage contracts include config fingerprint binding."""
    flags = []
    target = root / rule.get("scan", "crates/da-adapters/src/pipeline_stages.rs")
    if not target.exists():
        target = root / "crates/da-adapters/src/pipeline_stages.rs"
    if not target.exists():
        return flags

    text = target.read_text(errors="replace")
    for pattern in rule.get("required_patterns", []):
        if pattern not in text:
            flags.append(_flag(
                "data_integrity_cache_binding_missing",
                f"{target.relative_to(root)} missing required pattern '{pattern}' — "
                f"cache invalidation will break on DATA changes (M446 lesson)",
            ))

    return flags


def _check_artifact_gc(root: Path, rule: dict, _flag) -> list[DriftFlag]:
    """Flag artifact dirs older than max_age_days with no code readers."""
    flags = []
    artifacts_dir = root / "artifacts"
    if not artifacts_dir.exists():
        return flags

    max_age = timedelta(days=rule.get("max_age_days", 30))
    cutoff = datetime.now() - max_age

    # Collect all artifact dir names
    artifact_dirs = [d for d in artifacts_dir.iterdir() if d.is_dir()]

    # Scan code for references to each artifact dir
    code_text = ""
    crates_dir = root / "crates"
    if crates_dir.exists():
        for rs_file in crates_dir.rglob("*.rs"):
            try:
                code_text += rs_file.read_text(errors="replace")[:5000]
            except OSError:
                continue

    for adir in artifact_dirs:
        name = adir.name
        # Check if any code references this directory
        has_reader = name in code_text

        # Check modification time
        try:
            mtime = datetime.fromtimestamp(adir.stat().st_mtime)
        except OSError:
            continue

        if mtime < cutoff and not has_reader:
            age_days = (datetime.now() - mtime).days
            flags.append(_flag(
                "data_integrity_stale_artifact",
                f"artifacts/{name}/ is {age_days} days old with no code readers — archive it",
            ))

    return flags
