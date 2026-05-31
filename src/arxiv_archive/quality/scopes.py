"""Default source scopes for maintainability diagnostics."""

from __future__ import annotations

from pathlib import Path

DEFAULT_QUALITY_SCOPE = (Path("src/arxiv_archive"),)
DEFAULT_QUALITY_EXCLUDES = (
    "**/__pycache__/**",
    "**/.venv/**",
)


def normalize_scope(paths: tuple[str | Path, ...] | list[str | Path] | None = None) -> tuple[Path, ...]:
    """Return explicit scan paths or the repository's default Python package scope."""
    if not paths:
        return DEFAULT_QUALITY_SCOPE
    return tuple(Path(path) for path in paths)


def scope_payload(paths: tuple[Path, ...]) -> dict[str, list[str]]:
    """Return the diagnostic scope as a JSON-native payload."""
    return {
        "paths": [str(path) for path in paths],
        "exclude": list(DEFAULT_QUALITY_EXCLUDES),
    }


__all__ = ["DEFAULT_QUALITY_EXCLUDES", "DEFAULT_QUALITY_SCOPE", "normalize_scope", "scope_payload"]
