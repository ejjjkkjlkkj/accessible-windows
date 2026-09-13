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
| MADT parse + ISA IRQ override | PASS | `AW_IOAPIC_ROUTED isa_irq=0 gsi=2` (the override, not the IRQ number) |
| I/O APIC device IRQ delivery | PASS | `AW_IOAPIC_IRQ_FIRED`, `AW_IOAPIC_IRQ_MONOTONIC_OK ticks>=8` |
| I/O APIC negative test | PASS | `AW_IOAPIC_MASKED_STOPPED` then `AW_IOAPIC_UNMASKED_RESUMED` |
| MSI delivery | PASS | `AW_MSI_FIRED`, `AW_MSI_MONOTONIC_OK ticks>=8` (`msi-smoke`) |
| MSI negative test | PASS | `AW_MSI_MASKED_STOPPED` then `AW_MSI_UNMASKED_RESUMED` |
| MSI-X | TO BUILD | - |
| INTx routing through ACPI `_PRT` | TO BUILD | - |
| SMP bring-up (INIT-SIPI-SIPI) | PASS | `AW_SMP_ONLINE online=3 started=3` (`smp`, `-smp 4`) |
| Per-CPU GDT, TSS and IST | PASS | `AW_SMP_PER_CPU_TABLES_OK cpus=3` (distinct GDT/TSS/IST1 per CPU) |
| AP identity | PASS | `AW_SMP_AP_ONLINE apic_id=N requested=N`, read by the AP from its own APIC |
| Per-CPU state via `GS` | PASS | `AW_PERCPU_PROOF_OK cpus=4` (`smp`); each CPU's block reached through `gs:[0]`, index/APIC id distinct per CPU |
| Per-CPU interrupt counters | PASS | `AW_PERCPU_BSP ... timer_ticks>=8 device_ticks>=8`, counted into the block of the CPU that ran the ISR |
| Per-CPU timer armed on an AP | TO PROVE | APs park with interrupts masked; only the bootstrap processor's per-CPU counters advance |
| Per-CPU #DF on an AP's IST | TO PROVE | only the bootstrap processor's IST is proved by a real fault |
| Scheduler / anything running on an AP | TO BUILD | APs park in `hlt` |
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

**A device interrupt is proved on a device, not on the local APIC.** The timer
proof shows the CPU taking an interrupt the CPU itself generated, which says
nothing about the path a peripheral uses. The I/O APIC proof drives the 8254,
so the interrupt has to leave a device, cross a redirection entry, and arrive on
the vector that entry names. The pin is not assumed either: the MADT's interrupt
source overrides are applied, and the log records `isa_irq=0 gsi=2` - the case
where taking the IRQ number for the global system interrupt number would have
silently programmed the wrong pin.

**The MSI proof pokes the device throughout the masked window.** A periodic
timer keeps firing on its own, so masking it and watching the counter freeze is
enough. A device only fires when asked, so a frozen counter would prove nothing
if nobody were asking. The proof therefore keeps requesting interrupts for the
whole masked window: the counter staying still means the device's own MSI enable
bit suppressed interrupts that were actively being requested.

The `msi-smoke` configuration is the only one that builds a driver for QEMU's
`edu` device, behind the `msi-proof-device` feature. It exists because `edu` can
be asked to raise an interrupt without first implementing a real controller's
command protocol; everything it exercises - the capability walk, the message
encoding, bus mastering, the vector plumbing - is what an NVMe or xHCI driver
will use unchanged.

**An application processor is online only if it says so itself.** A counter the
bootstrap processor increments after sending a SIPI proves that a SIPI was sent.
Each AP instead reports the APIC ID it read from *its own* local APIC, plus the
GDT, TSS and IST1 addresses it actually loaded, and the suite requires those to
match the CPU that was asked for and to differ from every other CPU's. Sharing a
TSS between two CPUs is not a subtle bug - the busy bit `ltr` sets makes the
second `ltr` a `#GP`, and two CPUs faulting onto one IST stack corrupt each
other - so "the tables are private" is checked rather than assumed.

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
- Application processors park in `hlt` with interrupts masked, on a fixed
  bound of 8 CPUs. Their #DF stacks have a guard *page* but not a guard
  *hole*: the kernel's page tables were built before those stacks existed, so
  an AP stack overflow is currently silent where the bootstrap processor's
  faults. An AP is also never sent an interrupt, so its IDT is loaded but
  unexercised.
- MSI is proved on one emulated device with one vector. MSI-X, multiple
  vectors per device, and per-vector masking are untouched, as is INTx routing
  through the ACPI `_PRT` - which needs an AML interpreter, so a device without
  MSI cannot currently be routed at all.
- The kernel image is linked non-relocatable at 2 MiB. If firmware ever owns
  that range the loader fails the boot loudly (`reason=fixed_base_unavailable`)
  rather than misloading; a relocatable or higher-half image is the long-term
  answer.
