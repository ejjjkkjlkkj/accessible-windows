# Roadmap

## Phase 0 - Repository bootstrap

- [x] Rust workspace
- [x] Kernel boot-data contract
- [x] Accessibility semantic primitives
- [x] Accessibility invariant tests
- [x] Architecture specification
- [x] Cross-platform CI

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
- [ ] Define and validate the owned kernel handoff structure
- [ ] Split the post-firmware stage into the freestanding kernel crate

## Phase 2 - Kernel foundation

- [ ] Physical page allocator
- [ ] Virtual-memory manager
- [ ] GDT/IDT and exception handling
- [ ] APIC timers and interrupts
- [ ] SMP bring-up
- [ ] Scheduler
- [ ] User/kernel privilege separation
- [ ] IPC and handle/object model

## Phase 3 - Physical PC minimum

- [ ] ACPI enumeration
- [ ] PCI/PCIe enumeration
- [ ] NVMe read/write
- [ ] GPT parser/writer with safety checks
- [ ] USB host controller support
- [ ] USB HID keyboard
- [ ] Basic framebuffer console
- [ ] Power off/reboot

## Phase 4 - Installable system

- [ ] Accessible USB installer
- [ ] Disk selection with spoken device identity
- [ ] System partition creation
- [ ] Filesystem implementation/selection
- [ ] Install system image
- [ ] UEFI boot entry creation
- [ ] First boot from physical SSD
- [ ] Recovery environment

## Phase 5 - Desktop and accessibility

- [ ] Compositor
- [ ] Accessible native UI toolkit
- [ ] Semantic tree service
- [ ] Keyboard-only shell
- [ ] Speech service
- [ ] Native screen reader
- [ ] Braille transport layer
- [ ] Accessible settings, file manager and terminal

## Phase 6 - Networking, audio and updates

- [ ] Network stack
- [ ] Ethernet baseline
- [ ] Wi-Fi architecture
- [ ] Audio stack
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
