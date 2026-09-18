#!/usr/bin/env python3
from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CURRENT_PROOF = ROOT / "boot" / "uefi-hii-buffer-current-v1" / "os-uefi-hii-buffer-current-v1.labproof"
FIXTURE_PROOF = ROOT / "boot" / "uefi-hii-iscsi-fixture-v1" / "os-uefi-hii-iscsi-fixture-v1.labproof"


def parse(path: Path) -> dict[str, str]:
    values: dict[str, str] = {}
    for raw in path.read_text(encoding="utf-8").splitlines():
        if " = " not in raw:
            continue
        key, value = raw.split(" = ", 1)
        values[key.strip()] = value.strip()
    return values


def require(values: dict[str, str], key: str, expected: str) -> None:
    actual = values.get(key)
    if actual != expected:
        raise SystemExit(f"{key}: expected {expected!r}, got {actual!r}")


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit("usage: resolve_current_option.py OUTPUT")

    current = parse(CURRENT_PROOF)
    fixture = parse(FIXTURE_PROOF)

    require(current, "proof", "PASS")
    require(current, "proof-buffer-current-value", "PASS")
    require(current, "proof-buffer-current-identity-match", "PASS")
    require(current, "proof-iscsi-fixture-identity-match", "PASS")
    require(current, "proof-route-config-call", "false")
    require(current, "proof-runtime-variable-read", "false")
    require(current, "proof-set-variable-call", "false")
    require(current, "proof-varstore-write", "false")
    require(current, "proof-setting-mutation", "false")
    require(fixture, "proof", "PASS")

    identity_pairs = {
        "question-text": "proof-question-text",
        "varstore-id": "proof-varstore-id",
        "varstore-offset": "proof-varstore-offset",
        "varstore-size": "proof-varstore-size",
        "varstore-guid-raw-hex": "proof-varstore-guid-raw-hex",
    }
    for fixture_key, current_key in identity_pairs.items():
        if fixture.get(fixture_key) != current.get(current_key):
            raise SystemExit(
                f"identity mismatch {fixture_key}: "
                f"{fixture.get(fixture_key)!r} != {current.get(current_key)!r}"
            )

    mappings: dict[int, str] = {}
    for stem in ("disabled", "enabled", "enabled-for-mpio"):
        value = int(fixture[f"option-{stem}-value"], 10)
        label = fixture[f"option-{stem}-label"]
        if value in mappings:
            raise SystemExit(f"duplicate ONE_OF value: {value}")
        mappings[value] = label

    current_value = int(current["proof-current-value-unsigned"], 10)
    if current_value not in mappings:
        raise SystemExit(f"current ONE_OF value has no independently verified label: {current_value}")
    label = mappings[current_value]
    spoken_prefix = "".join(ch.lower() for ch in label if "a" <= ch.lower() <= "z")[:8]
    if not spoken_prefix:
        raise SystemExit("resolved option has no speakable Latin prefix")

    output = Path(sys.argv[1])
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(
        "\n".join(
            (
                "OS-UEFI-HII-CURRENT-OPTION-V1",
                f"question-text = {current['proof-question-text']}",
                f"question-id = {current['proof-question-id']}",
                f"varstore-id = {current['proof-varstore-id']}",
                f"varstore-offset = {current['proof-varstore-offset']}",
                f"current-width = {current['proof-current-width']}",
                f"current-value = {current_value}",
                f"resolved-current-option = {label}",
                f"spoken-prefix = {spoken_prefix}",
                "mapping-source = independent EDK II iSCSI fixture",
                "route-config-call = false",
                "runtime-variable-read = false",
                "set-variable-call = false",
                "varstore-write = false",
                "setting-mutation = false",
                "result = PASS",
            )
        )
        + "\n",
        encoding="utf-8",
    )
    print("HII_CURRENT_OPTION_IDENTITY=PASS")
    print("HII_CURRENT_OPTION_MAPPING=PASS")
    print(f"HII_CURRENT_OPTION_VALUE={current_value}")
    print(f"HII_CURRENT_OPTION_LABEL={label}")
    print(f"HII_CURRENT_OPTION_SPOKEN_PREFIX={spoken_prefix}")


if __name__ == "__main__":
    main()
