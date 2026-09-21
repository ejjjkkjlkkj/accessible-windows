#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import importlib.util
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "boot" / "uefi-native-speech-v2" / "native_speech_v2.py"

LETTER_UNITS = {
    "a": ("a",), "b": ("b","e"), "c": ("s","e"), "d": ("d","e"),
    "e": ("e",), "f": ("e","f"), "g": ("zh","e"), "h": ("a","sh"),
    "i": ("i",), "j": ("zh","i"), "k": ("k","a"), "l": ("e","l"),
    "m": ("e","m"), "n": ("e","n"), "o": ("o",), "p": ("p","e"),
    "q": ("k","u"), "r": ("e","r"), "s": ("e","s"), "t": ("t","e"),
    "u": ("u",), "v": ("v","e"), "w": ("d","u","b","l","e","v","e"),
    "x": ("i","k","s"), "y": ("i","g","r","e","k"), "z": ("z","e","d"),
}

def load_source():
    spec = importlib.util.spec_from_file_location("qev_native_speech_v2", SOURCE)
    if spec is None or spec.loader is None:
        raise SystemExit("cannot load native speech v2")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module

def arr_u8(name: str, values: list[int], cols: int = 16) -> str:
    lines = []
    for i in range(0, len(values), cols):
        lines.append("    " + ", ".join(str(x) for x in values[i:i+cols]) + ",")
    return f"const unsigned char {name}[] = {{\n" + "\n".join(lines) + "\n};\n"

def arr_u32(name: str, values: list[int], cols: int = 8) -> str:
    lines = []
    for i in range(0, len(values), cols):
        lines.append("    " + ", ".join(str(x) + "u" for x in values[i:i+cols]) + ",")
    return f"const unsigned int {name}[] = {{\n" + "\n".join(lines) + "\n};\n"

def main() -> None:
    if len(sys.argv) != 3:
        raise SystemExit("usage: generate_hii_speech_v2.py OUTPUT_C METADATA")

    output = Path(sys.argv[1])
    metadata = Path(sys.argv[2])
    speech = load_source()
    source_units = speech.make_units()

    required = {"sil"} | {u for seq in LETTER_UNITS.values() for u in seq}
    missing = sorted(required - set(source_units))
    if missing:
        raise SystemExit("missing v2 units: " + ",".join(missing))

    names = sorted(required)
    converted = {
        name: speech.stereo_s16le_48k(source_units[name])
        for name in names
    }

    bank = bytearray()
    offsets: list[int] = []
    lengths: list[int] = []
    for name in names:
        offsets.append(len(bank))
        payload = converted[name]
        lengths.append(len(payload))
        bank += payload

    # Existing freestanding HII/HDA runtime allocates 128 pages and reserves
    # the first 0x1000 bytes for BDL/control structures.
    max_bank = 128 * 4096 - 0x1000
    if len(bank) > max_bank:
        raise SystemExit(f"v2 unit bank too large: {len(bank)} > {max_bank}")

    index = {name: i for i, name in enumerate(names)}
    counts: list[int] = []
    flat: list[int] = []
    for ch in "abcdefghijklmnopqrstuvwxyz":
        seq = LETTER_UNITS[ch]
        if len(seq) > 8:
            raise SystemExit(f"{ch}: letter expansion too large")
        counts.append(len(seq))
        flat.extend([index[u] for u in seq] + [0] * (8 - len(seq)))

    source_sha = hashlib.sha256(SOURCE.read_bytes()).hexdigest()
    bank_sha = hashlib.sha256(bank).hexdigest()

    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text("\n".join([
        "/* Generated from first-party native speech v2. */",
        arr_u8("qev_unit_bank", list(bank)),
        f"const unsigned int qev_unit_bank_len = {len(bank)}u;\n",
        arr_u32("qev_unit_off", offsets),
        arr_u32("qev_unit_len", lengths),
        f"const unsigned int qev_unit_count = {len(names)}u;\n",
        f"const unsigned int qev_sil_unit_index = {index['sil']}u;\n",
        arr_u8("qev_letter_unit_count", counts),
        arr_u8("qev_letter_units", flat),
    ]), encoding="utf-8")

    metadata.write_text("\n".join([
        "UEFI-ACCESSIBILITY-PLATFORM-V1-SPEECH",
        "status = PASS",
        "source = boot/uefi-native-speech-v2/native_speech_v2.py",
        f"source-sha256 = {source_sha}",
        f"source-rate = {speech.SAMPLE_RATE}",
        "transport-rate = 48000",
        "transport-format = signed-16-bit-stereo",
        f"unit-count = {len(names)}",
        f"bank-bytes = {len(bank)}",
        f"bank-sha256 = {bank_sha}",
        "external-tts-dependency = 0",
        "full-utterance-assets = 0",
        "hii-fallback = french-letter-name-spelling",
        ""
    ]), encoding="utf-8")

    print("UEFI_ACCESSIBILITY_PLATFORM_SPEECH_V2=PASS")
    print(f"UNIT_COUNT={len(names)}")
    print(f"BANK_BYTES={len(bank)}")
    print(f"BANK_SHA256={bank_sha}")

if __name__ == "__main__":
    main()
