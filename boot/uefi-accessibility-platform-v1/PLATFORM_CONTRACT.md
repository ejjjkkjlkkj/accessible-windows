# UEFI Accessibility Platform v1

This directory defines the integration gate for a self-contained pre-OS accessibility platform.

The deliverable is not considered complete because a speech engine builds or because a virtual machine emits audio. Completion requires one integrated UEFI application/runtime to provide accessible firmware navigation with first-party speech and direct pre-OS audio.

## Required runtime properties

- Live firmware HII strings are the source of spoken UI text.
- Keyboard navigation changes focus and speech in real time.
- The speech engine is first-party and has no operating-system or external-TTS dependency.
- Speech v2 uses 16 kHz signed 16-bit synthesis and is converted deterministically to 48 kHz signed 16-bit stereo for HDA.
- HDA controller, codec, graph route, output pin and DMA progress are discovered/configured at runtime.
- Unknown labels have a deterministic spelling fallback.
- Password/secret controls must never speak secret contents.
- Destructive changes require an explicit confirmation step with spoken state.
- QEMU/OVMF and VMware are validation targets, not substitutes for physical firmware proof.
- Final release closure requires physical pre-OS boot, HII navigation and audible internal-speaker speech on the ASUS M1603QA / Ryzen 7 5800H target.

## Release states

- PLATFORM_BUILD_PASS: integrated EFI builds deterministically and contains live HII navigation plus speech v2.
- VIRTUALIZATION_PASS: QEMU/OVMF and VMware integration proofs are established.
- CLOSURE_BLOCKED_PHYSICAL: physical UEFI/audio proof is not yet established.
- RELEASE_PASS: all virtual and physical requirements are established.

No document, marker or CI result may promote the platform to RELEASE_PASS while any physical requirement remains pending.
