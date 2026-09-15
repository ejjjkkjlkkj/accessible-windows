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
| Persistent frame allocator | PASS | `AW_FRAME_ALLOCATOR_OK`; one owner backs the page tables and every later mapping |
| Runtime map/unmap (live tables) | PASS | `AW_VMM_MAP_OK` → `AW_VMM_MAP_READBACK_OK` → `AW_VMM_MAP_TRANSLATE_OK` → `AW_VMM_UNMAP_OK` |
| Runtime unmap negative test | PASS | `AW_VMM_UNMAP_FAULT_OK`; the unmapped address faults not-present after the TLB shootdown |
| Kernel heap / global allocator | PASS | `AW_HEAP_PROOF_OK`; `Box`/`Vec` allocate, a vector grows and sums, a freed block is reused, over-aligned allocations align |
| virtio-blk device read (legacy) | PASS | `AW_VIRTIO_BLK_PROOF_OK` (`virtio-blk`); one virtqueue reads sector 0 and it is a FAT boot sector |
| AHCI/SATA sector read (DMA) | PASS | `AW_AHCI_PROOF_OK` (`ahci`); brings up an AHCI HBA, finds the port with a disk, and reads LBA 0 by DMA (READ DMA EXT), checking the 0x55AA signature - the real controller model VMware and physical PCs use |
| AHCI/SATA sector write (DMA) | PASS | `AW_AHCI_WRITE_PROOF_OK` (`ahci-write`); writes a known pattern to LBA 0 of a dedicated scratch disk by DMA (WRITE DMA EXT) and reads it back byte-for-byte - never run against a data disk |
| GPT partition table read | PASS | `AW_GPT_PROOF_OK` (`gpt`); reads the GPT header off an AHCI disk, validates the `EFI PART` signature and header CRC32, walks the entry array and identifies the first partition (the ESP by type GUID) - read-only, so it also runs on VMware's real GPT boot disk |
| GPT partition table write | PASS | `AW_GPTWRITE_PROOF_OK` (`gpt-write`); on a blank 8 MiB scratch disk, write a protective MBR, a primary and backup GPT header (each with its own CRC32) and an entry array with one ESP, then read the table back and validate both headers' CRCs and the ESP entry - and the independent reader then validates the same table (`AW_GPT_PROOF_OK`, ESP at LBA 34). The installer's partitioning half, gated so it never touches a data disk |
| File read through the full storage stack | PASS | `AW_FSPART_PROOF_OK` (`gpt`); reads a file from the ESP's own FAT16 filesystem at the partition offset the GPT reported - AHCI -> GPT -> partition -> FAT -> file, the way an installed system's own files are reached |
| FAT16 file read | PASS | `AW_FS_PROOF_OK` (`virtio-blk`); parse the BPB, find `HELLO.TXT`, follow its cluster chain, match the bytes |
| FAT16 file write | PASS | `AW_FATWRITE_PROOF_OK` (`fat-write`); on a dedicated scratch FAT16 disk over AHCI, create `NEWFILE.TXT` - allocate a free cluster, write the data, chain the FAT in every copy, add a root-directory entry - then read it back through the ordinary reader and match the bytes (`AW_FATWRITE_READBACK_OK`); the installer foundation, gated so it never touches a data disk |
| FAT16 format (mkfs) | PASS | `AW_MKFS_PROOF_OK` (`mkfs`); write a fresh empty FAT16 volume onto a blank scratch disk - boot sector/BPB, two FATs with their reserved entries, a zeroed root directory (cluster count computed into the FAT16 range) - then create a file in it and read it back (`AW_MKFS_READBACK_OK`); with the GPT writer, the kernel can build a whole disk from blank. Gated, scratch disk only |
| Disk build (installer capstone) | PASS | `AW_DISKBUILD_PROOF_OK` (`disk-build`); on one blank 8 MiB scratch disk the kernel writes a GPT, re-reads it to find the ESP (LBA 34), formats that partition as FAT16, writes `INSTALL.TXT`, then reads it back through the whole stack it just built - GPT -> partition -> FAT -> file (`AW_DISKBUILD_READBACK_OK`). This is what an installer does to provision a target disk, proven end to end from blank. Gated, scratch disk only |
| Userland ELF loader | PASS | `AW_USER_LOADER_PROOF_OK` (`virtio-blk`); read `USERPROG.ELF` off the FAT16 disk, map its PT_LOAD segment as user pages, run it at CPL3, and see it report 0xC0DE through a syscall and exit |
| Userland preemptive multitasking | PASS | `AW_USER_INIT_PROOF_OK` (`virtio-blk`); two spinner programs (`USERA.ELF`/`USERB.ELF`) loaded from disk, each on its own kernel stack, are preempted back and forth by the timer at CPL3 and their counters advance comparably - fair alternation, not one starved |
| virtio-net ARP exchange (legacy) | PASS | `AW_VIRTIO_NET_PROOF_OK` (`net`); reads its MAC from config, `AW_VIRTIO_NET_QUIET_OK` shows the receive ring idle while nothing is sent, then an ARP request draws the SLIRP gateway's reply (`spa=10.0.2.2`) |
| virtio-net ICMP echo (IPv4) | PASS | `AW_VIRTIO_NET_ICMP_PROOF_OK` (`net`); one layer up from ARP - a routed IPv4 datagram with its own header checksum carries an ICMP echo request to the gateway, addressed on the wire to the hardware address ARP just resolved, and the gateway's echo reply comes back with our exact identifier, sequence and 16-byte payload (`AW_VIRTIO_NET_ICMP_REPLY_OK`) |
| 16550 serial console (COM1) | PASS | `AW_SERIAL_PROOF_OK` (`serial`); loopback self-test, then a banner appears in the host COM1 log |
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
| Per-CPU timer armed on an AP | PASS | `AW_PERCPU_AP_TIMER_OK aps=3` (`smp`); each AP arms its own Local APIC timer, idles under `sti`/`hlt`, and its own block's `timer_ticks` advances |
| Per-CPU #DF on an AP's IST | PASS | `AW_DOUBLE_FAULT_IST_OK` (`ap-double-fault-smoke`); an application processor forces a real #DF and the fault frame lands inside *that CPU's own* IST1 range, range-checked against the per-CPU block's bounds (not the bootstrap processor's) |
| Scheduler / anything running on an AP | PASS | `AW_AP_SCHED_PROOF_OK cpu=1 threads=2` (`ap-scheduler-smoke`); an application processor runs two kernel threads that context switch and take turns, before going on to report online and idle under its own timer - it no longer only parks in `hlt` |
| Ring 3 entry (`iretq` to CPL3) | PASS | `AW_RING3_PROOF_OK`; a mapped user page runs at CPL3, its syscall's saved RIP lands inside the user page |
| Versioned syscall ABI (`sysret`) | PASS | `AW_SYSCALL_ABI_PROOF_OK version=1`; dispatch table, `SYS_ADD`=5, `SYS_EXIT`, each returning via `sysret` |
| Validated user-pointer copy | PASS | `AW_SYSCALL_WRITE copied=13`; `SYS_WRITE` bounds-checks the user pointer and copies it in with SMAP `stac`/`clac` |
| Cooperative scheduler / threads | PASS | `AW_SCHED_PROOF_OK threads=3 switches=29`; three kernel threads context switch and take ten turns each |
| Monotonic TSC clock | PASS | `AW_CLOCK_PROOF_OK`; TSC monotonic, frequency calibrated against a PIT channel-2 one-shot |
| CMOS RTC wall clock | PASS | `AW_RTC_PROOF_OK` (`normal`); reads the date/time from the CMOS RTC over ports 0x70/0x71, waiting out any update-in-progress and accepting only a stable reading, decodes BCD/12-hour per Status Register B, and sanity-checks a plausible calendar date - read-only, so it runs on the normal boot path (and on VMware) |
| Preemptive scheduling | PASS | `AW_PREEMPT_PROOF_OK threads=3 switches=12`; three kernel threads that never yield are switched by the timer interrupt alone (each advances a counter), in exactly twelve timer-driven context switches, and the timer proof either side still passes |
| Ring 3 preemption / user scheduler | PASS | `AW_RING3_PREEMPT_PROOF_OK`; a CPL3 user thread that never makes a syscall spins on a counter with interrupts enabled, the timer preempts it (conditional `swapgs` keeps the per-CPU GS correct), and after six timer-driven switches the kernel takes control back on its own |
| User-page `swapgs` on entry | PASS | `AW_SWAPGS_PROOF_OK`; CPL3 runs on a distinct user `GS` base, and the syscall entry's `swapgs` makes `gs:[0]` reach this CPU's real per-CPU block (null if the swap were missing) |
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

