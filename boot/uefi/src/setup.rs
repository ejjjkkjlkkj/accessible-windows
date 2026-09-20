//! Accessible UEFI Setup Utility - a complete, spoken, keyboard-operable firmware
//! setup with real menus *and submenus*, modeled on the ones a sighted user sees
//! (AMI Aptio, the ASUS UEFI BIOS Utility) but usable entirely without sight, before
//! the operating system exists.
//!
//! A real firmware setup organizes everything into a tree: top tabs - Main, Advanced,
//! Boot, Security, Save and Exit - and, under them, submenus (CPU Configuration, Boot
//! Option Priorities, Secure Boot, ...) you descend into and back out of. None of that
//! is reachable by a blind user, because the firmware's own setup is silent. This
//! rebuilds the whole tree at the pre-OS stage the project controls and makes every
//! screen, every item and every help line *spoken* - through the one `aw-screen-reader`
//! engine for the console/marker wording and pre-recorded clips for the fixed
//! scaffolding through the real HDA codec - and operable on the firmware's own keyboard
//! (so a USB keyboard works before any kernel USB stack exists). Left/Right move across
//! the top tabs, Up/Down move within a screen, Enter opens a submenu or activates an
//! item, Escape steps back out (and, at the top level, boots normally).
//!
//! Honest scope: a loaded UEFI application cannot rewrite chipset or CPU straps the way
//! the firmware's own setup can, so Main, Advanced and Security here *read and speak*
//! real machine state (firmware identity, RTC time, memory, display, CPU, virtualization
//! support, Secure Boot) rather than pretending to change it, and offer "Enter firmware
//! setup" for the settings only the firmware itself owns. The Boot tab is fully
//! actionable through architected UEFI services: the real boot entries are enumerated
//! from `BootOrder`/`Boot####`, and for any of them a blind user can **boot it now**
//! (via `BootNext` + restart) or **make it the persistent default** (by rewriting
//! `BootOrder`) - things a silent firmware never lets them do. Save and Exit requests
//! the firmware UI through `OsIndications`, or resets / powers off through runtime
//! `ResetSystem`.
//!
//! Determinism and the unattended contract: with nobody at the keyboard a countdown
//! takes the safe default ("Boot normally") and continues, so a headless machine - and
//! the timed proof harness, which presses no key - never hangs. Every marker is emitted
//! through [`crate::aw_mark`], so it lands on the 0xE9 debug console (QEMU) and the COM1
//! mirror (VMware, hardware) alike.

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use core::time::Duration;

use aw_accessibility::{NodeId, Rect, Role, SemanticNode, State, validate_node};
use aw_screen_reader::{FocusContext, announce_focus};
use uefi::mem::memory_map::MemoryMap;
use uefi::proto::console::text::{Key, ScanCode};
use uefi::runtime::{self, VariableAttributes, VariableVendor};
use uefi::table::cfg::ConfigTableEntry;
use uefi::{CStr16, Status, boot, cstr16, system};

use crate::audio;
use crate::aw_mark;
use crate::hda;
use crate::sound;

/// Pitch of the cue when the highlight moves within a screen.
const CUE_MOVE_HZ: u32 = 740;
/// Pitch of the cue when a top tab changes or a submenu is entered.
const CUE_TAB_HZ: u32 = 622;
/// Pitch of the cue when stepping back out of a submenu.
const CUE_BACK_HZ: u32 = 466;
/// Pitch of the cue confirming a persistent change was written.
const CUE_APPLIED_HZ: u32 = 988;
/// Pitch of the cue confirming the boot is continuing.
const CUE_CONTINUE_HZ: u32 = 523;
/// Pitch of the cue when the setup is ready and waiting for the user.
const CUE_READY_HZ: u32 = 880;

/// How long an unattended boot waits for a key before it continues on its own.
/// Kept short: it is pure latency on every boot where nobody reviews, and it is in the
/// critical path of the timed boot proofs, which press no key and so always wait this
/// out before booting normally.
const REVIEW_WINDOW: Duration = Duration::from_secs(2);
/// How often the review window polls for a keystroke.
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// `EFI_OS_INDICATIONS_BOOT_TO_FW_UI`: asks the firmware to enter its own setup UI on
/// the next boot. Advertised in `OsIndicationsSupported` when honored.
const OS_INDICATIONS_BOOT_TO_FW_UI: u64 = 0x0000_0000_0000_0001;
/// `LOAD_OPTION_ACTIVE`: a `Boot####` option the firmware would actually try.
const LOAD_OPTION_ACTIVE: u32 = 0x0000_0001;

/// The language the setup speaks and shows. French is the default; a Language item on the
/// Main tab switches to English, the way a real ASUS/AMI BIOS offers a "System Language"
/// option. Only the fixed scaffolding is translated (labels, help, clips); dynamic values
/// (device names, SMBIOS strings) are the machine's own text in either language.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Lang {
    Fr,
    En,
}

impl Lang {
    /// The other language, for the toggle.
    fn toggled(self) -> Self {
        match self {
            Lang::Fr => Lang::En,
            Lang::En => Lang::Fr,
        }
    }
}

/// Pick the French or English form of a fixed string for the current language.
fn tx(lang: Lang, fr: &'static str, en: &'static str) -> &'static str {
    match lang {
        Lang::Fr => fr,
        Lang::En => en,
    }
}

/// Pick the French or English clip for the current language.
fn clip(lang: Lang, fr: &'static [u8], en: &'static [u8]) -> &'static [u8] {
    match lang {
        Lang::Fr => fr,
        Lang::En => en,
    }
}

/// The variable attributes a boot-control global carries (non-volatile, visible to boot
/// services and to the runtime), matching how firmware stores `BootOrder`/`BootNext`.
fn boot_var_attributes() -> VariableAttributes {
    VariableAttributes::NON_VOLATILE
        | VariableAttributes::BOOTSERVICE_ACCESS
        | VariableAttributes::RUNTIME_ACCESS
}

/// What activating an item does.
#[derive(Clone, Copy)]
enum Action {
    /// No action: a read-only line of machine state.
    Info,
    /// Open the submenu at this screen index.
    SubMenu(usize),
    /// Step back out of the current submenu (Escape does the same).
    Back,
    /// Continue booting Accessible Windows - the safe default.
    BootNormally,
    /// Set this `Boot####` id as `BootNext` and restart so the firmware boots it now.
    BootNow(u16),
    /// Rewrite `BootOrder` so this `Boot####` id is first - the persistent default.
    MakeDefault(u16),
    /// Move this `Boot####` id one place earlier in `BootOrder`.
    MoveUp(u16),
    /// Move this `Boot####` id one place later in `BootOrder`.
    MoveDown(u16),
    /// Request the firmware's own setup UI through `OsIndications`, then restart.
    EnterSetup,
    /// Cold-reset the machine now.
    Reset,
    /// Power the machine off now.
    Shutdown,
    /// Switch the setup's language (French <-> English) and rebuild.
    ToggleLang,
}

impl Action {
    /// The accessibility role used to announce an item: read-only lines are static text
    /// (no role word); everything selectable is a menu item.
    fn role(self) -> Role {
        match self {
            Action::Info => Role::StaticText,
            _ => Role::MenuItem,
        }
    }
}

/// One row in a screen: what it says, its help line, its spoken clip (when the label is
/// fixed and pre-recorded), and what selecting it does.
struct Item {
    text: String,
    help: String,
    clip: Option<&'static [u8]>,
    action: Action,
}

impl Item {
    fn info(text: String, help: &str) -> Self {
        Self {
            text,
            help: String::from(help),
            clip: None,
            action: Action::Info,
        }
    }

    fn action(text: &str, help: &str, clip: Option<&'static [u8]>, action: Action) -> Self {
        Self {
            text: String::from(text),
            help: String::from(help),
            clip,
            action,
        }
    }

    fn dynamic(text: String, help: &str, clip: Option<&'static [u8]>, action: Action) -> Self {
        Self {
            text,
            help: String::from(help),
            clip,
            action,
        }
    }
}

/// One screen in the tree: a top tab or a submenu. The five top tabs are the entries of
/// [`Tree::tabs`] and are announced as tabs with a 1..5 position; every other screen is a
/// submenu, reached from a parent and announced by title. Depth in the nav stack, not a
/// field here, is what tells the two apart at render/announce time.
struct Screen {
    title: String,
    title_clip: Option<&'static [u8]>,
    items: Vec<Item>,
}

/// The whole setup tree plus the indices of the five top tabs and which tab is Boot (the
/// safe-default landing tab, located by index so it is language-independent).
struct Tree {
    screens: Vec<Screen>,
    tabs: Vec<usize>,
    boot_tab: usize,
}

// ---- Machine-state gathering (read, never change) ------------------------------

/// The CPUID vendor string, e.g. "GenuineIntel" or "AuthenticAMD".
fn cpu_vendor() -> String {
    // CPUID leaf 0 is always available and side-effect free.
    let leaf = core::arch::x86_64::__cpuid(0);
    let mut bytes = [0u8; 12];
    bytes[0..4].copy_from_slice(&leaf.ebx.to_le_bytes());
    bytes[4..8].copy_from_slice(&leaf.edx.to_le_bytes());
    bytes[8..12].copy_from_slice(&leaf.ecx.to_le_bytes());
    String::from_utf8_lossy(&bytes).trim().into()
}

/// The processor brand string from extended CPUID leaves, when the CPU provides it.
fn cpu_brand() -> String {
    // Leaf 0x80000000 reports the highest extended leaf; reads are pure.
    let max = core::arch::x86_64::__cpuid(0x8000_0000).eax;
    if max < 0x8000_0004 {
        return cpu_vendor();
    }
    let mut bytes = [0u8; 48];
    for (block, leaf) in (0x8000_0002u32..=0x8000_0004).enumerate() {
        // Guarded by the max-leaf check above; reads are pure.
        let r = core::arch::x86_64::__cpuid(leaf);
        let base = block * 16;
        bytes[base..base + 4].copy_from_slice(&r.eax.to_le_bytes());
        bytes[base + 4..base + 8].copy_from_slice(&r.ebx.to_le_bytes());
        bytes[base + 8..base + 12].copy_from_slice(&r.ecx.to_le_bytes());
        bytes[base + 12..base + 16].copy_from_slice(&r.edx.to_le_bytes());
    }
    let brand: String = String::from_utf8_lossy(&bytes).into_owned();
    let trimmed = brand.trim();
    if trimmed.is_empty() {
        cpu_vendor()
    } else {
        String::from(trimmed)
    }
}

