#!/usr/bin/env python3
from __future__ import annotations

import argparse
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FINAL = ROOT / "boot" / "uefi-amd5800h-real-final-v1" / "os-uefi-amd5800h-real-final-v1.labproof"

REQUIRED_PASS = (
    "proof-qemu-ovmf-runtime = PASS",
    "proof-qemu-keyboard-navigation = PASS",
    "proof-qemu-hda-speech = PASS",
    "proof-vmware-workstation = PASS",
    "proof-vmware-integrated-hii-hda-speech = PASS",
    "proof-vmware-interactive-hii-navigation = PASS",
)

PHYSICAL_REQUIRED = (
    "proof-physical-native-uefi = PASS",
    "proof-physical-hii-navigation = PASS",
    "proof-physical-internal-speaker-pin = PASS",
    "proof-physical-audible-speaker = PASS",
    "proof-physical-pre-os-speaker = PASS",
)

def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--strict", action="store_true")
    args = ap.parse_args()

    text = FINAL.read_text(encoding="utf-8")
    missing_virtual = [x for x in REQUIRED_PASS if x not in text]
    missing_physical = [x for x in PHYSICAL_REQUIRED if x not in text]

    if missing_virtual:
        print("UEFI_ACCESSIBILITY_PLATFORM_VIRTUALIZATION=FAIL")
        for x in missing_virtual:
            print("MISSING=" + x)
        raise SystemExit(1)

    print("UEFI_ACCESSIBILITY_PLATFORM_VIRTUALIZATION=PASS")

    if missing_physical:
        print("UEFI_ACCESSIBILITY_PLATFORM_RELEASE=CLOSURE_BLOCKED_PHYSICAL")
        for x in missing_physical:
            print("PENDING=" + x)
        if args.strict:
            raise SystemExit(2)
        return

    print("UEFI_ACCESSIBILITY_PLATFORM_RELEASE=PASS")

if __name__ == "__main__":
    main()
