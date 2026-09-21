#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import importlib.util
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "boot" / "uefi-native-speech-v4" / "voicecore_v4.py"
DMA_BYTES = 2048 * 4096
PCM_OFF = 0x1000
LEAD_BYTES = 30 * 192
GAP_BYTES = 12 * 192
TAIL_BYTES = 45 * 192
MAX_LABEL = 32

def load_voicecore():
    spec = importlib.util.spec_from_file_location("voicecore_v4_embed", SOURCE)
    if spec is None or spec.loader is None:
        raise SystemExit("cannot load VoiceCore v4")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module

def arr_u8(name: str, values: bytes | list[int], cols: int = 16) -> str:
    vals = list(values)
    lines = []
    for i in range(0, len(vals), cols):
        lines.append("    " + ", ".join(str(v) for v in vals[i:i+cols]) + ",")
    return f"const unsigned char {name}[] = {{\n" + "\n".join(lines) + "\n};\n"

def arr_u32(name: str, values: list[int], cols: int = 8) -> str:
    lines = []
    for i in range(0, len(values), cols):
        lines.append("    " + ", ".join(str(v) + "u" for v in values[i:i+cols]) + ",")
    return f"const unsigned int {name}[] = {{\n" + "\n".join(lines) + "\n};\n"

def main() -> None:
    if len(sys.argv) != 3:
        raise SystemExit("usage: generate_hii_voicecore_v4.py OUTPUT_C METADATA")

    out_c = Path(sys.argv[1])
    meta = Path(sys.argv[2])
    vc = load_voicecore()

    names = list("abcdefghijklmnopqrstuvwxyz") + ["sil"]
    clips: dict[str, bytes] = {}
    durations: dict[str, float] = {}

    for ch in "abcdefghijklmnopqrstuvwxyz":
        spoken = vc.LETTER_NAMES[ch]
        samples = vc.synthesize(spoken, "screen")
        metrics = vc.quality_metrics(samples)
        if not samples or metrics["clip_ratio"] != 0.0:
            raise SystemExit(f"{ch}: invalid VoiceCore v4 letter clip")
        clips[ch] = vc.pcm_s16le_stereo(samples)
        durations[ch] = metrics["duration_s"]

    silence_samples = [0] * int(vc.SAMPLE_RATE * 0.055)
    clips["sil"] = vc.pcm_s16le_stereo(silence_samples)
    durations["sil"] = len(silence_samples) / vc.SAMPLE_RATE

    bank = bytearray()
    offsets: list[int] = []
    lengths: list[int] = []
    for name in names:
        offsets.append(len(bank))
        payload = clips[name]
        lengths.append(len(payload))
        bank += payload

    index = {name: i for i, name in enumerate(names)}
    counts = [1] * 26
    flat: list[int] = []
    for ch in "abcdefghijklmnopqrstuvwxyz":
        flat.extend([index[ch]] + [0] * 7)

    worst_ch = max("abcdefghijklmnopqrstuvwxyz", key=lambda ch: len(clips[ch]))
    worst_clip = len(clips[worst_ch])
    worst_total = LEAD_BYTES + TAIL_BYTES + MAX_LABEL * worst_clip + (MAX_LABEL - 1) * GAP_BYTES
    usable = DMA_BYTES - PCM_OFF
    if worst_total > usable:
        raise SystemExit(
            f"worst-case VoiceCore label exceeds UEFI DMA: {worst_total} > {usable} ({worst_ch})"
        )

    source_sha = hashlib.sha256(SOURCE.read_bytes()).hexdigest()
    bank_sha = hashlib.sha256(bank).hexdigest()

    out_c.parent.mkdir(parents=True, exist_ok=True)
    out_c.write_text("\n".join([
        "/* Generated from VoiceCore v4 full French letter-name clips. */",
        arr_u8("qev_unit_bank", bank),
        f"const unsigned int qev_unit_bank_len = {len(bank)}u;\n",
        arr_u32("qev_unit_off", offsets),
        arr_u32("qev_unit_len", lengths),
        f"const unsigned int qev_unit_count = {len(names)}u;\n",
        f"const unsigned int qev_sil_unit_index = {index['sil']}u;\n",
        arr_u8("qev_letter_unit_count", counts),
        arr_u8("qev_letter_units", flat),
    ]), encoding="utf-8")

    lines = [
        "UEFI-ACCESSIBILITY-PLATFORM-V2-VOICECORE-V4",
        "status = PASS",
        "speech-engine = VoiceCore v4",
        "voice = screen",
        "strategy = full-letter-name-clips",
        "external-tts-dependency = 0",
        "runtime-os-dependency = 0",
        f"source-sha256 = {source_sha}",
        f"engine-fingerprint = {vc.engine_fingerprint()}",
        f"sample-rate = {vc.SAMPLE_RATE}",
        "sample-format = signed-16-bit-stereo",
        f"unit-count = {len(names)}",
        f"bank-bytes = {len(bank)}",
        f"bank-sha256 = {bank_sha}",
        f"worst-letter = {worst_ch}",
        f"worst-letter-bytes = {worst_clip}",
        f"worst-32-char-label-bytes = {worst_total}",
        f"dma-usable-bytes = {usable}",
        "dma-capacity-check = PASS",
    ]
    for ch in "abcdefghijklmnopqrstuvwxyz":
        lines.append(f"letter-{ch}-duration-s = {durations[ch]:.6f}")
    meta.write_text("\n".join(lines) + "\n", encoding="utf-8")

    print("UEFI_HII_VOICECORE_V4_GENERATION=PASS")
    print(f"BANK_BYTES={len(bank)}")
    print(f"WORST_LETTER={worst_ch}")
    print(f"WORST_32_CHAR_LABEL_BYTES={worst_total}")
    print(f"DMA_USABLE_BYTES={usable}")

if __name__ == "__main__":
    main()
