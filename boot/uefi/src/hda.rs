//! Intel High Definition Audio at the firmware stage - real spoken words from the
//! UEFI screen reader, before any operating system.
//!
//! This is the payoff of the whole effort: on a thin laptop with no PC-speaker
//! buzzer, the machine's HDA codec is the only thing that can make a blind user
//! hear anything before the OS. The approach follows Machado & Vieira, "UEFI BIOS
//! Accessibility for the Visually Impaired" (arXiv:1712.03186) - reach the HDA
//! controller from the pre-OS environment and drive the codec - and takes it past
//! where that prototype stopped: where they left DMA as an open question and
//! validated only the codec's beep generator, this streams real PCM speech by DMA.
//!
//! The words are short clips of the boot screen's fixed lines, synthesized ahead
//! of time and embedded as raw 24 kHz mono PCM ([`CLIP_WELCOME`] and friends). At
//! boot the controller is brought up once ([`bring_up`]) and each clip is played
//! through it ([`Speaker::speak`]); the same DMA path will later carry a running
//! speech synthesizer for dynamic text. The firmware identity-maps all of memory
//! during boot services, so a `static`'s address is its physical address and no
//! page mapping is needed.

use uefi::boot;

/// The boot screen's spoken lines, synthesized offline to 24 kHz 16-bit mono PCM.
/// Regenerate with `scripts/gen-speech.ps1` to change wording or voice.
pub static CLIP_WELCOME: &[u8] = include_bytes!("speech/welcome.pcm");
pub static CLIP_ACTIVE: &[u8] = include_bytes!("speech/active.pcm");
pub static CLIP_STARTING: &[u8] = include_bytes!("speech/starting.pcm");
pub static CLIP_LOADING: &[u8] = include_bytes!("speech/loading.pcm");

// ---- PCI mechanism #1 (CF8/CFC) and MMIO -------------------------------

unsafe fn outl(port: u16, value: u32) {
    // SAFETY: caller names a valid dword-wide port.
    unsafe {
        core::arch::asm!("out dx, eax", in("dx") port, in("eax") value,
            options(nomem, nostack, preserves_flags));
    }
}

unsafe fn inl(port: u16) -> u32 {
    let value: u32;
    // SAFETY: caller names a valid dword-wide port.
    unsafe {
        core::arch::asm!("in eax, dx", out("eax") value, in("dx") port,
            options(nomem, nostack, preserves_flags));
    }
    value
}

const PCI_CONFIG_ADDRESS: u16 = 0x0cf8;
const PCI_CONFIG_DATA: u16 = 0x0cfc;

fn pci_address(bus: u8, device: u8, function: u8, offset: u8) -> u32 {
    0x8000_0000
        | (u32::from(bus) << 16)
        | (u32::from(device) << 11)
        | (u32::from(function) << 8)
        | u32::from(offset & 0xfc)
}

unsafe fn pci_read32(bus: u8, device: u8, function: u8, offset: u8) -> u32 {
    // SAFETY: CF8/CFC are the architected PCI configuration ports.
    unsafe {
        outl(PCI_CONFIG_ADDRESS, pci_address(bus, device, function, offset));
        inl(PCI_CONFIG_DATA)
    }
}

unsafe fn pci_write32(bus: u8, device: u8, function: u8, offset: u8, value: u32) {
    // SAFETY: CF8/CFC are the architected PCI configuration ports.
    unsafe {
        outl(PCI_CONFIG_ADDRESS, pci_address(bus, device, function, offset));
        outl(PCI_CONFIG_DATA, value);
    }
}

