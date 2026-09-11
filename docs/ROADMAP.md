# Roadmap

## Phase 0 - Repository bootstrap

- [x] Rust workspace
- [x] Kernel boot-data contract
- [x] Accessibility semantic primitives
- [x] Accessibility invariant tests
- [x] Architecture specification
- [x] Cross-platform CI
- [x] Define x64-only architecture scope
- [x] Define one generic AMD/Intel hardware-compatibility model

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
- [x] Split the post-firmware stage into the freestanding x86-64 kernel crate

## Phase 2 - Generic x64 kernel foundation

- [ ] CPUID-based CPU feature detection
- [ ] Detect AMD vs Intel without making either vendor mandatory
- [ ] Common x86-64 feature policy and safe fallbacks
- [ ] Physical page allocator
- [ ] Virtual-memory manager
- [ ] GDT/IDT and exception handling
- [ ] APIC/x2APIC timers and interrupts
- [ ] TSC capability/frequency handling with timer fallbacks
- [ ] SMP bring-up through ACPI topology
- [ ] Scheduler
- [ ] User/kernel privilege separation
- [ ] IPC and handle/object model

## Phase 3 - Generic physical PC minimum

- [ ] Full ACPI table enumeration
- [ ] PCI/PCIe enumeration
- [ ] PCI class/vendor/device/subsystem driver matching
- [ ] NVMe read/write
- [ ] AHCI/SATA read/write
- [ ] GPT parser/writer with safety checks
- [ ] xHCI USB host controller
- [ ] USB hub support
- [ ] USB HID keyboard
- [ ] USB HID pointer baseline
- [ ] UEFI GOP framebuffer console/recovery fallback
- [ ] ACPI power button/lid/battery baseline
- [ ] Power off/reboot
- [ ] Boot successfully on at least one physical AMD x64 PC
- [ ] Boot successfully on at least one physical Intel x64 PC

## Phase 4 - Generic installable system

- [ ] Produce generic x86-64 UEFI installation ISO
- [ ] Keep raw GPT/ESP image as deterministic CI/USB artifact
- [ ] Accessible USB/ISO installer
- [ ] Runtime hardware discovery instead of OEM-specific installer images
- [ ] Disk selection with spoken device identity
- [ ] NVMe and AHCI installation paths
- [ ] System partition creation
- [ ] Filesystem implementation/selection
- [ ] Install system image
- [ ] UEFI boot entry creation
- [ ] First boot from physical SSD on AMD
- [ ] First boot from physical SSD on Intel
- [ ] Recovery environment using generic framebuffer/input fallbacks

## Phase 5 - Desktop and accessibility

- [ ] Compositor
- [ ] Accessible native UI toolkit
- [ ] Semantic tree service
- [ ] Keyboard-only shell
- [ ] Speech service
- [ ] Native screen reader
- [ ] Braille transport layer
- [ ] Accessible settings, file manager and terminal

## Phase 6 - Broad hardware drivers, networking, audio and updates

- [ ] PCI High Definition Audio baseline
- [ ] AMD laptop audio extensions where required
- [ ] Intel laptop DSP/audio extensions where required
- [ ] Ethernet baseline
- [ ] Common Intel Ethernet families
- [ ] Common Realtek Ethernet families
- [ ] Network stack
- [ ] Wi-Fi architecture
- [ ] Intel Wi-Fi family
- [ ] Realtek Wi-Fi family
- [ ] MediaTek Wi-Fi family
- [ ] Bluetooth HCI/USB baseline
- [ ] AMD accelerated graphics family
- [ ] Intel accelerated graphics family
- [ ] Preserve GOP safe graphics fallback when native GPU driver fails
- [ ] IOMMU discovery: AMD-Vi and Intel VT-d
- [ ] Package manager
- [ ] Atomic updates
- [ ] Rollback/snapshots

## Phase 7 - Compatibility

- [ ] Win32 ABI/API compatibility program
- [ ] Windows application test corpus
- [ ] Linux subsystem/container runtime
- [ ] Web/PWA runtime
- [ ] Optional Android compatibility research

## Hardware validation rule

Virtual-machine success is necessary but never sufficient for a hardware milestone. Physical AMD and Intel results are tracked separately with firmware version, CPU identity, PCI/USB inventory, storage controller and boot result.

No OEM-specific machine such as ASUS, Dell, HP or Lenovo defines the release image. OEM quirks are optional runtime modules selected after standard hardware discovery.

## Definition of success

The project does not claim to exceed Windows 11 until reproducible benchmarks demonstrate improvements in selected areas such as accessibility coverage, idle resource use, recovery, update reliability, input latency and security isolation while maintaining useful application and hardware compatibility across both AMD and Intel x64 PCs.
