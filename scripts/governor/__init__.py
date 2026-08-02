"""governor — Hexagonal trajectory enforcement CLI for daily-archive.

Universal, data-driven, hexagonal/onion architecture:
- domain/: pure logic (no IO) — artifacts, checks, trajectory types
- ports/: abstract interfaces — what governor needs to read
- adapters/: concrete implementations — how governor reads filesystem/JSON
- application/: use cases — orchestrate domain + ports
- checks/: DATA files (YAML) — rules and dimensions, not code

Usage:
    python3 -m governor --phase preflight
    python3 -m governor --phase active
    python3 -m governor --phase closeout
    python3 -m governor --list-checks
"""

__version__ = "1.0.0"