unsafe fn read8(base: u64, offset: u64) -> u8 {
    // SAFETY: base+offset is inside the firmware-identity-mapped BAR0 window.
    unsafe { ((base + offset) as *const u8).read_volatile() }
}
unsafe fn write8(base: u64, offset: u64, value: u8) {
    // SAFETY: as read8.
    unsafe { ((base + offset) as *mut u8).write_volatile(value) };
}
unsafe fn read16(base: u64, offset: u64) -> u16 {
    // SAFETY: as read8.
    unsafe { ((base + offset) as *const u16).read_volatile() }
}
unsafe fn write16(base: u64, offset: u64, value: u16) {
    // SAFETY: as read8.
    unsafe { ((base + offset) as *mut u16).write_volatile(value) };
}
unsafe fn read32(base: u64, offset: u64) -> u32 {
    // SAFETY: as read8.
    unsafe { ((base + offset) as *const u32).read_volatile() }
}
unsafe fn write32(base: u64, offset: u64, value: u32) {
    // SAFETY: as read8.
    unsafe { ((base + offset) as *mut u32).write_volatile(value) };
}

// ---- HDA registers (see the kernel driver for the annotated set) ---------

const REG_GCAP: u64 = 0x00;
const REG_GCTL: u64 = 0x08;
const REG_STATESTS: u64 = 0x0e;
const REG_CORBLBASE: u64 = 0x40;
const REG_CORBUBASE: u64 = 0x44;
const REG_CORBWP: u64 = 0x48;
const REG_CORBRP: u64 = 0x4a;
const REG_CORBCTL: u64 = 0x4c;
const REG_CORBSIZE: u64 = 0x4e;
const REG_RIRBLBASE: u64 = 0x50;
const REG_RIRBUBASE: u64 = 0x54;
const REG_RIRBWP: u64 = 0x58;
const REG_RINTCNT: u64 = 0x5a;
const REG_RIRBCTL: u64 = 0x5c;
const REG_RIRBSTS: u64 = 0x5d;
const REG_RIRBSIZE: u64 = 0x5e;

const GCTL_CRST: u32 = 1 << 0;
const CORBRP_RST: u16 = 1 << 15;
const CORBCTL_RUN: u8 = 1 << 1;
const RIRBWP_RST: u16 = 1 << 15;
const RIRBCTL_DMAEN: u8 = 1 << 1;
const RIRBSTS_INTFL: u8 = 1 << 0;

const RING_ENTRIES: u16 = 256;
const RING_SIZE_256: u8 = 0x02;

const VERB_GET_PARAMETER: u32 = 0xf00;
const PARAM_VENDOR_ID: u32 = 0x00;
const PARAM_SUBNODE_COUNT: u32 = 0x04;
const PARAM_FUNCTION_GROUP_TYPE: u32 = 0x05;
const PARAM_WIDGET_CAP: u32 = 0x09;
const PARAM_PIN_CAP: u32 = 0x0c;

const WIDGET_AUDIO_OUTPUT: u32 = 0x0;
const WIDGET_PIN_COMPLEX: u32 = 0x4;

const VERB4_SET_FORMAT: u32 = 0x2;
const VERB4_SET_AMP: u32 = 0x3;
const VERB_SET_POWER_STATE: u32 = 0x705;
const VERB_SET_STREAM_CHANNEL: u32 = 0x706;
const VERB_SET_PIN_CONTROL: u32 = 0x707;
const VERB_SET_EAPD: u32 = 0x70c;

const PIN_CONTROL_OUT_ENABLE: u32 = 1 << 6;
const EAPD_ENABLE: u32 = 1 << 1;
const AMP_OUT_UNMUTE: u16 = (1 << 15) | (1 << 13) | (1 << 12) | 0x2a;

/// Clips are 24 kHz mono; played as 24 kHz 16-bit stereo (each sample duplicated
/// to both channels), the format value for base 48 kHz / 2, 16-bit, 2 channels.
const STREAM_FORMAT: u16 = 0x0111;
/// Bytes per second of the played stream (24000 frames * 2 channels * 2 bytes).
const BYTES_PER_SEC: u32 = 24000 * 2 * 2;
const STREAM_TAG: u8 = 1;

const SD_CTL: u64 = 0x00;
const SD_LPIB: u64 = 0x04;
const SD_CBL: u64 = 0x08;
const SD_LVI: u64 = 0x0c;
const SD_FMT: u64 = 0x12;
const SD_BDPL: u64 = 0x18;
const SD_BDPU: u64 = 0x1c;
const SDCTL_SRST: u8 = 1 << 0;
const SDCTL_RUN: u8 = 1 << 1;
const STREAM_BASE: u64 = 0x80;
const STREAM_STRIDE: u64 = 0x20;

