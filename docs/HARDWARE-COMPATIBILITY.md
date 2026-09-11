# Generic x64 hardware compatibility

Accessible Windows targets one generic x86-64 installation image for modern PCs. It is not tied to ASUS or to any single OEM. AMD and Intel systems use the same x86-64 UEFI image; CPU vendor and devices are discovered at runtime.

## Product rule

There is one x64 OS image and one x64 kernel ABI.

The boot path must never require a vendor-specific ASUS, AMD or Intel firmware image. OEM-specific support is layered on top of standard platform discovery.

## Baseline platform contract

The generic PC baseline is:

- x86-64 long mode;
- UEFI firmware;
- ACPI platform description;
- PCI/PCIe configuration and enumeration;
- APIC/x2APIC interrupt architecture where available;
- invariant/constant TSC detection with safe timer fallbacks;
- UEFI GOP framebuffer as the universal early-display fallback;
- GPT storage layout;
- NVMe over PCIe as the primary SSD path;
- AHCI/SATA as the secondary storage path;
- xHCI as the primary USB host-controller path;
- USB HID keyboard and pointer classes;
- PCI High Definition Audio as the first generic audio-controller path.

These interfaces are intentionally CPU-vendor neutral. CPU-specific code is selected only after CPUID/vendor/feature discovery.

## AMD and Intel CPU handling

The kernel must query CPUID rather than assume one vendor.

Common x86-64 code handles:

- paging;
- privilege levels;
- syscall/sysret where available and selected;
- XSAVE/FPU state based on CPUID;
- APIC/x2APIC;
- TSC capability detection;
- SMP discovery through ACPI.

Vendor-specific modules may then enable optional behavior:

- AMD-specific MSRs/features;
- Intel-specific MSRs/features;
- power/performance controls;
- IOMMU support (AMD-Vi / Intel VT-d);
- virtualization extensions (AMD-V / Intel VT-x) when useful later.

Unknown x86-64 CPUs must fall back to the common feature-detected path instead of failing because the vendor is unfamiliar.

## Driver matching model

Drivers are selected by bus/class identifiers, not by laptop model name.

### PCI/PCIe

Match in this order where appropriate:

1. class/subclass/programming-interface for standards-compliant controllers;
2. vendor/device ID for hardware that requires vendor-specific behavior;
3. subsystem vendor/device ID only for machine-specific quirks.

### USB

Prefer standard USB class drivers first:

- HID;
- mass storage;
- hubs;
- audio where practical;
- CDC/network classes where applicable.

Vendor/product-specific USB drivers are a second layer.

### ACPI

ACPI namespace objects describe batteries, buttons, lid, thermal zones, PCI routing, sleep/power state and many platform devices. OEM ACPI methods are handled by isolated quirk modules and must not define the generic boot contract.

## Initial driver coverage

### Required for the first broadly bootable ISO

- UEFI GOP framebuffer;
- ACPI parser/enumerator;
- PCI/PCIe enumerator;
- APIC/x2APIC and timers;
- NVMe;
- AHCI/SATA;
- xHCI;
- USB HID keyboard;
- USB HID mouse/touchpad baseline;
- generic GPT support;
- basic power-off/reboot paths.

### Next coverage tier

- Intel HDA-compatible controller;
- common Ethernet families;
- virtio devices for virtual-machine CI;
- IOMMU discovery;
- laptop battery/lid/power-button ACPI;
- USB audio and storage classes.

### Vendor-specific tier

Graphics acceleration, Wi-Fi, Bluetooth, advanced laptop audio DSPs, fingerprint readers and OEM hotkeys require dedicated driver families. They must not be prerequisites for installation because the OS retains generic fallbacks where technically possible.

## Graphics strategy

The installer and recovery environment must always be able to operate using the UEFI GOP framebuffer when firmware provides it.

Native accelerated graphics is added separately:

- AMD GPU driver family;
- Intel GPU driver family.

A GPU driver failure must fall back to a safe framebuffer/recovery display mode instead of making the system inaccessible.

## Audio strategy

The first native baseline is the PCI High Definition Audio controller model. Modern laptops may additionally require AMD ACP, Intel DSP/SST/SOF-style paths, SoundWire or codec-specific routing. Those are separate modules.

Accessibility requires an audio-independent fallback for diagnostics until native audio is available. Boot and kernel test markers therefore remain observable through debug/serial/test transports in addition to eventual speech output.

## Networking strategy

Ethernet support comes before broad Wi-Fi support because Wi-Fi frequently requires vendor-specific firmware and radio stacks.

Planned families include common Intel, Realtek and other widely deployed PCIe Ethernet adapters. Wi-Fi support is modular by chipset family and firmware redistribution terms must be reviewed before firmware blobs are shipped in an image.

## Installation image

The project will produce both:

- a raw GPT/ESP disk image for deterministic firmware and USB testing;
- a generic x86-64 UEFI bootable installation ISO once the installer payload exists.

Both artifacts boot the same x86-64 kernel and use runtime hardware discovery. There will not be separate AMD and Intel ISOs.

## Hardware validation matrix

CI validates multiple virtual machines and hardware profiles, but virtual machines do not count as physical compatibility proof.

Physical validation should eventually cover at minimum:

- AMD laptop;
- AMD desktop;
- Intel laptop;
- Intel desktop;
- NVMe and AHCI storage variants;
- multiple xHCI implementations;
- machines with Secure Boot off first, then Secure Boot support as a separate milestone.

Every physical result records firmware version, CPU, PCI IDs, USB IDs, ACPI tables relevant to failures, storage controller and boot outcome.

## Reference specifications

Implementation should follow public specifications rather than copying proprietary drivers:

- UEFI Specification 2.11: https://uefi.org/specifications
- ACPI Specification 6.6: https://uefi.org/specifications
- NVMe specifications: https://nvmexpress.org/specifications/
- AHCI: https://www.intel.com/content/www/us/en/io/serial-ata/ahci.html
- USB specifications: https://www.usb.org/documents
- Intel High Definition Audio specification: https://www.intel.com/content/www/us/en/documents/product-specifications/high-definition-audio-specification.pdf

Open-source operating-system drivers may be studied or reused only when their licenses are compatible with the way they are incorporated and distributed.