/// Read an MSR. Only ever called after a CPUID feature check proves the MSR exists on
/// this CPU, so it cannot fault the firmware.
///
/// # Safety
/// CPL0, and `msr` must be a register this CPU implements.
unsafe fn rdmsr(msr: u32) -> u64 {
    let lo: u32;
    let hi: u32;
    // SAFETY: `rdmsr` reads model-specific register `ecx`; the caller guarantees it
    // exists. No memory is touched.
    unsafe {
        core::arch::asm!("rdmsr", in("ecx") msr, out("eax") lo, out("edx") hi,
            options(nomem, nostack, preserves_flags));
    }
    (u64::from(hi) << 32) | u64::from(lo)
}

/// Describe hardware virtualization support - the setting a blind user could never reach
/// in a silent firmware. Intel VT-x (VMX) exposes a lock/enable state in
/// `IA32_FEATURE_CONTROL`; AMD-V (SVM) is reported from CPUID.
fn virtualization_status(lang: Lang) -> String {
    let vendor = cpu_vendor();
    // CPUID leaf 1 is always present; reads are pure.
    let vmx = core::arch::x86_64::__cpuid(1).ecx & (1 << 5) != 0;
    if vendor == "GenuineIntel" && vmx {
        // IA32_FEATURE_CONTROL (0x3A) exists on every VMX-capable Intel part, so the
        // CPUID check above makes this rdmsr safe. Bit 0 locks the register; bit 2
        // enables VMX outside SMX. Locked-but-disabled is the "off in firmware" case.
        // SAFETY: guarded by the Intel + VMX check.
        let feature_control = unsafe { rdmsr(0x3A) };
        let locked = feature_control & 0b001 != 0;
        let enabled = feature_control & 0b100 != 0;
        return String::from(match (locked, enabled) {
            (_, true) => tx(lang, "Intel VT-x, activé", "Intel VT-x, enabled"),
            (true, false) => tx(
                lang,
                "Intel VT-x, pris en charge mais désactivé dans le firmware",
                "Intel VT-x, supported but disabled in firmware",
            ),
            (false, false) => tx(lang, "Intel VT-x, pris en charge", "Intel VT-x, supported"),
        });
    }
    // Leaf 0x80000001 is present on all long-mode CPUs; reads are pure.
    let svm = core::arch::x86_64::__cpuid(0x8000_0001).ecx & (1 << 2) != 0;
    if vendor == "AuthenticAMD" && svm {
        return String::from(tx(lang, "AMD-V, pris en charge", "AMD-V, supported"));
    }
    String::from(tx(lang, "non pris en charge", "not supported"))
}

/// Total usable RAM in mebibytes, summed from the UEFI memory map. Best effort: a map
/// failure reports 0 rather than aborting the setup.
fn installed_memory_mib() -> u64 {
    match boot::memory_map(uefi::mem::memory_map::MemoryType::LOADER_DATA) {
        Ok(map) => {
            let pages: u64 = map.entries().map(|entry| entry.page_count).sum();
            pages * 4096 / (1024 * 1024)
        }
        Err(_) => 0,
    }
}

/// Read a small global UEFI variable by name into an owned buffer, or `None` if it is
/// absent or unreadable. Uses the boxed reader so a large `Boot####` option (long device
/// paths) is never truncated.
fn read_global(name: &CStr16) -> Option<Vec<u8>> {
    match runtime::get_variable_boxed(name, &VariableVendor::GLOBAL_VARIABLE) {
        Ok((data, _)) => Some(data.into_vec()),
        Err(_) => None,
    }
}

/// Decode the human description of a `Boot####` load option, or `None` if the variable
/// is malformed or the option is inactive. The layout is a `u32` attributes, a `u16`
/// device-path length, then a NUL-terminated UCS-2 description.
fn decode_boot_option(raw: &[u8]) -> Option<String> {
    if raw.len() < 6 {
        return None;
    }
    let attributes = u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]);
    if attributes & LOAD_OPTION_ACTIVE == 0 {
        return None;
    }
    // The description is UCS-2 starting at offset 6, terminated by a 0x0000 unit.
    let mut units = Vec::new();
    let mut offset = 6;
    while offset + 1 < raw.len() {
        let unit = u16::from_le_bytes([raw[offset], raw[offset + 1]]);
        if unit == 0 {
            break;
        }
        units.push(unit);
        offset += 2;
    }
    if units.is_empty() {
        return None;
    }
    Some(String::from_utf16_lossy(&units))
}

/// The `Boot####` variable name for `id`, built into `buffer` (nine UCS-2 units: `B o o
/// t` then four upper-hex digits then a NUL). Returns it as a `CStr16`.
fn boot_var_name(id: u16, buffer: &mut [u16; 9]) -> &CStr16 {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    *buffer = [
        b'B' as u16,
        b'o' as u16,
        b'o' as u16,
        b't' as u16,
        HEX[((id >> 12) & 0xf) as usize] as u16,
        HEX[((id >> 8) & 0xf) as usize] as u16,
        HEX[((id >> 4) & 0xf) as usize] as u16,
        HEX[(id & 0xf) as usize] as u16,
        0,
    ];
    // The buffer is always a valid NUL-terminated UCS-2 string by construction.
    CStr16::from_u16_with_nul(buffer).unwrap_or(cstr16!("Boot"))
}

/// One enumerated boot option: its `Boot####` id and its decoded human label.
struct BootOption {
    id: u16,
    label: String,
}

/// Enumerate the machine's active boot options in firmware priority order, exactly as
/// the AMI/ASUS "Boot Option Priorities" list does: read `BootOrder`, then decode each
/// referenced `Boot####`. Skips missing or malformed entries with a marker.
fn enumerate_boot_options() -> Vec<BootOption> {
    let mut options = Vec::new();
    let Some(order) = read_global(cstr16!("BootOrder")) else {
        aw_mark!("AW_UEFI_BOOT_ENUM count=0 reason=no_boot_order");
        return options;
    };
    let mut index = 0u32;
    for pair in order.as_chunks::<2>().0 {
        let id = u16::from_le_bytes(*pair);
        let mut name_buffer = [0u16; 9];
        let name = boot_var_name(id, &mut name_buffer);
        let Some(raw) = read_global(name) else {
            aw_mark!("AW_UEFI_BOOT_SKIP id=0x{:04x} reason=unreadable", id);
            continue;
        };
        let Some(label) = decode_boot_option(&raw) else {
            aw_mark!(
                "AW_UEFI_BOOT_SKIP id=0x{:04x} reason=inactive_or_malformed",
                id
            );
            continue;
        };
        aw_mark!(
            "AW_UEFI_BOOT_ENTRY index={} id=0x{:04x} \"{}\"",
            index,
            id,
            label
        );
        options.push(BootOption { id, label });
        index += 1;
    }
    aw_mark!("AW_UEFI_BOOT_ENUM count={}", options.len());
    options
}

/// Read the one-byte `SecureBoot`/`SetupMode` state into a spoken word (caller supplies
/// the words, so they can be French or English).
fn one_byte_state(name: &CStr16, one: &str, zero: &str, unknown: &str) -> String {
    match read_global(name) {
        Some(bytes) if !bytes.is_empty() => String::from(if bytes[0] == 1 { one } else { zero }),
        _ => String::from(unknown),
    }
}

/// The `Boot####` id the firmware selected for the current boot (`BootCurrent`), as a
/// label, or `None` if the variable is absent.
fn boot_current_label() -> Option<String> {
    read_global(cstr16!("BootCurrent"))
        .filter(|bytes| bytes.len() >= 2)
        .map(|bytes| format!("Boot{:04X}", u16::from_le_bytes([bytes[0], bytes[1]])))
}

/// The firmware boot-manager timeout (`Timeout`, in seconds) as a label, or `None`.
fn boot_timeout_label(lang: Lang) -> Option<String> {
    read_global(cstr16!("Timeout"))
        .filter(|bytes| bytes.len() >= 2)
        .map(|bytes| {
            format!(
                "{} {}",
                u16::from_le_bytes([bytes[0], bytes[1]]),
                tx(lang, "secondes", "seconds")
            )
        })
}

// ---- SMBIOS (system identity) --------------------------------------------------

/// The machine identity a real BIOS shows on its Main page, read from the SMBIOS tables
/// the firmware publishes. Without this a blind user has no way to hear the machine's
/// model, its firmware version, or its serial number.
struct SystemIdentity {
    bios: Option<String>,
    system: Option<String>,
    serial: Option<String>,
}

/// Pull one string from an SMBIOS structure's string-set by its 1-based `index` (0 means
/// "no string"). The set starts right after the `formatted_len`-byte formatted area.
fn smbios_string(
    data: &[u8],
    struct_start: usize,
    formatted_len: usize,
    index: u8,
) -> Option<String> {
    if index == 0 {
        return None;
    }
    let mut position = struct_start + formatted_len;
    let mut current = 1u8;
    while position < data.len() {
        if data[position] == 0 {
            return None; // set terminator reached before the wanted index
        }
        let start = position;
        while position < data.len() && data[position] != 0 {
            position += 1;
        }
        if current == index {
            let text = String::from_utf8_lossy(&data[start..position]);
            let trimmed = text.trim();
            return (!trimmed.is_empty()).then(|| String::from(trimmed));
        }
        position += 1;
        current += 1;
    }
    None
}