#[repr(C, align(4096))]
struct Page([u8; 4096]);

static mut CORB: Page = Page([0; 4096]);
static mut RIRB: Page = Page([0; 4096]);
static mut BDL: Page = Page([0; 4096]);

/// Playback buffer, page-aligned and identity-mapped. Sized for the longest clip
/// as stereo (mono clip bytes * 2): 384 KiB holds ~4 s of 24 kHz stereo.
const AUDIO_BYTES: usize = 393_216;
#[repr(C, align(4096))]
struct AudioBuffer([u8; AUDIO_BYTES]);
static mut AUDIO: AudioBuffer = AudioBuffer([0; AUDIO_BYTES]);

#[derive(Clone, Copy)]
struct PciLocation {
    bus: u8,
    device: u8,
    function: u8,
}

fn is_hda(location: PciLocation) -> bool {
    // SAFETY: configuration reads have no side effects.
    let id = unsafe { pci_read32(location.bus, location.device, location.function, 0x00) };
    if id & 0xffff == 0xffff {
        return false;
    }
    let class = unsafe { pci_read32(location.bus, location.device, location.function, 0x08) };
    (class >> 24) & 0xff == 0x04 && (class >> 16) & 0xff == 0x03
}

/// Enable memory space + bus mastering and return BAR0. No page mapping: the
/// firmware identity-maps the BAR, so its physical address is directly usable.
fn enable_bar0(location: PciLocation) -> Option<u64> {
    let PciLocation {
        bus,
        device,
        function,
    } = location;
    // SAFETY: enable MMIO + bus mastering, then read the 64-bit BAR0.
    let base = unsafe {
        let command = pci_read32(bus, device, function, 0x04);
        pci_write32(bus, device, function, 0x04, command | 0b110);
        let low = pci_read32(bus, device, function, 0x10);
        let high = pci_read32(bus, device, function, 0x14);
        (u64::from(low & 0xffff_fff0)) | (u64::from(high) << 32)
    };
    (base != 0).then_some(base)
}

/// A brought-up HDA controller with a configured output path: ready to speak.
pub struct Speaker {
    base: u64,
    codec: u8,
    input_streams: u8,
    dac: u8,
    pin: u8,
    rirb_read: u16,
}

fn build_verb(codec: u8, nid: u8, verb: u32, payload: u32) -> u32 {
    (u32::from(codec) << 28) | (u32::from(nid) << 20) | ((verb & 0xfff) << 8) | (payload & 0xff)
}