**The virtio-net proof makes the reply attributable to the request.** A card both
consumes and produces buffers, so a frame appearing in the receive ring proves
nothing unless it can be tied to something the guest did. The `net` proof posts
its receive buffers, then holds a window in which it sends nothing and requires
the receive ring to stay empty (`AW_VIRTIO_NET_QUIET_OK`) - the same shape as the
MSI masked window. Only then does it broadcast an ARP request, and it accepts the
result only if a reply comes back with opcode 2 and sender address 10.0.2.2, the
SLIRP gateway that could only answer because the request really left the guest.

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

## Second hypervisor: VMware Workstation

Every proof above runs under QEMU/OVMF. The dossier (sections 3.1 and 20) also
asks for validation on other hypervisors on the way to real hardware.
`scripts/Invoke-VMwareBoot.ps1` boots the same `dist` image under **VMware
Workstation** - its own EFI firmware and a virtual SATA controller, a completely
different firmware and device model than QEMU/OVMF - with COM1 routed to a file:

```bash
pwsh -NoProfile -File scripts/Invoke-VMwareBoot.ps1
```

| Subsystem | State | Evidence marker |
|---|---|---|
| Full clean boot on VMware EFI | PASS | `AW_VMWARE_BOOT_OK`; the native kernel reaches `AW_NATIVE_KERNEL_IDLE` on VMware with no `AW_NATIVE_EXCEPTION`/`AW_NATIVE_KERNEL_PANIC`, and the serial banner appears exactly once (no crash-reboot loop) - VMware's firmware booted `\EFI\BOOT\BOOTX64.EFI`, the loader handed off, and every native proof (W^X, Ring 3, swapgs, scheduler, APIC timer, clock, PCIe scan) ran on it |

0xE9 debugcon is a QEMU/Bochs convenience VMware does not have, so `debug_write`
mirrors every marker onto the real 16550 once `serial::prove` confirms it, and the
whole boot is captured on the guest COM1.

**What it took.** The first attempt reboot-looped right after the serial banner.
With the markers mirrored to COM1 the crash was pinned to the CR3 switch: the
kernel's identity map marks every non-code page NX, but the NX bit is only valid
with `EFER.NXE` set. QEMU/OVMF leaves it on; VMware's EFI leaves it off, so NX was
a reserved bit and the first stack access on the new map raised a reserved-bit
`#PF` and triple-faulted. The kernel now enables `EFER.NXE` itself before the map
goes live rather than trusting the firmware - a portability fix that matters for
real hardware too, not just VMware. The `physical=45 linear=48` address widths and
`vendor=amd` in the capture confirm this is a genuinely different CPU model than
the QEMU runs.