/// Read Type 0 (BIOS) and Type 1 (System) from the firmware's SMBIOS structure table:
/// BIOS vendor/version/date, and the machine's manufacturer, product name and serial.
/// The table is copied out of physical memory (bounded) and parsed as bytes, all before
/// ExitBootServices while the firmware still identity-maps memory.
fn read_system_identity() -> SystemIdentity {
    let mut identity = SystemIdentity {
        bios: None,
        system: None,
        serial: None,
    };

    let entry = system::with_config_table(|tables| {
        tables
            .iter()
            .find(|entry| entry.guid == ConfigTableEntry::SMBIOS3_GUID)
            .map(|entry| (entry.address as usize, true))
            .or_else(|| {
                tables
                    .iter()
                    .find(|entry| entry.guid == ConfigTableEntry::SMBIOS_GUID)
                    .map(|entry| (entry.address as usize, false))
            })
    });
    let Some((entry_addr, is_v3)) = entry else {
        return identity;
    };
    if entry_addr == 0 {
        return identity;
    }

    // SAFETY: the entry-point address is the firmware's own SMBIOS config-table entry; 32
    // bytes cover both the 32-bit ("_SM_") and 64-bit ("_SM3_") anchor layouts and are
    // identity-mapped during boot services.
    let header = unsafe { core::slice::from_raw_parts(entry_addr as *const u8, 32) };
    let (table_addr, table_len) = if is_v3 {
        let len =
            u32::from_le_bytes([header[0x0C], header[0x0D], header[0x0E], header[0x0F]]) as usize;
        let addr = u64::from_le_bytes([
            header[0x10],
            header[0x11],
            header[0x12],
            header[0x13],
            header[0x14],
            header[0x15],
            header[0x16],
            header[0x17],
        ]) as usize;
        (addr, len)
    } else {
        let len = u16::from_le_bytes([header[0x16], header[0x17]]) as usize;
        let addr =
            u32::from_le_bytes([header[0x18], header[0x19], header[0x1A], header[0x1B]]) as usize;
        (addr, len)
    };
    // Cap the copy so a bad length cannot request a huge or wrapping read.
    let table_len = table_len.min(64 * 1024);
    if table_addr == 0 || table_len < 4 {
        return identity;
    }
    // SAFETY: address and bounded length name the firmware's SMBIOS structure table,
    // identity-mapped and consumed here before ExitBootServices.
    let data = unsafe { core::slice::from_raw_parts(table_addr as *const u8, table_len) }.to_vec();

    let mut offset = 0usize;
    let mut guard = 0u32;
    while offset + 4 <= data.len() && guard < 1024 {
        guard += 1;
        let structure_type = data[offset];
        let formatted_len = data[offset + 1] as usize;
        if formatted_len < 4 {
            break;
        }
        let field = |relative: usize| -> Option<String> {
            data.get(offset + relative)
                .and_then(|&index| smbios_string(&data, offset, formatted_len, index))
        };
        match structure_type {
            0 => {
                let vendor = field(0x04);
                let version = field(0x05);
                let date = field(0x08);
                identity.bios = match (vendor, version) {
                    (Some(vendor), Some(version)) => Some(match date {
                        Some(date) => format!("{vendor} {version} ({date})"),
                        None => format!("{vendor} {version}"),
                    }),
                    (Some(vendor), None) => Some(vendor),
                    (None, Some(version)) => Some(version),
                    (None, None) => None,
                };
            }
            1 => {
                let manufacturer = field(0x04);
                let product = field(0x05);
                identity.serial = field(0x07);
                identity.system = match (manufacturer, product) {
                    (Some(manufacturer), Some(product)) => {
                        Some(format!("{manufacturer} {product}"))
                    }
                    (Some(manufacturer), None) => Some(manufacturer),
                    (None, Some(product)) => Some(product),
                    (None, None) => None,
                };
            }
            127 => break,
            _ => {}
        }
        // Advance past the formatted area and the string-set (ends at a double NUL).
        let mut end = offset + formatted_len;
        while end + 1 < data.len() && !(data[end] == 0 && data[end + 1] == 0) {
            end += 1;
        }
        offset = end + 2;
    }
    identity
}

// ---- Tree construction ---------------------------------------------------------

