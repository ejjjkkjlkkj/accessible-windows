# UEFI Accessibility Platform v2

Platform v2 binds VoiceCore v4 to the live HII screen-reader and native HDA path.

## Runtime path

Firmware HII text -> normalized ASCII fallback -> VoiceCore v4 full French letter-name clips -> contiguous pre-OS PCM -> HDA BDL -> codec route -> speaker or headphone output.

The full-letter strategy deliberately trades EFI image size for intelligibility. Arbitrary HII labels remain speakable without Windows, Linux, NVDA, a cloud service, or an external TTS runtime.

## Gates

- VoiceCore v4 core tests PASS.
- 26 deterministic full letter-name clips generated from the screen voice.
- 32-character worst-case label fits the UEFI speech DMA allocation.
- Integrated EFI compiles with interactive HII navigation.
- PE/COFF output is deterministic.
- Existing HII navigation and HDA runtime markers remain present.
- Virtual execution and physical listening remain separate later gates; build success alone is not treated as audible hardware proof.
