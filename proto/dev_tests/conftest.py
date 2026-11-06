import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
PROTO = os.path.dirname(HERE)
ROOT = os.path.dirname(PROTO)
SYN = os.path.join(ROOT, "data", "sessions", "synthetic")

if PROTO not in sys.path:
    sys.path.insert(0, PROTO)
