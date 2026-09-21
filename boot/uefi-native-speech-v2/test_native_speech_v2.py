#!/usr/bin/env python3
from native_speech_v2 import (
    SAMPLE_RATE,
    UNIT_SPECS,
    make_units,
    render_phrase,
    quality_metrics,
    stereo_s16le_48k,
)

def main() -> None:
    a = make_units()
    b = make_units()
    assert a == b, "speech bank must be deterministic"
    assert set(a) == set(UNIT_SPECS)

    for name, samples in a.items():
        assert samples, name
        assert all(-32768 <= s <= 32767 for s in samples), name
        m = quality_metrics(samples)
        assert m["peak"] <= 0.99, (name, m)
        assert m["dc"] < 0.08, (name, m)

    phrase = render_phrase("un", "continuer", "deux", "aide", "trois", "recuperation")
    m = quality_metrics(phrase)
    assert len(phrase) > SAMPLE_RATE
    assert 0.02 < m["rms"] < 0.85, m
    assert m["peak"] <= 0.99, m

    hda = stereo_s16le_48k(phrase)
    assert len(hda) == len(phrase) * 3 * 4
    assert len(hda) % 4 == 0

    print("OS_UEFI_NATIVE_SPEECH_V2_TEST=PASS")
    print(f"sample-rate={SAMPLE_RATE}")
    print(f"units={len(a)}")
    print(f"phrase-samples={len(phrase)}")
    print(f"phrase-rms={m['rms']:.6f}")
    print(f"phrase-peak={m['peak']:.6f}")
    print(f"phrase-dc={m['dc']:.6f}")
    print(f"hda-bytes={len(hda)}")

if __name__ == "__main__":
    main()
