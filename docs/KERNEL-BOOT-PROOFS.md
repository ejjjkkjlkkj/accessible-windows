# Kernel boot proofs

This document defines what the project accepts as evidence that a kernel
subsystem works, and records the proofs that currently exist.

## The rule

A component is never PASS because it compiled, and never PASS because a flag is
set in a data structure. For anything that runs on bare metal, PASS requires an
execution under QEMU/OVMF (or on hardware) that produced an observable marker
which the CPU could only have produced by actually doing the thing.

States are kept distinct: **PASS**, **PARTIAL**, **TO PROVE**, **TO BUILD**,
**BLOCKED**. Downgrading is normal: a subsystem that was PASS returns to TO
PROVE the moment its proof stops being replayed.

## Running the proofs

```bash
pwsh -NoProfile -File scripts/verify-windows.ps1
```

Host quality gates first (fmt, check, test, clippy on the workspace, plus clippy
on the kernel crate for every feature combination it ships - the kernel is
outside the workspace, so `clippy --workspace` never sees it), then:

```bash
pwsh -NoProfile -File scripts/Invoke-BootProofs.ps1
```

`Invoke-BootProofs.ps1` owns every marker assertion. It builds each kernel
configuration, boots it on a fresh QEMU/OVMF machine (q35, TCG, `-cpu max`), and
checks two lists per configuration:

- **required** markers that must appear, and
- **forbidden** markers that must not.

The forbidden list is what makes the suite honest. A boot that reaches
`AW_NATIVE_KERNEL_IDLE` while also printing `AW_MEMORY_PROTECTION_FAIL` is a
failure. So is one that prints `AW_NATIVE_EXCEPTION` on the normal path.

`scripts/Invoke-KernelBoot.ps1` is the single-run harness underneath: one
invocation builds one configuration, boots it, and returns the captured debug
log. Evidence lands in `target/boot-evidence/<name>/`, alongside the unstripped
kernel ELF for symbol-level triage.

## Current proofs

| Subsystem | State | Evidence marker |
|---|---|---|
| Fixed-base image load | PASS | `AW_KERNEL_IMAGE_HEADER_OK base=0x200000`, `AW_NATIVE_KERNEL_LOAD_OK … mode=fixed_base` |
| GDT + segment reload | PASS | `AW_GDT_SEGMENTS_RELOADED cs=0x…08 ss=0x…10` |
| IDT, 256 vectors | PASS | `AW_IDT_VECTOR_COUNT 32/256` |
| TSS 64-bit + IST1 | PASS | `AW_TSS_LOADED tr=0x18`, `AW_TSS_IST_READY vector=8 ist=1` |
| Real #UD delivery | PASS | `AW_NATIVE_EXCEPTION vector=6 name=invalid-opcode` |
| Real #DF on IST1 | PASS | `AW_DOUBLE_FAULT_IST_OK` (fault frame lands inside the IST range) |
| Kernel-owned page tables | PASS | `AW_VMM_CR3 prev=… new=…`, `AW_VMM_ACTIVE` |
| CPU protection bits | PASS | `AW_SECURITY_ENFORCED wp=1 nx=1 smep=1 smap=1 umip=1`, `AW_SECURITY_BASELINE_OK` |
| NX (no execute from data) | PASS | `AW_MEMORY_PROTECTION_OK name=nx-execute-data error_code=0x11` |
| NX (no execute from rodata) | PASS | `AW_MEMORY_PROTECTION_OK name=nx-execute-rodata error_code=0x11` |
| W^X (no write to `.text`) | PASS | `AW_MEMORY_PROTECTION_OK name=wx-write-text error_code=0x03` |
| Guard page below #DF stack | PASS | `AW_MEMORY_PROTECTION_OK name=guard-page error_code=0x00` |
| APIC timer IRQ delivery | PASS | `AW_APIC_TIMER_FIRED`, `AW_APIC_TIMER_MONOTONIC_OK ticks>=8` |
| APIC timer negative test | PASS | `AW_APIC_TIMER_MASKED_STOPPED` then `AW_APIC_TIMER_UNMASKED_RESUMED` |
| IOAPIC / MSI device IRQs | TO BUILD | - |
| SMP / per-CPU GDT-TSS-IST | TO BUILD | - |
| Ring 3 + syscalls | TO BUILD | - |
| Physical hardware boot | TO PROVE | never run on real hardware from this tree |

Error codes above are `#PF` error codes (Intel SDM 4.7): bit 0 present, bit 1
write, bit 2 user, bit 4 instruction fetch. `0x11` is *present + instruction
fetch*, which is what NX produces - and is deliberately distinguished from
`0x10`, a fetch from a page that simply is not mapped, which would prove nothing
about NX.

## Why the proofs are shaped this way

**The APIC timer counts, and the counter is checked against masking.** A
monotonically increasing counter on its own does not distinguish a real ISR from
a polling artefact. The proof requires several deliveries, then masks
`LVT_TIMER` and requires the counter to freeze, then unmasks it and requires it
to move again.

**The memory protections fault on purpose.** Page-table flags describe an
intention; only a `#PF` with the right error code shows the CPU enforcing it.
Each probe arms a narrow expectation in the exception handler (one vector, one
page), performs the offending access, and resumes at a recovery label. A probe
that does *not* fault reports `no-fault` and fails the suite - the dangerous
outcome here is silence, not a crash.

This is also why `common_exception_entry` restores the full context and
`iretq`s rather than halting: faults have to be recoverable for the probes to
work, and user-mode fault handling will need exactly the same machinery.

## Known limitations

- The proofs run under TCG emulation. Timing-sensitive behaviour and
  vendor-specific errata are not covered; hardware validation on one AMD and one
  Intel platform remains a separate, unmet requirement.
- The identity map covers the low 4 GiB. A machine whose firmware leaves the
  kernel stack or the framebuffer above that boundary is not yet supported.
- Only the #DF emergency stack has a guard page. The bootstrap kernel still runs
  on the stack the UEFI loader handed over; guarding it requires the kernel to
  allocate and switch to its own stack first.
- The kernel image is linked non-relocatable at 2 MiB. If firmware ever owns
  that range the loader fails the boot loudly (`reason=fixed_base_unavailable`)
  rather than misloading; a relocatable or higher-half image is the long-term
  answer.