impl Speaker {
    fn send_raw(&mut self, command: u32) -> Result<u32, &'static str> {
        let corb = core::ptr::addr_of_mut!(CORB) as *mut u32;
        // SAFETY: CORB is an identity-mapped ring; advance the write pointer and
        // place the verb at the new slot.
        unsafe {
            let next = (read16(self.base, REG_CORBWP) + 1) % RING_ENTRIES;
            corb.add(next as usize).write_volatile(command);
            write16(self.base, REG_CORBWP, next);
            let mut budget = 10_000_000u32;
            loop {
                let write = read16(self.base, REG_RIRBWP) & (RING_ENTRIES - 1);
                if write != self.rirb_read {
                    break;
                }
                budget -= 1;
                if budget == 0 {
                    return Err("no_response");
                }
                core::hint::spin_loop();
            }
            self.rirb_read = (self.rirb_read + 1) % RING_ENTRIES;
            let rirb = core::ptr::addr_of!(RIRB) as *const u32;
            let response = rirb.add(self.rirb_read as usize * 2).read_volatile();
            write8(self.base, REG_RIRBSTS, RIRBSTS_INTFL);
            Ok(response)
        }
    }

    fn command(&mut self, nid: u8, verb: u32, payload: u32) -> Result<u32, &'static str> {
        self.send_raw(build_verb(self.codec, nid, verb, payload))
    }

    fn command16(&mut self, nid: u8, verb4: u32, payload: u16) -> Result<(), &'static str> {
        let value = (u32::from(self.codec) << 28)
            | (u32::from(nid) << 20)
            | ((verb4 & 0xf) << 16)
            | u32::from(payload);
        self.send_raw(value).map(|_| ())
    }

    fn set(&mut self, nid: u8, verb: u32, payload: u32) -> Result<(), &'static str> {
        self.command(nid, verb, payload).map(|_| ())
    }

    fn get_parameter(&mut self, nid: u8, parameter: u32) -> Result<u32, &'static str> {
        self.command(nid, VERB_GET_PARAMETER, parameter)
    }

    fn widget_type(&mut self, nid: u8) -> Result<u32, &'static str> {
        Ok((self.get_parameter(nid, PARAM_WIDGET_CAP)? >> 20) & 0xf)
    }

    fn output_stream_base(&self) -> u64 {
        self.base + STREAM_BASE + u64::from(self.input_streams) * STREAM_STRIDE
    }

    /// Play one 24 kHz mono PCM clip through the codec, blocking until it has
    /// finished. Returns true when the link position advanced (the controller
    /// streamed the samples), which on real hardware is audible speech.
    pub fn speak(&mut self, clip: &[u8]) -> bool {
        // Duplicate each mono 16-bit sample to both channels into the aligned DMA
        // buffer, clamped to its capacity.
        let mono_samples = (clip.len() / 2).min(AUDIO_BYTES / 4);
        let audio = core::ptr::addr_of_mut!(AUDIO) as *mut i16;
        for index in 0..mono_samples {
            let sample = i16::from_le_bytes([clip[index * 2], clip[index * 2 + 1]]);
            // SAFETY: index*2+1 < AUDIO_BYTES/2, inside the buffer.
            unsafe {
                audio.add(index * 2).write_volatile(sample);
                audio.add(index * 2 + 1).write_volatile(sample);
            }
        }
        let stereo_bytes = (mono_samples * 4) as u32;
        if stereo_bytes == 0 {
            return false;
        }

        // Per-clip: set the converter format (all clips share it here).
        if self.command16(self.dac, VERB4_SET_FORMAT, STREAM_FORMAT).is_err() {
            return false;
        }

        let stream = self.output_stream_base();
        let bdl_phys = core::ptr::addr_of!(BDL) as u64;
        let audio_phys = core::ptr::addr_of!(AUDIO) as u64;
        let bdl = core::ptr::addr_of_mut!(BDL) as *mut u32;
        // SAFETY: BDL is an identity-mapped descriptor static; four dwords fit.
        unsafe {
            bdl.add(0).write_volatile(audio_phys as u32);
            bdl.add(1).write_volatile((audio_phys >> 32) as u32);
            bdl.add(2).write_volatile(stereo_bytes);
            bdl.add(3).write_volatile(1);
        }
        // SAFETY: `stream` is inside the identity-mapped BAR0 register file.
        unsafe {
            write8(stream, SD_CTL, SDCTL_SRST);
            let mut budget = 1_000_000u32;
            while read8(stream, SD_CTL) & SDCTL_SRST == 0 && budget > 0 {
                budget -= 1;
                core::hint::spin_loop();
            }
            write8(stream, SD_CTL, 0);
            let mut budget = 1_000_000u32;
            while read8(stream, SD_CTL) & SDCTL_SRST != 0 && budget > 0 {
                budget -= 1;
                core::hint::spin_loop();
            }
            write32(stream, SD_CBL, stereo_bytes);
            write16(stream, SD_LVI, 0);
            write16(stream, SD_FMT, STREAM_FORMAT);
            write32(stream, SD_BDPL, bdl_phys as u32);
            write32(stream, SD_BDPU, (bdl_phys >> 32) as u32);
            write8(stream, SD_CTL + 2, STREAM_TAG << 4);
            write8(stream, SD_CTL, read8(stream, SD_CTL) | SDCTL_RUN);
        }

        // Wait out the clip: its length plus a small margin, checking that the
        // position moved so a stuck stream is not reported as spoken.
        let duration_ms = stereo_bytes / (BYTES_PER_SEC / 1000);
        let mut moved = 0u32;
        let mut waited = 0u32;
        while waited < duration_ms + 150 {
            boot::stall(core::time::Duration::from_millis(20));
            waited += 20;
            // SAFETY: reading LPIB is side-effect-free.
            let position = unsafe { read32(stream, SD_LPIB) };
            moved = moved.max(position);
            if position >= stereo_bytes {
                break;
            }
        }
        // SAFETY: clearing RUN on our own stream descriptor.
        unsafe {
            write8(stream, SD_CTL, read8(stream, SD_CTL) & !SDCTL_RUN);
        }
        moved > 0
    }
}

