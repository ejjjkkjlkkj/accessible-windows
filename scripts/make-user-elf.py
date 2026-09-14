#!/usr/bin/env python3
"""Emit a minimal static x86-64 ELF for the userland-loader boot proof.

The program is position-dependent and tiny: it makes SYS_REPORT(0xC0DE) then
SYS_EXIT, both through `syscall`. The kernel's loader reads this file from the
FAT16 disk, maps its single PT_LOAD segment as user pages, and runs it at CPL3;
seeing 0xC0DE come back through SYS_REPORT proves the on-disk program ran.

Kept deliberately hand-built (no toolchain) so the proof has no build dependency
beyond Python, matching how the other on-disk test fixtures are produced.

Usage: make-user-elf.py OUT.ELF
"""

import struct
import sys

# Must match ring3.rs: SYS_REPORT = 3, SYS_EXIT = 0xff, EXPECTED_REPORT = 0xC0DE,
# and the load base LOADER expects the program at.
SYS_REPORT = 3
SYS_EXIT = 0xFF
REPORT_VALUE = 0xC0DE
BASE = 0x4_0000_0000

EHDR_SIZE = 64
PHDR_SIZE = 56
CODE_OFFSET = EHDR_SIZE + PHDR_SIZE  # single program header, then code

# mov rax, SYS_REPORT ; mov rdi, REPORT_VALUE ; syscall
# mov rax, SYS_EXIT   ; syscall
code = bytes(
    [0x48, 0xC7, 0xC0, *struct.pack("<I", SYS_REPORT)]      # mov rax, imm32
    + [0x48, 0xC7, 0xC7, *struct.pack("<I", REPORT_VALUE)]  # mov rdi, imm32
    + [0x0F, 0x05]                                           # syscall
    + [0x48, 0xC7, 0xC0, *struct.pack("<I", SYS_EXIT)]      # mov rax, imm32
    + [0x0F, 0x05]                                           # syscall
)

total = CODE_OFFSET + len(code)
entry = BASE + CODE_OFFSET

ehdr = struct.pack(
    "<16sHHIQQQIHHHHHH",
    b"\x7fELF\x02\x01\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00",  # e_ident
    2,           # e_type = ET_EXEC
    0x3E,        # e_machine = x86-64
    1,           # e_version
    entry,       # e_entry
    EHDR_SIZE,   # e_phoff
    0,           # e_shoff
    0,           # e_flags
    EHDR_SIZE,   # e_ehsize
    PHDR_SIZE,   # e_phentsize
    1,           # e_phnum
    0,           # e_shentsize
    0,           # e_shnum
    0,           # e_shstrndx
)

phdr = struct.pack(
    "<IIQQQQQQ",
    1,          # p_type = PT_LOAD
    5,          # p_flags = R + X
    0,          # p_offset (segment covers the whole file, headers included)
    BASE,       # p_vaddr
    BASE,       # p_paddr
    total,      # p_filesz
    total,      # p_memsz
    0x1000,     # p_align
)

image = ehdr + phdr + code
assert len(image) == total, (len(image), total)

with open(sys.argv[1], "wb") as handle:
    handle.write(image)
print(f"wrote {sys.argv[1]}: {total} bytes (entry={entry:#x})")
