#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import importlib.util
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT/'boot'/'uefi-native-speech-v1'/'build_uefi_native_speech.py'
LETTER_UNITS = {
    'a':('a',), 'b':('b','e'), 'c':('s','e'), 'd':('d','e'), 'e':('e',),
    'f':('e','f'), 'g':('sh','e'), 'h':('a','sh'), 'i':('i',), 'j':('sh','i'),
    'k':('k','a'), 'l':('e','l'), 'm':('e','m'), 'n':('e','n'), 'o':('o',),
    'p':('p','e'), 'q':('k','u'), 'r':('e','r'), 's':('e','s'), 't':('t','e'),
    'u':('u',), 'v':('v','e'), 'w':('d','u','b','l','e','v','e'),
    'x':('i','k','s'), 'y':('i','g','r','e','k'), 'z':('z','e','d'),
}

def load_source():
    spec=importlib.util.spec_from_file_location('qevarynx_native_speech_source',SOURCE)
    if spec is None or spec.loader is None:
        raise SystemExit('cannot load native speech source')
    module=importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module

def convert(raw: bytes) -> bytes:
    out=bytearray()
    for sample in raw:
        signed=max(-32768,min(32767,(sample-128)*180))
        frame=struct.pack('<hh',signed,signed)
        out += frame*6
    return bytes(out)

def emit_u8(name: str, vals: list[int], cols: int=24) -> str:
    rows=[]
    for i in range(0,len(vals),cols):
        rows.append('    '+', '.join(str(x) for x in vals[i:i+cols])+',')
    return f'const unsigned char {name}[] = {{\n'+'\n'.join(rows)+'\n};\n'

def emit_u32(name: str, vals: list[int], cols: int=8) -> str:
    rows=[]
    for i in range(0,len(vals),cols):
        rows.append('    '+', '.join(f'{x}u' for x in vals[i:i+cols])+',')
    return f'const unsigned int {name}[] = {{\n'+'\n'.join(rows)+'\n};\n'

def main() -> int:
    if len(sys.argv)!=3:
        raise SystemExit('usage: generate_hii_graph_speech_units.py OUTPUT_C METADATA')
    out_c=Path(sys.argv[1]); metadata=Path(sys.argv[2])
    speech=load_source(); units=speech.make_units()
    names=sorted({u for seq in LETTER_UNITS.values() for u in seq})
    converted={name:convert(units[name]) for name in names}
    offsets=[]; lengths=[]; bank=bytearray(); index={}
    for i,name in enumerate(names):
        index[name]=i; offsets.append(len(bank)); lengths.append(len(converted[name])); bank+=converted[name]
    max_seq=max(len(v) for v in LETTER_UNITS.values())
    counts=[]; table=[]
    for ch in 'abcdefghijklmnopqrstuvwxyz':
        seq=LETTER_UNITS[ch]; counts.append(len(seq))
        row=[index[u] for u in seq] + [0]*(max_seq-len(seq)); table.extend(row)
    bank_text=',\n'.join('    '+', '.join(f'0x{x:02x}' for x in bank[i:i+16]) for i in range(0,len(bank),16))
    source=(
        '/* Deterministic first-party alphabet allophone bank. */\n'
        f'const unsigned char qev_unit_bank[] = {{\n{bank_text}\n}};\n'
        f'const unsigned int qev_unit_bank_len = {len(bank)}u;\n\n'
        + emit_u32('qev_unit_offsets',offsets)
        + emit_u32('qev_unit_lengths',lengths)
        + f'const unsigned int qev_unit_count = {len(names)}u;\n'
        + emit_u8('qev_letter_count',counts)
        + emit_u8('qev_letter_units',table)
        + f'const unsigned int qev_letter_stride = {max_seq}u;\n'
    )
    out_c.write_text(source)
    metadata.write_text(
        'OS-UEFI-HII-GRAPH-SPEECH-UNITS-V1\n'
        'source=boot/uefi-native-speech-v1/build_uefi_native_speech.py\n'
        f'source-sha256={hashlib.sha256(SOURCE.read_bytes()).hexdigest()}\n'
        'alphabet=a-z\n'
        f'unit-count={len(names)}\n'
        f'letter-stride={max_seq}\n'
        f'bank-bytes={len(bank)}\n'
        f'bank-sha256={hashlib.sha256(bank).hexdigest()}\n'
        'full-utterance-asset=false\n'
        'runtime-source=live-HII-prompt\n'
    )
    print('HII_GRAPH_SPEECH_UNIT_GENERATION=PASS')
    print(f'UNIT_COUNT={len(names)}')
    print(f'BANK_BYTES={len(bank)}')
    print(f'LETTER_STRIDE={max_seq}')
    return 0

if __name__=='__main__':
    raise SystemExit(main())