fn reset(base: u64) -> bool {
    // SAFETY: BAR0 is the identity-mapped MMIO window.
    unsafe {
        write32(base, REG_GCTL, read32(base, REG_GCTL) & !GCTL_CRST);
        let mut budget = 10_000_000u32;
        while read32(base, REG_GCTL) & GCTL_CRST != 0 {
            budget -= 1;
            if budget == 0 {
                return false;
            }
            core::hint::spin_loop();
        }
        write32(base, REG_GCTL, read32(base, REG_GCTL) | GCTL_CRST);
        let mut budget = 10_000_000u32;
        while read32(base, REG_GCTL) & GCTL_CRST == 0 {
            budget -= 1;
            if budget == 0 {
                return false;
            }
            core::hint::spin_loop();
        }
    }
    true
}

fn setup_rings(base: u64) {
    let corb_phys = core::ptr::addr_of!(CORB) as u64;
    let rirb_phys = core::ptr::addr_of!(RIRB) as u64;
    // SAFETY: MMIO on a controller that exists; ring bases are identity-mapped.
    unsafe {
        write8(base, REG_CORBCTL, 0);
        write8(base, REG_RIRBCTL, 0);
        write8(base, REG_CORBSIZE, RING_SIZE_256);
        write32(base, REG_CORBLBASE, corb_phys as u32);
        write32(base, REG_CORBUBASE, (corb_phys >> 32) as u32);
        write16(base, REG_CORBRP, CORBRP_RST);
        let mut budget = 1_000_000u32;
        while read16(base, REG_CORBRP) & CORBRP_RST == 0 && budget > 0 {
            budget -= 1;
            core::hint::spin_loop();
        }
        write16(base, REG_CORBRP, 0);
        write16(base, REG_CORBWP, 0);
        write8(base, REG_RIRBSIZE, RING_SIZE_256);
        write32(base, REG_RIRBLBASE, rirb_phys as u32);
        write32(base, REG_RIRBUBASE, (rirb_phys >> 32) as u32);
        write16(base, REG_RIRBWP, RIRBWP_RST);
        // A high response-interrupt count: a threshold of 1 stalls the ring after
        // the first response (the bug the kernel driver hit and this avoids).
        write16(base, REG_RINTCNT, 0xff);
        write8(base, REG_RIRBCTL, RIRBCTL_DMAEN);
        write8(base, REG_CORBCTL, CORBCTL_RUN);
    }
}

fn find_output(speaker: &mut Speaker) -> Result<(u8, u8), &'static str> {
    let root = speaker.get_parameter(0, PARAM_SUBNODE_COUNT)?;
    let first_group = ((root >> 16) & 0xff) as u8;
    let group_count = (root & 0xff) as u8;
    for group in 0..group_count {
        let nid = first_group + group;
        if speaker.get_parameter(nid, PARAM_FUNCTION_GROUP_TYPE)? & 0xff != 0x01 {
            continue;
        }
        speaker.set(nid, VERB_SET_POWER_STATE, 0)?;
        let widgets = speaker.get_parameter(nid, PARAM_SUBNODE_COUNT)?;
        let first_widget = ((widgets >> 16) & 0xff) as u8;
        let widget_count = (widgets & 0xff) as u8;
        let mut dac: Option<u8> = None;
        let mut pin: Option<u8> = None;
        for index in 0..widget_count {
            let widget = first_widget + index;
            let widget_type = speaker.widget_type(widget)?;
            if widget_type == WIDGET_AUDIO_OUTPUT && dac.is_none() {
                dac = Some(widget);
            } else if widget_type == WIDGET_PIN_COMPLEX
                && pin.is_none()
                && speaker.get_parameter(widget, PARAM_PIN_CAP)? & (1 << 4) != 0
            {
                pin = Some(widget);
            }
        }
        if let (Some(dac), Some(pin)) = (dac, pin) {
            return Ok((dac, pin));
        }
    }
    Err("no_output_path")
}

