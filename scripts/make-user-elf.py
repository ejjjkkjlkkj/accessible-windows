#!/usr/bin/env python3
"""Emit a minimal static x86-64 ELF for the userland boot proofs.

Two tiny, hand-built programs (no toolchain, so the proofs have no build
dependency beyond Python, matching the other on-disk fixtures):

  * default: SYS_REPORT(0xC0DE) then SYS_EXIT - for the single-program loader
    proof. The kernel seeing 0xC0DE come back proves the on-disk program ran.
  * --spinner: increment a counter at BASE+0x1000 forever - for the init proof
    that preemptively schedules several userland programs. The kernel maps the
    counter page, runs the program at CPL3, and checks the counter advanced.

Usage:
  make-user-elf.py OUT.ELF [--base HEX]
  make-user-elf.py OUT.ELF --spinner --base HEX
"""

import struct
import sys

# Must match ring3.rs: SYS_REPORT = 3, SYS_EXIT = 0xff, EXPECTED_REPORT = 0xC0DE.
SYS_REPORT = 3
SYS_EXIT = 0xFF
REPORT_VALUE = 0xC0DE

EHDR_SIZE = 64
PHDR_SIZE = 56
CODE_OFFSET = EHDR_SIZE + PHDR_SIZE  # single program header, then code

# Counter page the init proof maps read-write, relative to the program's base.
WORK_OFFSET = 0x1000


def report_code():
    # mov rax, SYS_REPORT ; mov rdi, REPORT_VALUE ; syscall ; mov rax, SYS_EXIT ; syscall
    return bytes(
        [0x48, 0xC7, 0xC0, *struct.pack("<I", SYS_REPORT)]
        + [0x48, 0xC7, 0xC7, *struct.pack("<I", REPORT_VALUE)]
        + [0x0F, 0x05]
        + [0x48, 0xC7, 0xC0, *struct.pack("<I", SYS_EXIT)]
        + [0x0F, 0x05]
    )


def spinner_code(base):
    work = base + WORK_OFFSET
    # mov rax, imm64(work) ; loop: inc qword [rax] ; jmp loop
    return bytes(
        [0x48, 0xB8, *struct.pack("<Q", work)]
        + [0x48, 0xFF, 0x00]
        + [0xEB, 0xFB]
    )


def main():
    out = sys.argv[1]
    args = sys.argv[2:]
    spinner = "--spinner" in args
    base = 0x4_0000_0000
    if "--base" in args:
        base = int(args[args.index("--base") + 1], 0)

    code = spinner_code(base) if spinner else report_code()
    total = CODE_OFFSET + len(code)
    entry = base + CODE_OFFSET

    ehdr = struct.pack(
        "<16sHHIQQQIHHHHHH",
        b"\x7fELF\x02\x01\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00",
        2, 0x3E, 1, entry, EHDR_SIZE, 0, 0, EHDR_SIZE, PHDR_SIZE, 1, 0, 0, 0,
    )
    phdr = struct.pack(
        "<IIQQQQQQ",
        1,      # PT_LOAD
        5,      # R + X
        0, base, base, total, total, 0x1000,
    )
    image = ehdr + phdr + code
    assert len(image) == total, (len(image), total)
    with open(out, "wb") as handle:
        handle.write(image)
    kind = "spinner" if spinner else "report"
    print(f"wrote {out}: {total} bytes ({kind}, base={base:#x}, entry={entry:#x})")


if __name__ == "__main__":
    main()
