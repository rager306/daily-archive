"""Adapter: filesystem ADR repository — reads ADR files from doc/adr/."""

import re
from pathlib import Path

from governor.ports.adr_repository import ADRInfo, ADRRepository


class FilesystemADRRepository(ADRRepository):
    """Reads ADR files from the filesystem."""

    def __init__(self, project_root: Path):
        self.root = project_root

    def list_adrs(self, adr_dir: str = "doc/adr/") -> list[ADRInfo]:
        adr_root = self.root / adr_dir
        if not adr_root.exists():
            return []
        adrs = []
        for path in sorted(adr_root.glob("ADR-*.md")):
            match = re.match(r"ADR-(\d+)", path.name)
            if not match:
                continue
            number = int(match.group(1))
            title = self._extract_title(path)
            status = self._extract_status(path)
            adrs.append(ADRInfo(number=number, title=title, path=path, status=status))
        return sorted(adrs, key=lambda a: a.number)

    def highest_adr_number(self, adr_dir: str = "doc/adr/") -> int:
        adrs = self.list_adrs(adr_dir)
        return max((a.number for a in adrs), default=0)

    def _extract_title(self, path: Path) -> str:
        try:
            text = path.read_text(encoding="utf-8", errors="replace")
            for line in text.splitlines():
                if line.startswith("# "):
                    return line[2:].strip()
        except OSError:
            pass
        return path.stem

    def _extract_status(self, path: Path) -> str:
        try:
            text = path.read_text(encoding="utf-8", errors="replace")
            for line in text.splitlines()[:10]:
                if "Status" in line:
                    if "Accepted" in line:
                        return "Accepted"
                    if "Superseded" in line:
                        return "Superseded"
                    if "Deprecated" in line:
                        return "Deprecated"
        except OSError:
            pass
        return "Accepted"
