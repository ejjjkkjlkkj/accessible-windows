#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Page {
    Root,
    Main,
    SystemInformation,
    Advanced,
    CpuConfiguration,
    Boot,
    BootPriorities,
    BootOption(usize),
    Security,
    SecureBoot,
    SaveExit,
    ConfirmRestart,
    ConfirmShutdown,
}

struct BootOption {
    number: u16,
    description: String,
}

fn global_attrs() -> VariableAttributes {
    VariableAttributes::NON_VOLATILE
        | VariableAttributes::BOOTSERVICE_ACCESS
        | VariableAttributes::RUNTIME_ACCESS
}

fn read_variable(name: &uefi::CStr16) -> Option<alloc::boxed::Box<[u8]>> {
    runtime::get_variable_boxed(name, &VariableVendor::GLOBAL_VARIABLE)
        .ok()
        .map(|(data, _)| data)
}

fn read_u16(name: &uefi::CStr16) -> Option<u16> {
    let data = read_variable(name)?;
    (data.len() >= 2).then(|| u16::from_le_bytes([data[0], data[1]]))
}

fn read_flag(name: &uefi::CStr16) -> Option<bool> {
    read_variable(name).and_then(|data| data.first().map(|value| *value != 0))
}

fn boot_variable_name(number: u16) -> Option<CString16> {
    let name = format!("Boot{number:04X}");
    CString16::try_from(name.as_str()).ok()
}

fn parse_boot_description(data: &[u8]) -> Option<String> {
    if data.len() < 8 {
        return None;
    }
    let mut units = Vec::new();
    for pair in data[6..].chunks_exact(2) {
        let unit = u16::from_le_bytes([pair[0], pair[1]]);
        if unit == 0 {
            break;
        }
        units.push(unit);
    }
    if units.is_empty() {
        None
    } else {
        Some(String::from_utf16_lossy(&units))
    }
}

fn boot_options() -> Vec<BootOption> {
    let Some(order) = read_variable(cstr16!("BootOrder")) else {
        return Vec::new();
    };

    let mut options = Vec::new();
    for pair in order.chunks_exact(2).take(MAX_BOOT_OPTIONS) {
        let number = u16::from_le_bytes([pair[0], pair[1]]);
        let Some(name) = boot_variable_name(number) else {
            continue;
        };
        let description = read_variable(&name)
            .and_then(|data| parse_boot_description(&data))
            .unwrap_or_else(|| format!("Boot {number:04X}"));
        options.push(BootOption {
            number,
            description,
        });
    }
    options
}

fn cpu_brand() -> String {
    #[cfg(target_arch = "x86_64")]
    {
        // SAFETY: CPUID is architected and side-effect free in x86_64 UEFI.
        let max = unsafe { core::arch::x86_64::__cpuid(0x8000_0000) }.eax;
        if max >= 0x8000_0004 {
            let mut bytes = [0_u8; 48];
            for (leaf_index, leaf) in (0x8000_0002..=0x8000_0004).enumerate() {
                // SAFETY: these leaves are advertised by the extended maximum.
                let result = unsafe { core::arch::x86_64::__cpuid(leaf) };
                for (register_index, register) in
                    [result.eax, result.ebx, result.ecx, result.edx].iter().enumerate()
                {
                    let start = leaf_index * 16 + register_index * 4;
                    bytes[start..start + 4].copy_from_slice(&register.to_le_bytes());
                }
            }
            if let Ok(text) = core::str::from_utf8(&bytes) {
                let text = text.trim_matches('\0').trim();
                if !text.is_empty() {
                    return text.to_string();
                }
            }
        }
    }
    "Unknown CPU".to_string()
}

fn virtualization_capability() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        // AMD SVM is CPUID 0x80000001 ECX bit 2. Intel VMX is leaf 1 ECX bit 5.
        // This reports hardware capability, not a writable OEM firmware setting.
        // SAFETY: CPUID is architected and side-effect free.
        let basic = unsafe { core::arch::x86_64::__cpuid(1) };
        let ext_max = unsafe { core::arch::x86_64::__cpuid(0x8000_0000) }.eax;
        let svm = if ext_max >= 0x8000_0001 {
            unsafe { core::arch::x86_64::__cpuid(0x8000_0001) }.ecx & (1 << 2) != 0
        } else {
            false
        };
        svm || (basic.ecx & (1 << 5) != 0)
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        false
    }
}

fn set_boot_next(number: u16) -> bool {
    runtime::set_variable(
        cstr16!("BootNext"),
        &VariableVendor::GLOBAL_VARIABLE,
        global_attrs(),
        &number.to_le_bytes(),
    )
    .is_ok()
}

fn set_boot_default(number: u16) -> bool {
    let Some(data) = read_variable(cstr16!("BootOrder")) else {
        return false;
    };
    let mut order: Vec<u16> = data
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    order.retain(|entry| *entry != number);
    order.insert(0, number);

    let mut encoded = Vec::with_capacity(order.len() * 2);
    for entry in order {
        encoded.extend_from_slice(&entry.to_le_bytes());
    }
    runtime::set_variable(
        cstr16!("BootOrder"),
        &VariableVendor::GLOBAL_VARIABLE,
        global_attrs(),
        &encoded,
    )
    .is_ok()
}

fn page_title(page: Page) -> (&'static str, Clip) {
    match page {
        Page::Root => ("Accessible Windows firmware setup", Clip::Setup),
        Page::Main => ("Main", Clip::Main),
        Page::SystemInformation => ("System information", Clip::SystemInformation),
        Page::Advanced => ("Advanced", Clip::Advanced),
        Page::CpuConfiguration => ("CPU configuration", Clip::CpuConfiguration),
        Page::Boot => ("Boot", Clip::Boot),
        Page::BootPriorities => ("Boot option priorities", Clip::BootOptionPriorities),
        Page::BootOption(_) => ("Boot option", Clip::BootOption),
        Page::Security => ("Security", Clip::Security),
        Page::SecureBoot => ("Secure Boot", Clip::SecureBoot),
        Page::SaveExit => ("Save and Exit", Clip::SaveExit),
        Page::ConfirmRestart => ("Confirm restart", Clip::ConfirmRestart),
        Page::ConfirmShutdown => ("Confirm shut down", Clip::ConfirmShutdown),
    }
}

fn parent(page: Page) -> Page {
    match page {
        Page::Root => Page::Root,
        Page::Main | Page::Advanced | Page::Boot | Page::Security | Page::SaveExit => Page::Root,
        Page::SystemInformation => Page::Main,
        Page::CpuConfiguration => Page::Advanced,
        Page::BootPriorities => Page::Boot,
        Page::BootOption(_) => Page::BootPriorities,
        Page::SecureBoot => Page::Security,
        Page::ConfirmRestart | Page::ConfirmShutdown => Page::SaveExit,
    }
}