/// Build the whole setup tree - top tabs and every submenu - from real machine state, in
/// the chosen language (French or English), so the labels, help and clips baked into each
/// item are already in that language and the render/announce path needs no language logic.
/// Child screens are pushed first so their parents can reference them by index.
fn build_tree(lang: Lang, width: usize, height: usize) -> Tree {
    let mut screens: Vec<Screen> = Vec::new();

    // Submenu: CPU Configuration (child of Advanced).
    let cpu_screen = screens.len();
    screens.push(Screen {
        title: String::from(tx(lang, "Configuration du processeur", "CPU Configuration")),
        title_clip: Some(clip(lang, hda::CLIP_FR_SUB_CPU, hda::CLIP_SUB_CPU)),
        items: alloc::vec![
            Item::info(
                format!(
                    "{}, {}",
                    tx(lang, "Technologie de virtualisation", "Virtualization technology"),
                    virtualization_status(lang)
                ),
                tx(
                    lang,
                    "Prise en charge Intel VT-x ou AMD-V et état du firmware ; se modifie dans la configuration du firmware.",
                    "Intel VT-x or AMD-V support and firmware state; change it in firmware setup.",
                ),
            ),
            Item::info(
                format!("{}, {}", tx(lang, "Processeur", "Processor"), cpu_brand()),
                tx(
                    lang,
                    "La chaîne de marque du processeur.",
                    "The processor brand string reported by the CPU.",
                ),
            ),
            Item::info(
                format!(
                    "{}, {}",
                    tx(lang, "Fournisseur du processeur", "Processor vendor"),
                    cpu_vendor()
                ),
                tx(
                    lang,
                    "L'identifiant du fournisseur du processeur.",
                    "The CPU vendor identification string.",
                ),
            ),
            Item::action(
                tx(lang, "Revenir", "Go back"),
                tx(lang, "Revenir à l'onglet Avancé.", "Return to the Advanced tab."),
                Some(clip(lang, hda::CLIP_FR_ACT_BACK, hda::CLIP_ACT_BACK)),
                Action::Back
            ),
        ],
    });

    // Submenu: Secure Boot (child of Security).
    let secure_screen = screens.len();
    screens.push(Screen {
        title: String::from("Secure Boot"),
        title_clip: Some(clip(lang, hda::CLIP_FR_SUB_SECURE_BOOT, hda::CLIP_SUB_SECURE_BOOT)),
        items: alloc::vec![
            Item::info(
                format!(
                    "Secure Boot, {}",
                    one_byte_state(
                        cstr16!("SecureBoot"),
                        tx(lang, "activé", "enabled"),
                        tx(lang, "désactivé", "disabled"),
                        tx(lang, "inconnu", "unknown"),
                    )
                ),
                tx(
                    lang,
                    "Si le firmware applique la vérification des signatures Secure Boot.",
                    "Whether the firmware is enforcing Secure Boot signature checks.",
                ),
            ),
            Item::info(
                format!(
                    "{}, {}",
                    tx(lang, "Mode de configuration", "Setup Mode"),
                    one_byte_state(
                        cstr16!("SetupMode"),
                        tx(lang, "mode configuration", "setup mode"),
                        tx(lang, "mode utilisateur", "user mode"),
                        tx(lang, "inconnu", "unknown"),
                    )
                ),
                tx(
                    lang,
                    "Si les clés Secure Boot sont provisionnées (mode utilisateur) ou ouvertes (mode configuration).",
                    "Whether Secure Boot keys are provisioned (user mode) or open (setup mode).",
                ),
            ),
            Item::action(
                tx(lang, "Revenir", "Go back"),
                tx(lang, "Revenir à l'onglet Sécurité.", "Return to the Security tab."),
                Some(clip(lang, hda::CLIP_FR_ACT_BACK, hda::CLIP_ACT_BACK)),
                Action::Back
            ),
        ],
    });

    // Submenu: Boot Option Priorities (child of Boot), with one submenu per real boot
    // device. Build each device's action screen first, then the priorities list.
    let options = enumerate_boot_options();
    let mut priority_items: Vec<Item> = Vec::new();
    for option in &options {
        let device_screen = screens.len();
        screens.push(Screen {
            title: option.label.clone(),
            title_clip: Some(clip(lang, hda::CLIP_FR_BOOT_DEVICE, hda::CLIP_BOOT_DEVICE)),
            items: alloc::vec![
                Item::action(
                    tx(lang, "Démarrer ce périphérique maintenant", "Boot this device now"),
                    tx(
                        lang,
                        "Démarrer le périphérique sélectionné au prochain redémarrage.",
                        "Boot the selected device on the next restart.",
                    ),
                    Some(clip(lang, hda::CLIP_FR_ACT_BOOT_NOW, hda::CLIP_ACT_BOOT_NOW)),
                    Action::BootNow(option.id),
                ),
                Item::action(
                    tx(
                        lang,
                        "Définir comme périphérique de démarrage par défaut",
                        "Make this the default boot device",
                    ),
                    tx(
                        lang,
                        "Placer ce périphérique en tête de l'ordre de démarrage, de façon permanente.",
                        "Put this device first in the firmware boot order, permanently.",
                    ),
                    Some(clip(lang, hda::CLIP_FR_ACT_MAKE_DEFAULT, hda::CLIP_ACT_MAKE_DEFAULT)),
                    Action::MakeDefault(option.id),
                ),
                Item::action(
                    tx(lang, "Monter dans l'ordre de démarrage", "Move up in boot order"),
                    tx(
                        lang,
                        "Monter ce périphérique d'une place dans l'ordre de démarrage, de façon permanente.",
                        "Move this device one place earlier in the boot order, permanently.",
                    ),
                    Some(clip(lang, hda::CLIP_FR_ACT_MOVE_UP, hda::CLIP_ACT_MOVE_UP)),
                    Action::MoveUp(option.id),
                ),
                Item::action(
                    tx(lang, "Descendre dans l'ordre de démarrage", "Move down in boot order"),
                    tx(
                        lang,
                        "Descendre ce périphérique d'une place dans l'ordre de démarrage, de façon permanente.",
                        "Move this device one place later in the boot order, permanently.",
                    ),
                    Some(clip(lang, hda::CLIP_FR_ACT_MOVE_DOWN, hda::CLIP_ACT_MOVE_DOWN)),
                    Action::MoveDown(option.id),
                ),
                Item::action(
                    tx(lang, "Revenir", "Go back"),
                    tx(lang, "Revenir à la liste des périphériques.", "Return to the boot device list."),
                    Some(clip(lang, hda::CLIP_FR_ACT_BACK, hda::CLIP_ACT_BACK)),
                    Action::Back,
                ),
            ],
        });
        priority_items.push(Item::dynamic(
            option.label.clone(),
            tx(
                lang,
                "Ouvrir ce périphérique pour le démarrer ou le définir par défaut.",
                "Open this boot device to boot it now or make it the default.",
            ),
            Some(clip(lang, hda::CLIP_FR_BOOT_DEVICE, hda::CLIP_BOOT_DEVICE)),
            Action::SubMenu(device_screen),
        ));
    }
    priority_items.push(Item::action(
        tx(lang, "Revenir", "Go back"),
        tx(
            lang,
            "Revenir à l'onglet Démarrage.",
            "Return to the Boot tab.",
        ),
        Some(clip(lang, hda::CLIP_FR_ACT_BACK, hda::CLIP_ACT_BACK)),
        Action::Back,
    ));
    let priorities_screen = screens.len();
    screens.push(Screen {
        title: String::from(tx(lang, "Priorités de démarrage", "Boot Option Priorities")),
        title_clip: Some(clip(
            lang,
            hda::CLIP_FR_SUB_BOOT_PRIO,
            hda::CLIP_SUB_BOOT_PRIO,
        )),
        items: priority_items,
    });

    // Top tab: Main. Lead with the Language selector (a real ASUS/AMI BIOS puts "System
    // Language" on the Main page), then the SMBIOS system identity a real BIOS shows - the
    // machine's model, serial and BIOS version - which a blind user otherwise cannot hear.
    let identity = read_system_identity();
    let mut main_items: Vec<Item> = Vec::new();
    main_items.push(Item::action(
        tx(lang, "Langue : Français", "Language: English"),
        tx(
            lang,
            "Choisir la langue de cet utilitaire ; appuyez sur Entrée pour passer en anglais.",
            "Choose this utility's language; press Enter to switch to French.",
        ),
        Some(clip(lang, hda::CLIP_FR_LANG, hda::CLIP_ACT_LANGUAGE)),
        Action::ToggleLang,
    ));
    if let Some(system_name) = identity.system {
        main_items.push(Item::info(
            format!("{}, {system_name}", tx(lang, "Système", "System")),
            tx(
                lang,
                "Le fabricant et le modèle de la machine, d'après SMBIOS.",
                "The machine's manufacturer and model, from SMBIOS.",
            ),
        ));
    }
    if let Some(serial) = identity.serial {
        main_items.push(Item::info(
            format!("{}, {serial}", tx(lang, "Numéro de série", "Serial number")),
            tx(
                lang,
                "Le numéro de série de la machine, d'après SMBIOS.",
                "The machine's serial number, from SMBIOS.",
            ),
        ));
    }
    if let Some(bios) = identity.bios {
        main_items.push(Item::info(
            format!("BIOS, {bios}"),
            tx(
                lang,
                "Le fournisseur, la version et la date du BIOS, d'après SMBIOS.",
                "The BIOS vendor, version and release date, from SMBIOS.",
            ),
        ));
    }
    main_items.push(Item::info(
        format!(
            "{}, {}",
            tx(lang, "Fournisseur du firmware", "Firmware vendor"),
            system::firmware_vendor()
        ),
        tx(
            lang,
            "Le firmware UEFI qui a démarré cette machine.",
            "The UEFI firmware that started this machine.",
        ),
    ));
    let revision = system::firmware_revision();
    main_items.push(Item::info(
        format!(
            "{}, {}.{}",
            tx(lang, "Version du firmware", "Firmware version"),
            revision >> 16,
            revision & 0xffff
        ),
        tx(
            lang,
            "Le numéro de version du firmware.",
            "The firmware's own version number.",
        ),
    ));
    let uefi = system::uefi_revision();
    main_items.push(Item::info(
        format!(
            "{}, {}.{}",
            tx(lang, "Version UEFI", "UEFI version"),
            uefi.major(),
            uefi.minor()
        ),
        tx(
            lang,
            "La révision de la spécification UEFI implémentée par le firmware.",
            "The UEFI specification revision the firmware implements.",
        ),
    ));
    if let Ok(time) = runtime::get_time() {
        main_items.push(Item::info(
            format!(
                "{}, {:04}-{:02}-{:02} {:02}:{:02}:{:02}",
                tx(lang, "Heure système", "System time"),
                time.year(),
                time.month(),
                time.day(),
                time.hour(),
                time.minute(),
                time.second()
            ),
            tx(
                lang,
                "La date et l'heure de l'horloge temps réel.",
                "The real-time clock's current date and time.",
            ),
        ));
    }
    main_items.push(Item::info(
        format!(
            "{}, {} {}",
            tx(lang, "Mémoire installée", "Installed memory"),
            installed_memory_mib(),
            tx(lang, "Mio", "mebibytes")
        ),
        tx(
            lang,
            "La mémoire totale rapportée par le firmware dans sa carte mémoire.",
            "Total memory the firmware reported in its memory map.",
        ),
    ));
    main_items.push(Item::info(
        format!(
            "{}, {} {} {}",
            tx(lang, "Résolution d'affichage", "Display resolution"),
            width,
            tx(lang, "par", "by"),
            height
        ),
        tx(
            lang,
            "Le mode graphique fourni par le firmware à Accessible Windows.",
            "The graphics mode the firmware handed to Accessible Windows.",
        ),
    ));
    let main_screen = screens.len();
    screens.push(Screen {
        title: String::from(tx(lang, "Principal", "Main")),
        title_clip: Some(clip(lang, hda::CLIP_FR_TAB_MAIN, hda::CLIP_TAB_MAIN)),
        items: main_items,
    });

    // Top tab: Advanced (one submenu today: CPU Configuration).
    let advanced_screen = screens.len();
    screens.push(Screen {
        title: String::from(tx(lang, "Avancé", "Advanced")),
        title_clip: Some(clip(
            lang,
            hda::CLIP_FR_TAB_ADVANCED,
            hda::CLIP_TAB_ADVANCED,
        )),
        items: alloc::vec![Item::action(
            tx(lang, "Configuration du processeur", "CPU Configuration"),
            tx(
                lang,
                "Détails du processeur et état de la virtualisation.",
                "Processor details and virtualization state.",
            ),
            Some(clip(lang, hda::CLIP_FR_SUB_CPU, hda::CLIP_SUB_CPU)),
            Action::SubMenu(cpu_screen),
        ),],
    });

    // Top tab: Boot (Boot normally + the priorities submenu, then the boot-manager state
    // a real BIOS shows: which entry booted this time, and the boot-menu timeout).
    let mut boot_items = alloc::vec![
        Item::action(
            tx(lang, "Démarrer normalement", "Boot normally"),
            tx(
                lang,
                "Continuer et charger Accessible Windows.",
                "Continue and load Accessible Windows now."
            ),
            Some(clip(
                lang,
                hda::CLIP_FR_ACT_BOOT_NORMALLY,
                hda::CLIP_ACT_BOOT_NORMALLY
            )),
            Action::BootNormally,
        ),
        Item::action(
            tx(lang, "Priorités de démarrage", "Boot Option Priorities"),
            tx(
                lang,
                "Les périphériques de démarrage de la machine, à démarrer ou réordonner.",
                "The machine's boot devices, to boot now or reorder.",
            ),
            Some(clip(
                lang,
                hda::CLIP_FR_SUB_BOOT_PRIO,
                hda::CLIP_SUB_BOOT_PRIO
            )),
            Action::SubMenu(priorities_screen),
        ),
    ];
    if let Some(current) = boot_current_label() {
        boot_items.push(Item::info(
            format!(
                "{}, {current}",
                tx(lang, "Démarrage actuel", "Boot current")
            ),
            tx(
                lang,
                "L'entrée de démarrage sélectionnée par le firmware pour ce démarrage.",
                "The boot entry the firmware selected for the current boot.",
            ),
        ));
    }
    if let Some(timeout) = boot_timeout_label(lang) {
        boot_items.push(Item::info(
            format!(
                "{}, {timeout}",
                tx(lang, "Délai du menu de démarrage", "Boot menu timeout")
            ),
            tx(
                lang,
                "Le temps d'attente du menu de démarrage du firmware.",
                "How long the firmware's own boot menu waits before booting.",
            ),
        ));
    }
    let boot_screen = screens.len();
    screens.push(Screen {
        title: String::from(tx(lang, "Démarrage", "Boot")),
        title_clip: Some(clip(lang, hda::CLIP_FR_TAB_BOOT, hda::CLIP_TAB_BOOT)),
        items: boot_items,
    });

    // Top tab: Security (Secure Boot submenu).
    let security_screen = screens.len();
    screens.push(Screen {
        title: String::from(tx(lang, "Sécurité", "Security")),
        title_clip: Some(clip(
            lang,
            hda::CLIP_FR_TAB_SECURITY,
            hda::CLIP_TAB_SECURITY,
        )),
        items: alloc::vec![Item::action(
            "Secure Boot",
            tx(
                lang,
                "État de Secure Boot et du mode de configuration.",
                "Secure Boot and Setup Mode state."
            ),
            Some(clip(
                lang,
                hda::CLIP_FR_SUB_SECURE_BOOT,
                hda::CLIP_SUB_SECURE_BOOT
            )),
            Action::SubMenu(secure_screen),
        ),],
    });

    // Top tab: Save and Exit (the actions).
    let save_exit_screen = screens.len();
    screens.push(Screen {
        title: String::from(tx(lang, "Enregistrer et quitter", "Save and Exit")),
        title_clip: Some(clip(
            lang,
            hda::CLIP_FR_TAB_SAVEEXIT,
            hda::CLIP_TAB_SAVEEXIT,
        )),
        items: alloc::vec![
            Item::action(
                tx(lang, "Démarrer normalement", "Boot normally"),
                tx(
                    lang,
                    "Continuer et charger Accessible Windows.",
                    "Continue and load Accessible Windows now."
                ),
                Some(clip(
                    lang,
                    hda::CLIP_FR_ACT_BOOT_NORMALLY,
                    hda::CLIP_ACT_BOOT_NORMALLY
                )),
                Action::BootNormally,
            ),
            Item::action(
                tx(
                    lang,
                    "Entrer dans la configuration du firmware",
                    "Enter firmware setup"
                ),
                tx(
                    lang,
                    "Redémarrer dans l'écran de configuration du firmware.",
                    "Restart into the firmware's own setup screen.",
                ),
                Some(clip(
                    lang,
                    hda::CLIP_FR_ACT_ENTER_SETUP,
                    hda::CLIP_ACT_ENTER_SETUP
                )),
                Action::EnterSetup,
            ),
            Item::action(
                tx(lang, "Redémarrer le système", "Reset the system"),
                tx(
                    lang,
                    "Redémarrer la machine maintenant.",
                    "Restart the machine now."
                ),
                Some(clip(lang, hda::CLIP_FR_ACT_RESET, hda::CLIP_ACT_RESET)),
                Action::Reset,
            ),
            Item::action(
                tx(lang, "Éteindre le système", "Shut down the system"),
                tx(
                    lang,
                    "Éteindre la machine maintenant.",
                    "Power the machine off now."
                ),
                Some(clip(
                    lang,
                    hda::CLIP_FR_ACT_SHUTDOWN,
                    hda::CLIP_ACT_SHUTDOWN
                )),
                Action::Shutdown,
            ),
        ],
    });

    Tree {
        screens,
        tabs: alloc::vec![
            main_screen,
            advanced_screen,
            boot_screen,
            security_screen,
            save_exit_screen,
        ],
        boot_tab: 2,
    }
}

// ---- Rendering and speech ------------------------------------------------------