/// Find and bring up an HDA controller with a codec and an output path, ready to
/// speak. Returns `None` (and logs why) when there is no usable audio, so the
/// caller can fall back to the PC speaker.
pub fn bring_up() -> Option<Speaker> {
    let mut found: Option<PciLocation> = None;
    'scan: for bus in 0..=255u16 {
        for device in 0..32u8 {
            for function in 0..8u8 {
                let location = PciLocation {
                    bus: bus as u8,
                    device,
                    function,
                };
                if is_hda(location) {
                    found = Some(location);
                    break 'scan;
                }
            }
        }
    }
    let location = found?;
    let base = enable_bar0(location)?;
    if !reset(base) {
        log::error!("AW_UEFI_HDA_FAIL reason=reset");
        return None;
    }
    // SAFETY: BAR0 is the identity-mapped MMIO window.
    let gcap = unsafe { read16(base, REG_GCAP) };
    setup_rings(base);

    // SAFETY: reading STATESTS has no side effects.
    let statests = unsafe {
        let mut budget = 1_000_000u32;
        let mut bits = read16(base, REG_STATESTS);
        while bits == 0 && budget > 0 {
            budget -= 1;
            core::hint::spin_loop();
            bits = read16(base, REG_STATESTS);
        }
        bits
    };
    if statests == 0 {
        log::info!("AW_UEFI_HDA_UNAVAILABLE");
        return None;
    }

    let mut speaker = Speaker {
        base,
        codec: statests.trailing_zeros() as u8,
        input_streams: ((gcap >> 8) & 0xf) as u8,
        dac: 0,
        pin: 0,
        rirb_read: 0,
    };

    let vendor = match speaker.get_parameter(0, PARAM_VENDOR_ID) {
        Ok(vendor) if vendor != 0 && vendor != 0xffff_ffff => vendor,
        _ => {
            log::error!("AW_UEFI_HDA_FAIL reason=codec");
            return None;
        }
    };
    log::info!("AW_UEFI_HDA_CODEC_ID vendor_device=0x{vendor:08x}");

    let (dac, pin) = match find_output(&mut speaker) {
        Ok(path) => path,
        Err(reason) => {
            log::error!("AW_UEFI_HDA_FAIL reason={reason}");
            return None;
        }
    };
    speaker.dac = dac;
    speaker.pin = pin;

    // Configure the output path once (format is set per clip in `speak`).
    let configured = speaker.set(dac, VERB_SET_POWER_STATE, 0).is_ok()
        && speaker
            .set(dac, VERB_SET_STREAM_CHANNEL, u32::from(STREAM_TAG) << 4)
            .is_ok()
        && speaker.command16(dac, VERB4_SET_AMP, AMP_OUT_UNMUTE).is_ok()
        && speaker.set(pin, VERB_SET_POWER_STATE, 0).is_ok()
        && speaker.set(pin, VERB_SET_PIN_CONTROL, PIN_CONTROL_OUT_ENABLE).is_ok()
        && speaker.set(pin, VERB_SET_EAPD, EAPD_ENABLE).is_ok()
        && speaker.command16(pin, VERB4_SET_AMP, AMP_OUT_UNMUTE).is_ok();
    if !configured {
        log::error!("AW_UEFI_HDA_FAIL reason=configure");
        return None;
    }
    log::info!("AW_UEFI_HDA_READY dac={dac} pin={pin}");
    Some(speaker)
}
