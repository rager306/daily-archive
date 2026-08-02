#!/usr/bin/env python3
"""Legacy entry point — delegates to governor hexagonal package (M286-wda3cp).

This file remains for backward compatibility. The actual logic now lives in
scripts/governor/ — a hexagonal Python package with:
- domain/ (pure logic, no IO)
- ports/ (abstract interfaces)
- adapters/ (filesystem implementations)
- application/ (use cases)
- checks/ (YAML data files — rules and dimensions)

Usage:
    python3 scripts/check_project_trajectory.py [--phase preflight|active|closeout]
    python3 scripts/governor --phase preflight  (new canonical path)
"""

import os
import sys
from pathlib import Path

# Add scripts/ to PYTHONPATH so `import governor` resolves.
_scripts_dir = Path(__file__).resolve().parent
if str(_scripts_dir) not in sys.path:
    sys.path.insert(0, str(_scripts_dir))

# Set PYTHONPATH for the subprocess (governor __main__.py uses it).
os.environ["PYTHONPATH"] = _scripts_dir.as_posix()

from governor.__main__ import main

if __name__ == "__main__":
    sys.exit(main())