/// Draw the current screen on the visible console: title, the tab bar (when at a top
/// tab), the items with the focus marked, the focused item's help line, and the key
/// legend. This is the sighted mirror of what is spoken; the spoken form is primary.
fn render(tree: &Tree, tab_index: usize, screen_index: usize, item_index: usize, depth: usize) {
    let _ = system::with_stdout(|stdout| stdout.clear());
    uefi::println!("Accessible Windows Setup Utility");

    if depth == 0 {
        let mut bar = String::new();
        for (index, &screen) in tree.tabs.iter().enumerate() {
            if index == tab_index {
                bar.push_str(&format!("[{}]  ", tree.screens[screen].title));
            } else {
                bar.push_str(&format!(" {}   ", tree.screens[screen].title));
            }
        }
        uefi::println!("{bar}");
    } else {
        uefi::println!("{}", tree.screens[screen_index].title);
    }
    uefi::println!();

    let screen = &tree.screens[screen_index];
    for (index, item) in screen.items.iter().enumerate() {
        let marker = if index == item_index { ">" } else { " " };
        uefi::println!("  {marker} {}", item.text);
    }

    uefi::println!();
    if let Some(item) = screen.items.get(item_index) {
        uefi::println!("  {}", item.help);
    }
    uefi::println!();
    if depth == 0 {
        uefi::println!(
            "  Left/Right: tab.  Up/Down: item.  Enter: select.  Esc: boot normally.  Space: repeat.  A: read all.  S: spell.  C: command.  Plus/minus: volume.  M: mute.  P: phonetic.  H: help."
        );
    } else {
        uefi::println!(
            "  Up/Down: item.  Enter: select.  Esc: back.  Space: repeat.  A: read all.  S: spell.  C: command.  Plus/minus: volume.  M: mute.  P: phonetic.  H: help.  W: where."
        );
    }
}

/// Announce a screen's title through the one screen-reader engine and speak its clip:
/// a top tab as "<Name>, tab, <n> of 5" (`AW_UEFI_SETUP_TAB`), a submenu by title
/// (`AW_UEFI_SETUP_MENU`).
fn announce_screen(
    tree: &Tree,
    tab_index: usize,
    screen_index: usize,
    depth: usize,
    speaker: &mut Option<audio::Speaker>,
    pending: &mut Option<Key>,
) {
    let screen = &tree.screens[screen_index];
    let (name, context) = if depth == 0 {
        (
            format!("{}, tab", screen.title),
            FocusContext::in_set(tab_index as u32 + 1, tree.tabs.len() as u32),
        )
    } else {
        (screen.title.clone(), FocusContext::NONE)
    };
    let node = SemanticNode {
        id: NodeId(300),
        parent: Some(NodeId(0)),
        role: Role::StaticText,
        name: &name,
        description: "",
        value: "",
        state: State::from_bits(0),
        bounds: Rect {
            x: 0,
            y: 0,
            width: 640,
            height: 32,
        },
    };
    if validate_node(&node).is_err() {
        log::error!("AW_UEFI_MENU_FAIL reason=invalid_screen");
        return;
    }
    let mut buffer = [0u8; 192];
    let text = announce_focus(&node, context, &mut buffer);
    if depth == 0 {
        aw_mark!("AW_UEFI_SETUP_TAB \"{text}\"");
    } else {
        aw_mark!("AW_UEFI_SETUP_MENU \"{text}\"");
    }
    if let Some(clip) = screen.title_clip {
        play(clip, speaker, pending);
    }
}

/// Announce the focused item through the engine and speak its clip. A selectable row is
/// a "menu item" (`AW_UEFI_MENU_ITEM`), a read-only row is static text
/// (`AW_UEFI_SETUP_ITEM`). Returns false only on an invariant violation (a bug).
fn announce_item(
    screen: &Screen,
    item_index: usize,
    speaker: &mut Option<audio::Speaker>,
    pending: &mut Option<Key>,
) -> bool {
    let Some(item) = screen.items.get(item_index) else {
        return true;
    };
    let node = SemanticNode {
        id: NodeId(400 + item_index as u64),
        parent: Some(NodeId(0)),
        role: item.action.role(),
        name: &item.text,
        description: "",
        value: "",
        state: match item.action {
            Action::Info => State::from_bits(0),
            _ => State::from_bits(State::FOCUSABLE),
        },
        bounds: Rect {
            x: 0,
            y: 0,
            width: 640,
            height: 32,
        },
    };
    if validate_node(&node).is_err() {
        log::error!("AW_UEFI_MENU_FAIL reason=invalid_item");
        return false;
    }
    let mut buffer = [0u8; 192];
    let context = FocusContext::in_set(item_index as u32 + 1, screen.items.len() as u32);
    let text = announce_focus(&node, context, &mut buffer);
    match item.action {
        Action::Info => aw_mark!("AW_UEFI_SETUP_ITEM \"{text}\""),
        _ => aw_mark!("AW_UEFI_MENU_ITEM \"{text}\""),
    }
    if let Some(clip) = item.clip {
        play(clip, speaker, pending);
    }
    // Read-only state lines (CPU, memory, display, Secure Boot, virtualization) carry their
    // value only in text, with no whole-line clip. Speak it automatically on focus - spelled
    // through the alphabet bank - so a blind user hears the value on arrival instead of
    // having to ask for it with the spell key. Interruptible: moving on stops a long value.
    if matches!(item.action, Action::Info) {
        let french = CURRENT_FRENCH.load(core::sync::atomic::Ordering::Relaxed);
        spell_chars(&item.text, french, speaker, pending);
    }
    true
}

// ---- Actions -------------------------------------------------------------------

/// Set `BootNext` to `id` so the firmware boots that option on the next restart, then
/// cold-reset. Returns without resetting only if the variable write fails, so a failure
/// never strands the user.
fn boot_now(id: u16) {
    let data = id.to_le_bytes();
    match runtime::set_variable(
        cstr16!("BootNext"),
        &VariableVendor::GLOBAL_VARIABLE,
        boot_var_attributes(),
        &data,
    ) {
        Ok(()) => {
            aw_mark!(
                "AW_UEFI_MENU_SELECT name=\"boot_now\" boot_next=0x{:04x}",
                id
            );
            runtime::reset(runtime::ResetType::COLD, Status::SUCCESS, None);
        }
        Err(error) => {
            log::error!(
                "AW_UEFI_MENU_FAIL reason=boot_next status={:?}",
                error.status()
            );
        }
    }
}

/// Rewrite `BootOrder` so `id` is first, making it the persistent default boot device -
/// the reorder a blind user cannot do in a silent firmware. Reads the current order,
/// moves `id` to the front, and writes it back; stays in the menu on success (no reset),
/// so the user can keep reviewing. Returns whether the order was changed.
fn make_default(id: u16) -> bool {
    let Some(order) = read_global(cstr16!("BootOrder")) else {
        log::error!("AW_UEFI_MENU_FAIL reason=make_default_no_order");
        return false;
    };
    let mut ids: Vec<u16> = order
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u16::from_le_bytes(*p))
        .collect();
    ids.retain(|&existing| existing != id);
    ids.insert(0, id);
    let mut bytes = Vec::with_capacity(ids.len() * 2);
    for value in &ids {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    match runtime::set_variable(
        cstr16!("BootOrder"),
        &VariableVendor::GLOBAL_VARIABLE,
        boot_var_attributes(),
        &bytes,
    ) {
        Ok(()) => {
            aw_mark!(
                "AW_UEFI_MENU_SELECT name=\"make_default\" boot_first=0x{:04x}",
                id
            );
            true
        }
        Err(error) => {
            log::error!(
                "AW_UEFI_MENU_FAIL reason=boot_order status={:?}",
                error.status()
            );
            false
        }
    }
}

/// Move `id` one position earlier (`up`) or later in `BootOrder` - a finer, persistent
/// reorder than "make default", the boot priority a blind user could not otherwise change.
/// A no-op (returns false) if `id` is absent or already at the end it is moving toward.
fn move_in_boot_order(id: u16, up: bool) -> bool {
    let Some(order) = read_global(cstr16!("BootOrder")) else {
        log::error!("AW_UEFI_MENU_FAIL reason=move_no_order");
        return false;
    };
    let mut ids: Vec<u16> = order
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u16::from_le_bytes(*p))
        .collect();
    let Some(pos) = ids.iter().position(|&existing| existing == id) else {
        return false;
    };
    let target = if up {
        if pos == 0 {
            return false;
        }
        pos - 1
    } else {
        if pos + 1 >= ids.len() {
            return false;
        }
        pos + 1
    };
    ids.swap(pos, target);
    let mut bytes = Vec::with_capacity(ids.len() * 2);
    for value in &ids {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    match runtime::set_variable(
        cstr16!("BootOrder"),
        &VariableVendor::GLOBAL_VARIABLE,
        boot_var_attributes(),
        &bytes,
    ) {
        Ok(()) => {
            aw_mark!(
                "AW_UEFI_MENU_SELECT name=\"move_{}\" boot=0x{:04x} position={}",
                if up { "up" } else { "down" },
                id,
                target
            );
            true
        }
        Err(error) => {
            log::error!(
                "AW_UEFI_MENU_FAIL reason=boot_order status={:?}",
                error.status()
            );
            false
        }
    }
}

/// Request the firmware's own setup UI on the next boot through `OsIndications`, then
/// cold-reset - but only when the firmware advertises support in `OsIndicationsSupported`.
/// Otherwise it is a no-op with a marker, never a lie.
fn enter_firmware_setup() {
    let read_u64 = |name| {
        read_global(name)
            .filter(|bytes| bytes.len() >= 8)
            .map(|bytes| {
                u64::from_le_bytes([
                    bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
                ])
            })
            .unwrap_or(0)
    };
    if read_u64(cstr16!("OsIndicationsSupported")) & OS_INDICATIONS_BOOT_TO_FW_UI == 0 {
        aw_mark!("AW_UEFI_MENU_SETUP_UNSUPPORTED");
        return;
    }
    let current = read_u64(cstr16!("OsIndications")) | OS_INDICATIONS_BOOT_TO_FW_UI;
    match runtime::set_variable(
        cstr16!("OsIndications"),
        &VariableVendor::GLOBAL_VARIABLE,
        boot_var_attributes(),
        &current.to_le_bytes(),
    ) {
        Ok(()) => {
            aw_mark!("AW_UEFI_MENU_SELECT name=\"enter_setup\"");
            runtime::reset(runtime::ResetType::COLD, Status::SUCCESS, None);
        }
        Err(error) => {
            log::error!(
                "AW_UEFI_MENU_FAIL reason=os_indications status={:?}",
                error.status()
            );
        }
    }
}

// ---- Input and loop ------------------------------------------------------------

