# Roadmap

## Phase 0 - Repository bootstrap

- [x] Rust workspace
- [x] Kernel boot-data contract
- [x] Accessibility semantic primitives
- [x] Accessibility invariant tests
- [x] Architecture specification
- [x] Cross-platform x64 CI
- [x] Committed Cargo lockfiles and reproducibility gate
- [x] CodeQL, RustSec, fuzzing, coverage and dependency review
- [x] SBOM/checksum/provenance pipeline

## Phase 1 - UEFI bring-up

- [x] Add an x86-64 UEFI boot application
- [x] Build the `.efi` executable in CI
- [x] Print deterministic UEFI console diagnostics
- [x] Boot the EFI stage automatically under OVMF/QEMU
- [x] Read and validate availability of the UEFI memory map
- [x] Detect GOP and current display mode
- [x] Capture framebuffer address/size when GOP exposes direct framebuffer access
- [x] Locate the ACPI RSDP through the UEFI configuration table
- [x] Validate ACPI RSDP signature, declared length and checksums
- [x] Produce an aligned GPT disk image with a FAT32 EFI System Partition
- [x] Boot-test the generated raw disk image under OVMF/QEMU
- [x] Exit UEFI Boot Services after dropping boot-services resources
- [x] Prove post-firmware execution through debugcon in CI
- [x] Define and validate the owned kernel handoff structure
- [x] Split the post-firmware stage into a freestanding x86-64 kernel
- [x] Dynamically load the PIC flat kernel from the ESP
- [x] Execute the separate native kernel after ExitBootServices
- [x] Write directly to the framebuffer from the native kernel

## Phase 2 - Generic x64 kernel foundation

- [x] Detect AMD, Intel and unknown x86-64 CPU vendors with CPUID
- [x] Validate common APIC/SSE2/long-mode boot baseline
- [x] Detect x2APIC and invariant-TSC capabilities
- [x] Parse ACPI MCFG and pass PCIe ECAM regions to the kernel
- [x] Enumerate PCIe configuration space through ECAM
- [x] Keep PCI mechanism #1 (CF8/CFC) as a legacy fallback
- [x] Classify NVMe, AHCI, xHCI and HDA controllers by PCI class
- [x] Decode PCI I/O, MMIO32 and MMIO64 BARs
- [ ] Transfer the complete final UEFI memory map to the kernel
- [ ] Physical page allocator
- [ ] Virtual-memory manager
- [ ] GDT/IDT and exception handling
- [ ] APIC timers and interrupts
- [ ] SMP bring-up on both AMD and Intel test profiles
- [ ] Scheduler
- [ ] User/kernel privilege separation
- [ ] IPC and handle/object model

## Phase 3 - Generic physical PC minimum

- [ ] ACPI table enumeration beyond MCFG
- [ ] MADT/APIC topology parsing
- [ ] PCI bridge-aware enumeration
- [ ] NVMe controller initialization and identify
- [ ] NVMe read/write
- [ ] AHCI controller initialization
- [ ] AHCI/SATA read/write
- [ ] GPT parser/writer with safety checks
- [ ] xHCI controller initialization
- [ ] USB hub enumeration
- [ ] USB HID keyboard
- [ ] USB HID pointer baseline
- [ ] Basic framebuffer console
- [ ] ACPI power off/reboot
- [ ] Physical boot validation on at least one AMD x64 PC
- [ ] Physical boot validation on at least one Intel x64 PC

## Phase 4 - Installable generic x64 system

- [ ] Produce one AMD/Intel x64 installation image
- [ ] Accessible USB installer
- [ ] Disk selection with spoken device identity
- [ ] System partition creation
- [ ] Filesystem implementation/selection
- [ ] Install system image
- [ ] UEFI boot entry creation
- [ ] First boot from physical NVMe/SATA SSD
- [ ] Recovery environment
- [ ] Installer fallback to GOP without accelerated GPU driver

## Phase 5 - Desktop and accessibility

- [ ] Compositor
- [ ] Accessible native UI toolkit
- [ ] Semantic tree service
- [ ] Keyboard-only shell
- [ ] Speech service
- [ ] Native screen reader
- [ ] Braille transport layer
- [ ] Accessible settings, file manager and terminal
- [ ] Accessibility available in boot, installer, recovery and first boot

## Phase 6 - Networking, audio, graphics and updates

- [ ] Generic HDA controller baseline
- [ ] Common Ethernet driver families
- [ ] AMD GPU driver family
- [ ] Intel GPU driver family
- [ ] Wi-Fi architecture and first supported families
- [ ] Bluetooth architecture
- [ ] Audio stack including vendor DSP extensions later
- [ ] Network stack
- [ ] Package manager
- [ ] Atomic updates
- [ ] Rollback/snapshots

## Phase 7 - Compatibility

- [ ] Win32 ABI/API compatibility program
- [ ] Windows application test corpus
- [ ] Linux subsystem/container runtime
- [ ] Web/PWA runtime
- [ ] Optional Android compatibility research

## Definition of success

The project does not claim to exceed Windows 11 until reproducible benchmarks demonstrate improvements in selected areas such as accessibility coverage, idle resource use, recovery, update reliability, input latency and security isolation while maintaining useful application and hardware compatibility.
