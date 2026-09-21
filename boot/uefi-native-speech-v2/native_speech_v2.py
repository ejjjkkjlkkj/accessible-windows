#!/usr/bin/env python3
from __future__ import annotations

import math
import struct
from dataclasses import dataclass

SAMPLE_RATE = 16000
TAU = math.tau
MAX_I16 = 32767
MIN_I16 = -32768
CROSSFADE_MS = 8
WORD_GAP_MS = 28

@dataclass(frozen=True)
class UnitSpec:
    kind: str
    duration_ms: int
    pitch_hz: float
    formants: tuple[int, int, int]
    gains: tuple[float, float, float]

UNIT_SPECS: dict[str, UnitSpec] = {
    "a": UnitSpec("v", 120, 118.0, (730, 1090, 2440), (0.55, 0.30, 0.15)),
    "e": UnitSpec("v", 115, 126.0, (530, 1840, 2480), (0.55, 0.30, 0.15)),
    "i": UnitSpec("v", 110, 130.0, (270, 2290, 3010), (0.55, 0.30, 0.15)),
    "o": UnitSpec("v", 120, 116.0, (570, 840, 2410), (0.58, 0.28, 0.14)),
    "u": UnitSpec("v", 112, 122.0, (300, 870, 2240), (0.58, 0.28, 0.14)),
    "eu": UnitSpec("v", 120, 120.0, (420, 1550, 2480), (0.56, 0.29, 0.15)),
    "on": UnitSpec("n", 132, 116.0, (500, 1000, 2000), (0.60, 0.27, 0.13)),
    "n": UnitSpec("n", 88, 116.0, (260, 1000, 2100), (0.64, 0.24, 0.12)),
    "m": UnitSpec("n", 92, 116.0, (250, 900, 2050), (0.66, 0.22, 0.12)),
    "r": UnitSpec("r", 82, 120.0, (420, 1450, 2200), (0.52, 0.30, 0.18)),
    "l": UnitSpec("l", 88, 118.0, (390, 1500, 2400), (0.55, 0.29, 0.16)),
    "w": UnitSpec("v", 78, 118.0, (330, 900, 2200), (0.60, 0.26, 0.14)),
    "s": UnitSpec("f", 105, 0.0, (2600, 4300, 6200), (0.18, 0.38, 0.44)),
    "sh": UnitSpec("f", 112, 0.0, (1800, 3200, 5000), (0.24, 0.38, 0.38)),
    "f": UnitSpec("f", 102, 0.0, (1200, 2500, 4800), (0.25, 0.35, 0.40)),
    "v": UnitSpec("z", 102, 116.0, (700, 1500, 2800), (0.48, 0.31, 0.21)),
    "z": UnitSpec("z", 102, 118.0, (900, 1800, 3000), (0.47, 0.32, 0.21)),
    "zh": UnitSpec("z", 108, 118.0, (800, 1800, 3000), (0.47, 0.32, 0.21)),
    "t": UnitSpec("p", 82, 0.0, (2500, 4300, 6200), (0.20, 0.35, 0.45)),
    "d": UnitSpec("p", 88, 112.0, (700, 1600, 2600), (0.48, 0.32, 0.20)),
    "k": UnitSpec("p", 92, 0.0, (1500, 3000, 5200), (0.25, 0.35, 0.40)),
    "g": UnitSpec("p", 92, 108.0, (600, 1400, 2400), (0.50, 0.31, 0.19)),
    "p": UnitSpec("p", 88, 0.0, (900, 2400, 4500), (0.27, 0.34, 0.39)),
    "b": UnitSpec("p", 88, 108.0, (500, 1300, 2300), (0.52, 0.30, 0.18)),
}

WORDS: dict[str, tuple[str, ...]] = {
    "un": ("eu", "n"),
    "continuer": ("k", "on", "t", "i", "n", "u", "e"),
    "deux": ("d", "eu"),
    "aide": ("e", "d"),
    "trois": ("t", "r", "w", "a"),
    "recuperation": ("r", "e", "k", "u", "p", "e", "r", "a", "s", "i", "on"),
    "erreur": ("e", "r", "eu", "r"),
    "non": ("n", "on"),
}

def _noise(seed: int) -> tuple[int, float]:
    seed ^= (seed << 13) & 0xFFFFFFFF
    seed ^= seed >> 17
    seed ^= (seed << 5) & 0xFFFFFFFF
    return seed & 0xFFFFFFFF, ((seed & 0xFFFF) / 32767.5) - 1.0

def _envelope(index: int, count: int) -> float:
    ramp = max(1, int(SAMPLE_RATE * 0.006))
    attack = min(1.0, index / ramp)
    release = min(1.0, (count - 1 - index) / ramp)
    return max(0.0, min(attack, release))

def _formant(spec: UnitSpec, t: float) -> float:
    return sum(
        gain * math.sin(TAU * freq * t)
        for gain, freq in zip(spec.gains, spec.formants)
    )

