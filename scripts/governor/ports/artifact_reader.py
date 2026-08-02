"""Port: artifact reader — reads JSON artifacts from the filesystem."""

from abc import ABC, abstractmethod
from pathlib import Path
from typing import Iterator


class ArtifactReader(ABC):
    """Reads JSON artifact files from the project."""

    @abstractmethod
    def iter_artifacts(self, base_dir: str = "artifacts/") -> Iterator[Path]:
        """Yield all JSON artifact files under base_dir."""
        ...

    @abstractmethod
    def read_json(self, path: Path) -> dict:
        """Read a JSON file and return its contents."""
        ...
