"""Procedural sound design for motion graphics. Pure Python stdlib, 48 kHz stereo.

    python3 -m sfx sfx/cues/effort.py      render a cue sheet to WAV
    python3 -m sfx --list                  list the sound generators
"""

from .mix import Mix
from .synth import SR

__all__ = ["Mix", "SR"]
