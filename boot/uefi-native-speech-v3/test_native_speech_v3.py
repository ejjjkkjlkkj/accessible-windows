#!/usr/bin/env python3
from __future__ import annotations
import hashlib
from native_speech_v3 import SAMPLE_RATE, PHONEMES, VOICES, normalize_text, text_to_phonemes, synthesize, pcm_s16le_stereo, quality_metrics

def sha(samples):
    return hashlib.sha256(pcm_s16le_stereo(samples)).hexdigest()

def main():
    assert SAMPLE_RATE == 48000
    assert len(PHONEMES) >= 30
    assert {"clair","velours","screen","grave"} <= set(VOICES)
    n=normalize_text("UEFI 2026, erreur!")
    assert "u e f i" in n and "deux zéro deux six" in n
    ph=text_to_phonemes("Bonjour, lecteur d'écran.")
    assert len(ph)>10 and "ɔ̃" in ph and "ʁ" in ph
    phrase="Bonjour. UEFI, menu, continuer, récupération, erreur."
    outputs={}
    for voice in VOICES:
        a=synthesize(phrase,voice); b=synthesize(phrase,voice)
        assert a==b, f"{voice}: nondeterministic"; assert len(a)>SAMPLE_RATE
        m=quality_metrics(a); assert .008<m["rms"]<.75,(voice,m); assert m["peak"]<=.95,(voice,m); assert m["dc"]<.01,(voice,m); assert m["clip_ratio"]==0.0,(voice,m)
        pcm=pcm_s16le_stereo(a); assert len(pcm)==len(a)*4 and len(pcm)%4==0
        outputs[voice]=sha(a); print(f"{voice}: duration={m['duration_s']:.3f}s rms={m['rms']:.6f} peak={m['peak']:.6f} sha256={outputs[voice]}")
    assert len(set(outputs.values()))==len(outputs),outputs
    arbitrary=synthesize("Navigation système Windows, audio USB, PCI, HDA, écran 48.","screen")
    assert len(arbitrary)>SAMPLE_RATE
    print("VOICECORE_V3_TEST=PASS")

if __name__=="__main__":
    main()
