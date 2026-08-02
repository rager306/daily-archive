"""Port: rule repository — loads check rules from YAML data files."""

from abc import ABC, abstractmethod
from typing import Any


class RuleRepository(ABC):
    """Loads check rules, dimensions, and artifact field definitions from YAML."""

    @abstractmethod
    def load_adr_rules(self) -> list[dict[str, Any]]:
        """Load ADR audit rules from checks/adr_rules.yaml."""
        ...

    @abstractmethod
    def load_dimensions(self) -> list[dict[str, Any]]:
        """Load dimension definitions from checks/dimensions.yaml."""
        ...
