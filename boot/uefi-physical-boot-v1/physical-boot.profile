QEVARYNOX-UEFI-PHYSICAL-BOOT-V1
target = ASUS M1603QA / AMD Ryzen 7 5800H
source = boot/uefi-hii-graph-prompt-speech-v1/hii_graph_prompt_speech_uefi.c
boot-path = EFI/BOOT/BOOTX64.EFI
disk-layout = GPT + EFI System Partition
firmware-nvram-write = forbidden
host-windows-filesystem-write = forbidden
efi-runtime-write-scope = boot media only
expected-controller = PCI_VEN_1022_DEV_15E3
expected-codec = HDAUDIO_VEN_10EC_DEV_0256
expected-controller-selection = PREFERRED_AMD_1022_15E3
expected-proof-file = QEVARYNOX-PHYSICAL-PROOF.TXT
physical-controls = Up previous prompt; Down next prompt; Home first prompt; End last prompt; PageUp minus 5 prompts; PageDown plus 5 prompts; R repeat; Esc exit and persist proof
proof-navigation = HII_GRAPH_NAV_UP=PASS + HII_GRAPH_NAV_DOWN=PASS + HII_GRAPH_NAV_HOME=PASS + HII_GRAPH_NAV_END=PASS + HII_GRAPH_NAV_PAGE_UP=PASS + HII_GRAPH_NAV_PAGE_DOWN=PASS + HII_GRAPH_NAV_REPEAT=PASS + HII_GRAPH_NAV_EXIT=PASS
proof-navigation-speech-events = at least 7
proof-DMA = HII_GRAPH_SPEECH_DMA=PASS + LPIB_PROGRESS=PASS
proof-DMA-reuse = HII_GRAPH_SPEECH_DMA_REUSE=PASS
audible-speaker-proof = human confirmation remains required
claim = a valid proof file after a real ASUS UEFI boot establishes native execution, analog HDA controller selection, live graph routing, selector programming and DMA progress
claim = the proof file alone does not establish that the laptop speaker was audibly heard