/// Read one key from the firmware console without blocking. Any read error is treated as
/// "no key", so a flaky console cannot wedge the boot.
fn read_key_raw() -> Option<Key> {
    system::with_stdin(|stdin| stdin.read_key().unwrap_or(None))
}

/// Play a clip with barge-in - the screen-reader behaviour of stopping speech the moment
/// the user acts. If a key is already queued in `pending`, the clip is skipped entirely;
/// if a key arrives while it plays, the clip is cut short and that key stashed in
/// `pending`, so the caller handles it next instead of the setup talking over it. A no-op
/// when there is no audio codec.
fn play(clip: &'static [u8], speaker: &mut Option<audio::Speaker>, pending: &mut Option<Key>) {
    if pending.is_some() {
        return;
    }
    if let Some(sp) = speaker.as_mut() {
        let mut hit: Option<Key> = None;
        // `speak_until` returns whether the DMA link position actually advanced - i.e.
        // whether the codec really streamed the samples. A codec can be present, brought
        // up and unmuted yet still stream nothing (a topology this driver mis-routed, a
        // disconnected jack): the classic way "the audio works" turns out unreliable.
        let advanced = sp.speak_until(clip, || {
            if hit.is_none() {
                hit = read_key_raw();
            }
            hit.is_some()
        });
        if hit.is_some() {
            *pending = hit;
        } else if !advanced {
            // The words did not sound. Guarantee audible feedback with a PC-speaker cue,
            // so a blind user is never left with silence they cannot tell from a hang.
            aw_mark!("AW_UEFI_AUDIO_FALLBACK reason=stream_silent");
            sound::cue(CUE_READY_HZ, Duration::from_millis(60));
        }
    }
}

/// Error prevention: before an irreversible action - one that reboots or powers the
/// machine off - require a second, deliberate keystroke. The prompt is spoken and shown;
/// Enter confirms, anything else cancels. Returns whether to proceed. The user has already
/// pressed a key to get here, so this deliberately blocks; the timed proof, which presses
/// no key, never selects such an action and so never reaches it.
fn confirm(lang: Lang, speaker: &mut Option<audio::Speaker>, pending: &mut Option<Key>) -> bool {
    uefi::println!(
        "  {}",
        tx(
            lang,
            "Appuyez à nouveau sur Entrée pour confirmer, ou Échap pour annuler.",
            "Press Enter again to confirm, or Escape to cancel.",
        )
    );
    aw_mark!("AW_UEFI_SETUP_CONFIRM");
    sound::cue(CUE_READY_HZ, Duration::from_millis(90));
    play(
        clip(lang, hda::CLIP_FR_CONFIRM_PROMPT, hda::CLIP_CONFIRM_PROMPT),
        speaker,
        pending,
    );
    loop {
        if let Some(key) = pending.take().or_else(read_key_raw) {
            if matches!(classify(key), Nav::Select) {
                return true;
            }
            aw_mark!("AW_UEFI_SETUP_CONFIRM_CANCEL");
            play(
                clip(lang, hda::CLIP_FR_CONFIRM_CANCEL, hda::CLIP_CONFIRM_CANCEL),
                speaker,
                pending,
            );
            return false;
        }
        boot::stall(POLL_INTERVAL);
    }
}

/// A keystroke translated into a setup command. Beyond navigation, the screen-reader
/// affordances a blind user expects: repeat the current item, read its help, say where
/// they are, and jump to the first or last item.
enum Nav {
    NextTab,
    PreviousTab,
    NextItem,
    PreviousItem,
    First,
    Last,
    Select,
    /// Escape: step back out, or boot normally at the top level.
    Back,
    /// Re-read the focused item (Space).
    Repeat,
    /// Read the focused item's help line (H or F1).
    Help,
    /// Say where we are: the screen title and the item's position (W).
    Where,
    /// Spell the focused item character by character (S), for dynamic names.
    Spell,
    /// Read every item on the current screen top to bottom (A), interruptibly.
    SayAll,
    /// Open the command agent (C): type a plain instruction instead of walking the tree.
    Command,
    /// Raise the speech volume (+ or =).
    VolumeUp,
    /// Lower the speech volume (-).
    VolumeDown,
    /// Toggle mute (M).
    Mute,
    /// Toggle NATO phonetic spelling (P).
    Phonetic,
    Ignore,
}

/// Map a keystroke to a command: Left/Right change tab, Up/Down (or Tab) move the
/// highlight, Home/End jump to the ends, Enter opens/selects, Escape steps back (or boots
/// normally at the top). The screen-reader keys - Space (repeat), H or F1 (help), W
/// (where am I) - work on every screen and never change what is focused.
fn classify(key: Key) -> Nav {
    match key {
        Key::Special(ScanCode::RIGHT) => Nav::NextTab,
        Key::Special(ScanCode::LEFT) => Nav::PreviousTab,
        Key::Special(ScanCode::DOWN) => Nav::NextItem,
        Key::Special(ScanCode::UP) => Nav::PreviousItem,
        Key::Special(ScanCode::HOME) => Nav::First,
        Key::Special(ScanCode::END) => Nav::Last,
        Key::Special(ScanCode::ESCAPE) => Nav::Back,
        Key::Special(ScanCode::FUNCTION_1) => Nav::Help,
        Key::Printable(character) => match char::from(character) {
            '\r' => Nav::Select,
            '\t' => Nav::NextItem,
            ' ' => Nav::Repeat,
            'h' | 'H' => Nav::Help,
            'w' | 'W' => Nav::Where,
            's' | 'S' => Nav::Spell,
            'a' | 'A' => Nav::SayAll,
            'c' | 'C' => Nav::Command,
            '+' | '=' => Nav::VolumeUp,
            '-' | '_' => Nav::VolumeDown,
            'm' | 'M' => Nav::Mute,
            'p' | 'P' => Nav::Phonetic,
            _ => Nav::Ignore,
        },
        Key::Special(_) => Nav::Ignore,
    }
}

/// Read the focused item's help line aloud: on the console and as an
/// `AW_UEFI_SETUP_HELP` marker. Help text is composed at runtime, so it has no clip;
/// this is the "what is this?" a screen-reader user presses H for.
fn announce_help(screen: &Screen, item_index: usize) {
    if let Some(item) = screen.items.get(item_index) {
        uefi::println!("  {}", item.help);
        aw_mark!("AW_UEFI_SETUP_HELP \"{}\"", item.help);
    }
}

/// Spell the focused line character by character through the HDA codec - a screen
/// reader's "read by character", and the answer to the one line a blind user cannot
/// otherwise hear by name: the runtime-composed device names and machine-state values
/// that have no whole-line clip. Letters, digits and spaces are spoken from the spelling
/// alphabet, and meaningful punctuation is spoken by name in the active language so a
/// value's separators are not silently lost. The full text is also emitted as a marker and
/// shown on the console.
fn spell_current(
    text: &str,
    lang: Lang,
    speaker: &mut Option<audio::Speaker>,
    pending: &mut Option<Key>,
) {
    uefi::println!("  Spelling: {text}");
    aw_mark!("AW_UEFI_SETUP_SPELL \"{text}\"");
    spell_chars(text, matches!(lang, Lang::Fr), speaker, pending);
}

/// Play one clip per character of `text`, honouring phonetic mode and language, and
/// stopping at once on any key (barge-in). The shared core of both the explicit "spell"
/// command and the automatic reading of a focused value.
fn spell_chars(
    text: &str,
    french: bool,
    speaker: &mut Option<audio::Speaker>,
    pending: &mut Option<Key>,
) {
    for character in text.chars() {
        if pending.is_some() {
            break;
        }
        if let Some(clip) = hda::spell_clip(character, french) {
            play(clip, speaker, pending);
        }
    }
}

/// The active setup language, mirrored into a flag so [`announce_item`] can read a focused
/// value aloud without threading the language through every call site. Set when the setup
/// starts and whenever the language is toggled.
static CURRENT_FRENCH: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(true);

/// Record the active language for [`announce_item`]'s automatic value reading.
fn set_current_lang(lang: Lang) {
    CURRENT_FRENCH.store(
        matches!(lang, Lang::Fr),
        core::sync::atomic::Ordering::Relaxed,
    );
}

/// The command agent: type a plain instruction ("boot usb", "secure boot", "restart")
/// and it speaks back what it understood and carries it out - one flat command surface
/// instead of walking the whole tree, which a screen-reader user often finds faster. It
/// does the things a loaded UEFI application is allowed to do (boot a device now, set the
/// default boot device, open the firmware's own setup, restart, shut down). Firmware-owned
/// settings a loaded app cannot change - Secure Boot (immutable by the UEFI spec), the
/// virtualization straps - are read aloud and routed to the firmware's own setup instead of
/// pretending to toggle them. Typed characters are echoed and spoken; Enter runs, Escape
/// cancels, Backspace edits.
fn run_agent(lang: Lang, speaker: &mut Option<audio::Speaker>, pending: &mut Option<Key>) {
    let french = matches!(lang, Lang::Fr);
    uefi::println!();
    uefi::println!("Command >");
    aw_mark!("AW_UEFI_AGENT_OPEN");
    play(hda::agent_clip(hda::AGENT_PROMPT, french), speaker, pending);

    let mut buffer = String::new();
    loop {
        let Some(key) = pending.take().or_else(read_key_raw) else {
            boot::stall(POLL_INTERVAL);
            continue;
        };
        match key {
            Key::Special(ScanCode::ESCAPE) => {
                uefi::println!();
                aw_mark!("AW_UEFI_AGENT_CANCEL");
                return;
            }
            Key::Printable(character) => match char::from(character) {
                '\r' => break,
                // Backspace: drop the last character and redraw the line.
                '\u{8}' => {
                    buffer.pop();
                    uefi::print!("\rCommand > {buffer} \r");
                    uefi::print!("Command > {buffer}");
                }
                ch => {
                    buffer.push(ch);
                    uefi::print!("{ch}");
                    // Echo the typed character aloud, so a blind user hears what they enter.
                    if let Some(clip) = hda::spell_clip(ch, french) {
                        play(clip, speaker, pending);
                    }
                }
            },
            Key::Special(_) => {}
        }
    }
    uefi::println!();
    let command = buffer.trim().to_ascii_lowercase();
    aw_mark!("AW_UEFI_AGENT_COMMAND \"{command}\"");
    dispatch_agent(&command, lang, french, speaker, pending);
}