def make_unit(name: str) -> list[int]:
    spec = UNIT_SPECS[name]
    nyquist_guard = int(SAMPLE_RATE * 0.45)
    if any(freq < 0 or freq > nyquist_guard for freq in spec.formants):
        raise ValueError(f"{name}: formant exceeds spectral guard")
    count = max(1, SAMPLE_RATE * spec.duration_ms // 1000)
    seed = 0x51564532 ^ sum((i + 1) * ord(ch) for i, ch in enumerate(name))
    out: list[int] = []
    dc = 0.0
    for i in range(count):
        t = i / SAMPLE_RATE
        env = _envelope(i, count)
        form = _formant(spec, t)
        seed, noise = _noise(seed)
        if spec.kind == "v":
            glottal = 0.64 * math.sin(TAU * spec.pitch_hz * t) + 0.22 * math.sin(TAU * spec.pitch_hz * 2 * t)
            x = (0.62 * form + 0.38 * glottal) * env
        elif spec.kind == "n":
            glottal = math.sin(TAU * spec.pitch_hz * t)
            x = (0.52 * form + 0.26 * glottal + 0.08 * noise) * env
        elif spec.kind == "r":
            trill = 0.65 + 0.35 * abs(math.sin(TAU * 24.0 * t))
            x = (0.62 * form * trill + 0.18 * math.sin(TAU * spec.pitch_hz * t)) * env
        elif spec.kind == "l":
            x = (0.70 * form + 0.18 * math.sin(TAU * spec.pitch_hz * t)) * env
        elif spec.kind == "z":
            x = (0.46 * form + 0.28 * math.sin(TAU * spec.pitch_hz * t) + 0.18 * noise) * env
        elif spec.kind == "f":
            carrier = math.sin(TAU * min(spec.formants[1], nyquist_guard) * t)
            x = (0.10 * form + 0.70 * noise * (0.55 + 0.45 * carrier)) * env
        else:
            burst_len = max(1, int(SAMPLE_RATE * 0.012))
            burst = 1.0 if i < burst_len else 0.16
            voiced = 0.18 * math.sin(TAU * spec.pitch_hz * t) if spec.pitch_hz else 0.0
            x = burst * (0.58 * noise + 0.16 * form) + voiced
            x *= env
        # One-pole DC blocker / gentle high-pass behaviour.
        dc = 0.995 * dc + 0.005 * x
        x -= dc
        x = math.tanh(x * 1.35) * 0.78
        sample = int(round(x * MAX_I16))
        out.append(max(MIN_I16, min(MAX_I16, sample)))
    return out

def make_units() -> dict[str, list[int]]:
    return {name: make_unit(name) for name in UNIT_SPECS}

def crossfade(a: list[int], b: list[int], ms: int = CROSSFADE_MS) -> list[int]:
    if not a:
        return list(b)
    if not b:
        return list(a)
    n = min(len(a), len(b), max(1, SAMPLE_RATE * ms // 1000))
    out = list(a[:-n])
    for i in range(n):
        wb = (i + 1) / (n + 1)
        wa = 1.0 - wb
        out.append(int(round(a[-n + i] * wa + b[i] * wb)))
    out.extend(b[n:])
    return out

def render_word(word: str, units: dict[str, list[int]] | None = None) -> list[int]:
    if word not in WORDS:
        raise KeyError(word)
    bank = units if units is not None else make_units()
    out: list[int] = []
    for name in WORDS[word]:
        out = crossfade(out, bank[name])
    return out

def render_phrase(*words: str) -> list[int]:
    bank = make_units()
    gap = [0] * (SAMPLE_RATE * WORD_GAP_MS // 1000)
    out: list[int] = []
    for wi, word in enumerate(words):
        if wi:
            out.extend(gap)
        out.extend(render_word(word, bank))
    return out

def pcm_s16le(samples: list[int]) -> bytes:
    return b"".join(struct.pack("<h", max(MIN_I16, min(MAX_I16, s))) for s in samples)

def stereo_s16le_48k(samples: list[int]) -> bytes:
    # Exact 3x upsample from 16 kHz to the HDA runtime's 48 kHz format.
    out = bytearray()
    if not samples:
        return bytes(out)
    for i, sample in enumerate(samples):
        nxt = samples[i + 1] if i + 1 < len(samples) else sample
        for phase in range(3):
            v = int(round(sample + (nxt - sample) * (phase / 3.0)))
            v = max(MIN_I16, min(MAX_I16, v))
            frame = struct.pack("<hh", v, v)
            out += frame
    return bytes(out)

def quality_metrics(samples: list[int]) -> dict[str, float]:
    if not samples:
        return {"peak": 0.0, "rms": 0.0, "dc": 0.0}
    peak = max(abs(s) for s in samples) / MAX_I16
    rms = math.sqrt(sum(float(s) * s for s in samples) / len(samples)) / MAX_I16
    dc = abs(sum(samples) / len(samples)) / MAX_I16
    return {"peak": peak, "rms": rms, "dc": dc}
