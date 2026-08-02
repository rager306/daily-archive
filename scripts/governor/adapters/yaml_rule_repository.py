"""Adapter: YAML rule repository — loads check rules from YAML data files."""

from pathlib import Path
from typing import Any

import yaml

from governor.ports.rule_repository import RuleRepository


class YAMLRuleRepository(RuleRepository):
    """Loads rules from YAML files in the checks/ directory."""

    def __init__(self, checks_dir: Path):
        self.checks_dir = checks_dir

    def load_adr_rules(self) -> list[dict[str, Any]]:
        path = self.checks_dir / "adr_rules.yaml"
        if not path.exists():
            return []
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        return data.get("rules", [])

    def load_dimensions(self) -> list[dict[str, Any]]:
        path = self.checks_dir / "dimensions.yaml"
        if not path.exists():
            return []
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        return data.get("dimensions", [])