/// Match one typed command to an intent and carry it out. Keyword matching accepts both
/// languages, so "boot usb" and "demarrer usb" both work. Ordered from most specific to
/// least, and terminal actions (boot, restart, shut down, open firmware setup) never
/// return because they reset the machine.
fn dispatch_agent(
    cmd: &str,
    lang: Lang,
    french: bool,
    speaker: &mut Option<audio::Speaker>,
    pending: &mut Option<Key>,
) {
    let ag = |pair| hda::agent_clip(pair, french);
    let has = |needle: &str| cmd.contains(needle);

    if cmd.is_empty() {
        return;
    }

    // Help: speak the list of commands.
    if has("help") || has("aide") || cmd == "?" {
        play(ag(hda::AGENT_HELP), speaker, pending);
        return;
    }

    // Boot management, on the machine's real Boot#### entries: list them, or boot / set as
    // default any one - by name ("boot usb", "boot windows") or by its position ("boot 2").
    if has("boot") || has("demarr") || has("default") || has("defaut") || has("par def") {
        let options = enumerate_boot_options();

        // "list" / "liste": read every entry with its number, so a blind user can choose.
        if has("list") || has("liste") {
            play(ag(hda::AGENT_BOOT_LIST), speaker, pending);
            for (index, opt) in options.iter().enumerate() {
                if pending.is_some() {
                    break;
                }
                // Number, then the device name spelled out.
                let line = format!("{}. {}", index + 1, opt.label);
                spell_current(&line, lang, speaker, pending);
            }
            return;
        }

        let want_default = has("default") || has("defaut") || has("par def");
        let target = find_boot_target(cmd, &options);
        match target {
            None => play(ag(hda::AGENT_NO_MATCH), speaker, pending),
            Some(opt) if want_default => {
                if make_default(opt.id) {
                    play(ag(hda::AGENT_SET_DEFAULT), speaker, pending);
                }
            }
            Some(opt) => {
                play(ag(hda::AGENT_BOOTING), speaker, pending);
                boot_now(opt.id); // sets BootNext and resets; does not return
            }
        }
        return;
    }

    // Secure Boot: a loaded application cannot change it (the SecureBoot variable is
    // immutable per the UEFI spec), so read its state aloud and route to firmware setup.
    if has("secure") {
        let state = one_byte_state(
            cstr16!("SecureBoot"),
            tx(lang, "active", "enabled"),
            tx(lang, "desactive", "disabled"),
            tx(lang, "inconnu", "unknown"),
        );
        play(ag(hda::AGENT_SECURE_BOOT_IS), speaker, pending);
        spell_current(&state, lang, speaker, pending);
        play(ag(hda::AGENT_FIRMWARE_ONLY), speaker, pending);
        enter_firmware_setup(); // resets into firmware setup when supported
        play(ag(hda::AGENT_SETUP_DENIED), speaker, pending);
        return;
    }

    // Virtualization: also a firmware-owned strap. Speak the live CPU state and route.
    if has("virtu") || has("vt-x") || has("vmx") || has("svm") {
        let state = virtualization_status(lang);
        play(ag(hda::AGENT_VALUE_IS), speaker, pending);
        spell_current(&state, lang, speaker, pending);
        play(ag(hda::AGENT_FIRMWARE_ONLY), speaker, pending);
        enter_firmware_setup();
        play(ag(hda::AGENT_SETUP_DENIED), speaker, pending);
        return;
    }

    // System information: print the full machine facts (including the long CPU brand) on
    // the console, and speak the shorter ones - installed memory and virtualization - which
    // are what a user usually checks here.
    if has("info") || has("system") || has("systeme") || has("machine") {
        let mem = installed_memory_mib();
        let virt = virtualization_status(lang);
        uefi::println!("  {}: {}", tx(lang, "Processeur", "Processor"), cpu_brand());
        uefi::println!("  {}: {mem} MiB", tx(lang, "Memoire", "Memory"));
        uefi::println!("  {}: {virt}", tx(lang, "Virtualisation", "Virtualization"));
        aw_mark!("AW_UEFI_AGENT_INFO mem={mem}");
        play(ag(hda::AGENT_VALUE_IS), speaker, pending);
        let summary = format!("{mem} {}, {virt}", tx(lang, "mega-octets", "megabytes"));
        spell_current(&summary, lang, speaker, pending);
        return;
    }

    // Open the firmware's own setup (for everything a loaded app cannot reach).
    if has("firmware") || has("setup") || has("bios") || has("config") {
        play(ag(hda::AGENT_OPENING_SETUP), speaker, pending);
        enter_firmware_setup();
        play(ag(hda::AGENT_SETUP_DENIED), speaker, pending);
        return;
    }

    // Restart now.
    if has("restart") || has("reboot") || has("redemarr") || has("reset") {
        play(ag(hda::AGENT_RESTARTING), speaker, pending);
        runtime::reset(runtime::ResetType::COLD, Status::SUCCESS, None);
    }

    // Shut down now.
    if has("shut") || has("eteind") || has("arret") || has("power") {
        play(ag(hda::AGENT_SHUTTING_DOWN), speaker, pending);
        runtime::reset(runtime::ResetType::SHUTDOWN, Status::SUCCESS, None);
    }

    // Nothing matched.
    play(ag(hda::AGENT_UNKNOWN), speaker, pending);
}

/// Resolve which boot entry a command refers to: first by a position number ("boot 2"),
/// then by a descriptive word the user typed that appears in a device's name ("boot usb",
/// "boot windows"). The command verbs are ignored so they cannot match a label word like
/// "Boot" in "Windows Boot Manager".
fn find_boot_target<'a>(cmd: &str, options: &'a [BootOption]) -> Option<&'a BootOption> {
    if let Some(position) = first_number(cmd) {
        if position >= 1 && position <= options.len() {
            return options.get(position - 1);
        }
    }
    let terms: Vec<&str> = cmd
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|word| word.len() >= 2)
        .filter(|word| {
            !matches!(
                *word,
                "boot"
                    | "default"
                    | "defaut"
                    | "demarrer"
                    | "demarre"
                    | "demarrage"
                    | "par"
                    | "def"
                    | "sur"
                    | "now"
                    | "maintenant"
            )
        })
        .collect();
    options
        .iter()
        .find(|opt| {
            let label = opt.label.to_ascii_lowercase();
            terms.iter().any(|term| label.contains(term))
        })
}

/// The first run of decimal digits in `s`, parsed as a 1-based position, or `None`.
fn first_number(s: &str) -> Option<usize> {
    let digits: String = s
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse().ok()
}

/// One level of the descent: which screen we are in and which item is focused.
#[derive(Clone, Copy)]
struct Frame {
    screen: usize,
    item: usize,
}

