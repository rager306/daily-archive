"""Port: ADR repository — reads ADR files and their metadata."""

from abc import ABC, abstractmethod
from dataclasses import dataclass
from pathlib import Path


@dataclass
class ADRInfo:
    """Metadata about an ADR file."""

    number: int
    title: str
    path: Path
    status: str = "Accepted"


class ADRRepository(ABC):
    """Reads ADR files from doc/adr/ directory."""

    @abstractmethod
    def list_adrs(self, adr_dir: str = "doc/adr/") -> list[ADRInfo]:
        """List all ADR files sorted by number."""
        ...

    @abstractmethod
    def highest_adr_number(self, adr_dir: str = "doc/adr/") -> int:
        """Return the highest ADR number (for next-ADR suggestion)."""
        ...
