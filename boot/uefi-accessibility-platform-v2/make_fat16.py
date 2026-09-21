#!/usr/bin/env python3
from pathlib import Path
import struct
import sys

src = Path(sys.argv[1]).read_bytes()
out = Path(sys.argv[2])
size_mib = int(sys.argv[3]) if len(sys.argv) >= 4 else 64
if size_mib < 16 or size_mib > 512:
    raise SystemExit("size MiB must be between 16 and 512")
total = size_mib * 1024 * 1024 // 512
start = 2048
psecs = total - start
bps = 512
spc = 4 if size_mib <= 128 else 8
reserved = 1
nfats = 2
roots = 512
rootsecs = (roots * 32 + bps - 1) // bps
spf = 1
while True:
    clusters = (psecs - reserved - rootsecs - nfats * spf) // spc
    need = ((clusters + 2) * 2 + bps - 1) // bps
    if need == spf:
        break
    spf = need

if clusters < 4085 or clusters > 65524:
    raise SystemExit(f"invalid FAT16 cluster count: {clusters}")
data_start = start + reserved + nfats * spf + rootsecs
clbytes = bps * spc
file_clusters = (len(src) + clbytes - 1) // clbytes
if 4 + file_clusters >= clusters:
    raise SystemExit("image too small")

img = bytearray(total * bps)
img[0x1BE] = 0
img[0x1BF:0x1C2] = b"\xfe\xff\xff"
img[0x1C2] = 0x06
img[0x1C3:0x1C6] = b"\xfe\xff\xff"
struct.pack_into("<II", img, 0x1C6, start, psecs)
img[510:512] = b"\x55\xaa"

bs = start * bps
img[bs:bs+3] = b"\xeb\x3c\x90"
img[bs+3:bs+11] = b"MSDOS5.0"
struct.pack_into("<HBHBHHBHHHII", img, bs+11, bps, spc, reserved, nfats, roots, 0, 0xF8, spf, 63, 255, start, psecs)
img[bs+36] = 0x80
img[bs+38] = 0x29
struct.pack_into("<I", img, bs+39, 0x56433431)
img[bs+43:bs+54] = b"VOICECORE4  "
img[bs+54:bs+62] = b"FAT16   "
img[bs+510:bs+512] = b"\x55\xaa"

fat = bytearray(spf * bps)
def setfat(n: int, v: int) -> None:
    struct.pack_into("<H", fat, n * 2, v)

setfat(0, 0xFFF8)
setfat(1, 0xFFFF)
setfat(2, 0xFFFF)
setfat(3, 0xFFFF)
first = 4
for i in range(file_clusters):
    n = first + i
    setfat(n, 0xFFFF if i == file_clusters - 1 else n + 1)

for k in range(nfats):
    off = (start + reserved + k * spf) * bps
    img[off:off+len(fat)] = fat

root = (start + reserved + nfats * spf) * bps
def ent(off: int, name: bytes, attr: int, cluster: int, size: int = 0) -> None:
    img[off:off+11] = name
    img[off+11] = attr
    struct.pack_into("<H", img, off+26, cluster)
    struct.pack_into("<I", img, off+28, size)

ent(root, b"EFI        ", 0x10, 2)

def coff(n: int) -> int:
    return (data_start + (n - 2) * spc) * bps

e = coff(2)
ent(e, b".          ", 0x10, 2)
ent(e+32, b"..         ", 0x10, 0)
ent(e+64, b"BOOT       ", 0x10, 3)

b = coff(3)
ent(b, b".          ", 0x10, 3)
ent(b+32, b"..         ", 0x10, 2)
ent(b+64, b"BOOTX64 EFI", 0x20, first, len(src))

pos = 0
for i in range(file_clusters):
    off = coff(first + i)
    chunk = src[pos:pos+clbytes]
    img[off:off+len(chunk)] = chunk
    pos += len(chunk)

out.parent.mkdir(parents=True, exist_ok=True)
out.write_bytes(img)
print(f"FAT16_UEFI_DISK=PASS bytes={len(img)} size_mib={size_mib} spc={spc} clusters={file_clusters}")
