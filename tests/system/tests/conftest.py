"""Pone tests/system/tests y tests/system/tools en sys.path (ver t3_helpers.py)."""
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
for p in (HERE, HERE.parent / "tools"):
    if str(p) not in sys.path:
        sys.path.insert(0, str(p))