/// Present the accessible setup tree and act on the user's choices. Voiced on the
/// console and through markers and HDA clips, with audible cues; operated on the
/// firmware's own keyboard. With no key pressed, a countdown boots normally, so an
/// unattended boot - and the timed proof harness - always proceeds.
pub fn run(width: usize, height: usize, speaker: &mut Option<audio::Speaker>) {
    // French is the default language; the Language item on the Main tab switches it.
    let mut lang = Lang::Fr;
    set_current_lang(lang);
    let mut tree = build_tree(lang, width, height);

    // Open on the Boot tab with "Boot normally" focused: the safe default a user or the
    // countdown takes, and the first thing announced. The tab is located by index, so it
    // is the same whatever the language.
    let mut tab_index = tree.boot_tab;
    // `stack` always holds at least the current top-tab frame; deeper frames are
    // submenus. Its length minus one is the current depth.
    let mut stack: Vec<Frame> = alloc::vec![Frame {
        screen: tree.tabs[tab_index],
        item: 0
    }];

    // A keystroke captured while a clip was playing (barge-in). It is processed before a
    // fresh key is read, so acting on the setup always interrupts what it was saying.
    let mut pending: Option<Key> = None;

    aw_mark!("AW_UEFI_SR_READY");
    {
        let top = *stack.last().unwrap();
        render(&tree, tab_index, top.screen, top.item, stack.len() - 1);
    }
    sound::cue(CUE_READY_HZ, Duration::from_millis(90));
    // Name the setup, then teach the interaction model up front, so a blind user knows how
    // to drive it before anything else is said - and can press a key to skip straight in.
    play(
        clip(lang, hda::CLIP_FR_INTRO, hda::CLIP_SETUP_INTRO),
        speaker,
        &mut pending,
    );
    play(
        clip(lang, hda::CLIP_FR_INSTRUCTIONS, hda::CLIP_INSTRUCTIONS),
        speaker,
        &mut pending,
    );
    announce_screen(
        &tree,
        tab_index,
        tree.tabs[tab_index],
        0,
        speaker,
        &mut pending,
    );
    announce_item(
        &tree.screens[tree.tabs[tab_index]],
        0,
        speaker,
        &mut pending,
    );

    let mut interacted = false;
    let mut waited = Duration::ZERO;

    loop {
        let next_key = pending.take().or_else(read_key_raw);
        if let Some(key) = next_key {
            interacted = true;
            let depth = stack.len() - 1;
            match classify(key) {
                Nav::NextTab | Nav::PreviousTab if depth == 0 => {
                    tab_index = match classify(key) {
                        Nav::NextTab => (tab_index + 1) % tree.tabs.len(),
                        _ => (tab_index + tree.tabs.len() - 1) % tree.tabs.len(),
                    };
                    stack = alloc::vec![Frame {
                        screen: tree.tabs[tab_index],
                        item: 0
                    }];
                    sound::cue(CUE_TAB_HZ, Duration::from_millis(45));
                    render(&tree, tab_index, tree.tabs[tab_index], 0, 0);
                    announce_screen(
                        &tree,
                        tab_index,
                        tree.tabs[tab_index],
                        0,
                        speaker,
                        &mut pending,
                    );
                    announce_item(
                        &tree.screens[tree.tabs[tab_index]],
                        0,
                        speaker,
                        &mut pending,
                    );
                }
                Nav::NextTab | Nav::PreviousTab => {} // ignored inside a submenu
                Nav::NextItem | Nav::PreviousItem => {
                    let frame = stack.last_mut().unwrap();
                    let count = tree.screens[frame.screen].items.len().max(1);
                    frame.item = match classify(key) {
                        Nav::NextItem => (frame.item + 1) % count,
                        _ => (frame.item + count - 1) % count,
                    };
                    let (screen, item) = (frame.screen, frame.item);
                    sound::cue(CUE_MOVE_HZ, Duration::from_millis(35));
                    render(&tree, tab_index, screen, item, stack.len() - 1);
                    announce_item(&tree.screens[screen], item, speaker, &mut pending);
                }
                Nav::First | Nav::Last => {
                    let frame = stack.last_mut().unwrap();
                    let count = tree.screens[frame.screen].items.len().max(1);
                    frame.item = if matches!(classify(key), Nav::First) {
                        0
                    } else {
                        count - 1
                    };
                    let (screen, item) = (frame.screen, frame.item);
                    sound::cue(CUE_MOVE_HZ, Duration::from_millis(35));
                    render(&tree, tab_index, screen, item, stack.len() - 1);
                    announce_item(&tree.screens[screen], item, speaker, &mut pending);
                }
                Nav::Select => {
                    let frame = *stack.last().unwrap();
                    let action = tree.screens[frame.screen].items[frame.item].action;
                    match action {
                        Action::Info => {}
                        Action::SubMenu(child) => {
                            stack.push(Frame {
                                screen: child,
                                item: 0,
                            });
                            sound::cue(CUE_TAB_HZ, Duration::from_millis(45));
                            render(&tree, tab_index, child, 0, stack.len() - 1);
                            announce_screen(
                                &tree,
                                tab_index,
                                child,
                                stack.len() - 1,
                                speaker,
                                &mut pending,
                            );
                            announce_item(&tree.screens[child], 0, speaker, &mut pending);
                        }
                        Action::Back => {
                            back_out(&tree, tab_index, &mut stack, speaker, &mut pending);
                        }
                        Action::BootNormally => {
                            sound::cue(CUE_CONTINUE_HZ, Duration::from_millis(150));
                            aw_mark!("AW_UEFI_MENU_SELECT name=\"boot_normally\"");
                            aw_mark!("AW_UEFI_SR_CONTINUE reason=selected");
                            return;
                        }
                        // Booting a chosen device restarts the machine, so confirm first.
                        Action::BootNow(id) => {
                            if confirm(lang, speaker, &mut pending) {
                                boot_now(id);
                            } else {
                                announce_item(
                                    &tree.screens[frame.screen],
                                    frame.item,
                                    speaker,
                                    &mut pending,
                                );
                            }
                        }
                        // Persistent but reversible: apply at once, and say "Done" so the
                        // user knows it worked - not just a tone.
                        Action::MakeDefault(id) => {
                            if make_default(id) {
                                sound::cue(CUE_APPLIED_HZ, Duration::from_millis(120));
                                play(
                                    clip(lang, hda::CLIP_FR_CONFIRM_DONE, hda::CLIP_CONFIRM_DONE),
                                    speaker,
                                    &mut pending,
                                );
                            }
                        }
                        Action::MoveUp(id) => {
                            if move_in_boot_order(id, true) {
                                sound::cue(CUE_APPLIED_HZ, Duration::from_millis(120));
                                play(
                                    clip(lang, hda::CLIP_FR_CONFIRM_DONE, hda::CLIP_CONFIRM_DONE),
                                    speaker,
                                    &mut pending,
                                );
                            }
                        }
                        Action::MoveDown(id) => {
                            if move_in_boot_order(id, false) {
                                sound::cue(CUE_APPLIED_HZ, Duration::from_millis(120));
                                play(
                                    clip(lang, hda::CLIP_FR_CONFIRM_DONE, hda::CLIP_CONFIRM_DONE),
                                    speaker,
                                    &mut pending,
                                );
                            }
                        }
                        // Each of these reboots or powers off, so confirm first.
                        Action::EnterSetup => {
                            if confirm(lang, speaker, &mut pending) {
                                enter_firmware_setup();
                            } else {
                                announce_item(
                                    &tree.screens[frame.screen],
                                    frame.item,
                                    speaker,
                                    &mut pending,
                                );
                            }
                        }
                        Action::Reset => {
                            if confirm(lang, speaker, &mut pending) {
                                aw_mark!("AW_UEFI_MENU_SELECT name=\"reset\"");
                                runtime::reset(runtime::ResetType::COLD, Status::SUCCESS, None);
                            } else {
                                announce_item(
                                    &tree.screens[frame.screen],
                                    frame.item,
                                    speaker,
                                    &mut pending,
                                );
                            }
                        }
                        Action::Shutdown => {
                            if confirm(lang, speaker, &mut pending) {
                                aw_mark!("AW_UEFI_MENU_SELECT name=\"shutdown\"");
                                runtime::reset(runtime::ResetType::SHUTDOWN, Status::SUCCESS, None);
                            } else {
                                announce_item(
                                    &tree.screens[frame.screen],
                                    frame.item,
                                    speaker,
                                    &mut pending,
                                );
                            }
                        }
                        // Switch language and rebuild the whole tree in the new one, then
                        // land back on the same tab so the change is heard immediately.
                        Action::ToggleLang => {
                            lang = lang.toggled();
                            set_current_lang(lang);
                            aw_mark!(
                                "AW_UEFI_SETUP_LANG lang={}",
                                match lang {
                                    Lang::Fr => "fr",
                                    Lang::En => "en",
                                }
                            );
                            tree = build_tree(lang, width, height);
                            // Land on the Main tab (index 0), where the Language item is,
                            // so its first announcement is the new language.
                            tab_index = 0;
                            stack = alloc::vec![Frame {
                                screen: tree.tabs[tab_index],
                                item: 0
                            }];
                            sound::cue(CUE_TAB_HZ, Duration::from_millis(45));
                            render(&tree, tab_index, tree.tabs[tab_index], 0, 0);
                            announce_screen(
                                &tree,
                                tab_index,
                                tree.tabs[tab_index],
                                0,
                                speaker,
                                &mut pending,
                            );
                            announce_item(
                                &tree.screens[tree.tabs[tab_index]],
                                0,
                                speaker,
                                &mut pending,
                            );
                        }
                    }
                }
                Nav::Back => {
                    if stack.len() > 1 {
                        back_out(&tree, tab_index, &mut stack, speaker, &mut pending);
                    } else {
                        sound::cue(CUE_CONTINUE_HZ, Duration::from_millis(150));
                        aw_mark!("AW_UEFI_SR_CONTINUE reason=escape");
                        return;
                    }
                }
                Nav::Repeat => {
                    let frame = *stack.last().unwrap();
                    announce_item(
                        &tree.screens[frame.screen],
                        frame.item,
                        speaker,
                        &mut pending,
                    );
                }
                Nav::Help => {
                    let frame = *stack.last().unwrap();
                    announce_help(&tree.screens[frame.screen], frame.item);
                }
                Nav::Where => {
                    let frame = *stack.last().unwrap();
                    let depth = stack.len() - 1;
                    announce_screen(&tree, tab_index, frame.screen, depth, speaker, &mut pending);
                    announce_item(
                        &tree.screens[frame.screen],
                        frame.item,
                        speaker,
                        &mut pending,
                    );
                }
                Nav::Spell => {
                    let frame = *stack.last().unwrap();
                    spell_current(
                        &tree.screens[frame.screen].items[frame.item].text,
                        lang,
                        speaker,
                        &mut pending,
                    );
                }
                Nav::SayAll => {
                    let frame = *stack.last().unwrap();
                    let count = tree.screens[frame.screen].items.len();
                    for index in 0..count {
                        // Barge-in stops the read-through at once.
                        if pending.is_some() {
                            break;
                        }
                        announce_item(&tree.screens[frame.screen], index, speaker, &mut pending);
                    }
                }
                Nav::Command => {
                    run_agent(lang, speaker, &mut pending);
                    // Re-announce where we are, so the user is oriented after the agent.
                    let frame = *stack.last().unwrap();
                    announce_item(
                        &tree.screens[frame.screen],
                        frame.item,
                        speaker,
                        &mut pending,
                    );
                }
                Nav::VolumeUp => {
                    let level = audio::volume_up();
                    aw_mark!("AW_UEFI_VOLUME level={level} muted=false");
                    // A PC-speaker cue whose pitch rises with the level gives instant,
                    // always-audible feedback; re-announcing the item lets the user hear the
                    // new speech volume on the real channel.
                    sound::cue(300 + level * 2, Duration::from_millis(90));
                    let frame = *stack.last().unwrap();
                    announce_item(&tree.screens[frame.screen], frame.item, speaker, &mut pending);
                }
                Nav::VolumeDown => {
                    let level = audio::volume_down();
                    aw_mark!("AW_UEFI_VOLUME level={level} muted=false");
                    sound::cue(300 + level * 2, Duration::from_millis(90));
                    let frame = *stack.last().unwrap();
                    announce_item(&tree.screens[frame.screen], frame.item, speaker, &mut pending);
                }
                Nav::Mute => {
                    let muted = audio::toggle_mute();
                    aw_mark!("AW_UEFI_VOLUME muted={muted}");
                    // The cue is on the PC speaker, so it is heard even while speech is muted.
                    sound::cue(
                        if muted { 240 } else { 660 },
                        Duration::from_millis(120),
                    );
                    if !muted {
                        let frame = *stack.last().unwrap();
                        announce_item(
                            &tree.screens[frame.screen],
                            frame.item,
                            speaker,
                            &mut pending,
                        );
                    }
                }
                Nav::Phonetic => {
                    let on = hda::toggle_phonetic();
                    aw_mark!("AW_UEFI_PHONETIC on={on}");
                    sound::cue(if on { 660 } else { 440 }, Duration::from_millis(90));
                    // Spell the letter A at once, so the user hears the new mode: the NATO
                    // word "Alpha" when on, the plain letter when off.
                    if let Some(clip) = hda::spell_clip('a', matches!(lang, Lang::Fr)) {
                        play(clip, speaker, &mut pending);
                    }
                }
                Nav::Ignore => {}
            }
            continue;
        }

        // No key waiting. An unattended boot counts down and then boots normally; once
        // someone has interacted, the countdown is abandoned and we wait.
        if !interacted {
            if waited >= REVIEW_WINDOW {
                sound::cue(CUE_CONTINUE_HZ, Duration::from_millis(150));
                aw_mark!("AW_UEFI_SR_CONTINUE reason=timeout");
                return;
            }
            waited += POLL_INTERVAL;
        }
        boot::stall(POLL_INTERVAL);
    }
}

/// Pop one submenu level and re-announce the screen and focused item we return to.
fn back_out(
    tree: &Tree,
    tab_index: usize,
    stack: &mut Vec<Frame>,
    speaker: &mut Option<audio::Speaker>,
    pending: &mut Option<Key>,
) {
    stack.pop();
    let frame = *stack.last().unwrap();
    let depth = stack.len() - 1;
    sound::cue(CUE_BACK_HZ, Duration::from_millis(45));
    render(tree, tab_index, frame.screen, frame.item, depth);
    announce_screen(tree, tab_index, frame.screen, depth, speaker, pending);
    announce_item(&tree.screens[frame.screen], frame.item, speaker, pending);
}
