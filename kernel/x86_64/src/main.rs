#![no_main]
#![no_std]

use aw_kernel_core::{HANDOFF_FLAG_FRAMEBUFFER_PRESENT, HandoffPixelFormat, KernelHandoff};
use aw_x86_platform::{CpuFeatures, CpuIdentity, CpuSignature, CpuVendor, CpuidRegisters};
use core::arch::asm;
use core::arch::x86_64::__cpuid_count;
use core::panic::PanicInfo;

const DEBUG_PORT: u16 = 0x00e9;

#[inline(always)]
unsafe fn outb(port: u16, value: u8) {
    // SAFETY: The caller ensures this is used only for the QEMU/Bochs debug
    // output port during the earliest x86-64 kernel stage.
    unsafe {
        asm!(
            "out dx, al",
            in("dx") port,
            in("al") value,
            options(nomem, nostack, preserves_flags)
        );
    }
}

fn debug_write(message: &str) {
    for byte in message.bytes() {
        // SAFETY: DEBUG_PORT is the conventional byte-wide debug console port.
        unsafe { outb(DEBUG_PORT, byte) };
    }
}

#[inline(always)]
fn halt_forever() -> ! {
    loop {
        // SAFETY: Interrupts are disabled before entering the kernel main path,
        // so HLT cannot dispatch into an uninitialized interrupt descriptor table.
        unsafe { asm!("hlt", options(nomem, nostack, preserves_flags)) };
    }
}

#[inline(always)]
fn cpuid(leaf: u32, subleaf: u32) -> CpuidRegisters {
    // SAFETY: CPUID is available in x86-64 long mode. The intrinsic only reads
    // architectural CPU identification registers and does not dereference memory.
    let registers = unsafe { __cpuid_count(leaf, subleaf) };
    CpuidRegisters {
        eax: registers.eax,
        ebx: registers.ebx,
        ecx: registers.ecx,
        edx: registers.edx,
    }
}

fn detect_cpu() -> CpuIdentity {
    let leaf0 = cpuid(0, 0);
    let max_basic_leaf = leaf0.eax;
    let extended_leaf0 = cpuid(0x8000_0000, 0);
    let max_extended_leaf = extended_leaf0.eax;

    let leaf1 = if max_basic_leaf >= 1 {
        cpuid(1, 0)
    } else {
        CpuidRegisters::ZERO
    };
    let leaf7 = (max_basic_leaf >= 7).then(|| cpuid(7, 0));
    let extended_leaf1 = (max_extended_leaf >= 0x8000_0001).then(|| cpuid(0x8000_0001, 0));
    let extended_leaf7 = (max_extended_leaf >= 0x8000_0007).then(|| cpuid(0x8000_0007, 0));

    CpuIdentity {
        vendor: CpuVendor::from_leaf0(leaf0),
        signature: CpuSignature::from_leaf1_eax(leaf1.eax),
        max_basic_leaf,
        max_extended_leaf,
        features: CpuFeatures::from_leaves(leaf1, leaf7, extended_leaf1, extended_leaf7),
    }
}

fn validate_cpu_baseline() {
    let cpu = detect_cpu();

    debug_write("AW_CPU_VENDOR_OK vendor=");
    debug_write(cpu.vendor.canonical_name());
    debug_write("\n");

    if cpu.features.meets_boot_baseline() {
        debug_write("AW_CPU_BASELINE_OK apic=1 sse2=1 long_mode=1\n");
    } else {
        debug_write("AW_CPU_BASELINE_FAIL\n");
        halt_forever();
    }

    if cpu.is_supported_vendor() {
        debug_write("AW_CPU_VENDOR_SUPPORTED\n");
    } else {
        // Unknown x86-64 vendors are allowed to continue on the generic path
        // when the required architectural features are present.
        debug_write("AW_CPU_VENDOR_GENERIC_FALLBACK\n");
    }

    if cpu.features.x2apic {
        debug_write("AW_CPU_X2APIC_AVAILABLE\n");
    } else {
        debug_write("AW_CPU_X2APIC_FALLBACK_APIC\n");
    }

    if cpu.features.invariant_tsc {
        debug_write("AW_CPU_INVARIANT_TSC_AVAILABLE\n");
    } else {
        debug_write("AW_CPU_TIMER_FALLBACK_REQUIRED\n");
    }
}

fn paint_boot_marker(handoff: &KernelHandoff) -> bool {
    if handoff.flags & HANDOFF_FLAG_FRAMEBUFFER_PRESENT == 0 {
        return false;
    }

    let framebuffer = handoff.framebuffer;
    if !matches!(
        framebuffer.pixel_format,
        HandoffPixelFormat::Rgb | HandoffPixelFormat::Bgr
    ) {
        return false;
    }

    let required_bytes = u64::from(framebuffer.stride_pixels)
        .saturating_mul(u64::from(framebuffer.height))
        .saturating_mul(4);
    if required_bytes > framebuffer.byte_len || framebuffer.physical_address == 0 {
        return false;
    }

    let rows = framebuffer.height.min(32);
    let width = framebuffer.width;
    let stride = u64::from(framebuffer.stride_pixels);
    let base = framebuffer.physical_address as *mut u32;

    for row in 0..rows {
        for column in 0..width {
            let pixel_index = u64::from(row)
                .saturating_mul(stride)
                .saturating_add(u64::from(column));
            if pixel_index >= framebuffer.byte_len / 4 {
                return false;
            }

            // Magenta is symmetric under RGB/BGR channel swapping and provides
            // a visible physical-hardware marker without needing a GPU driver.
            // SAFETY: The handoff was validated, bounds are checked above, and
            // this range is the UEFI-provided linear framebuffer.
            unsafe {
                core::ptr::write_volatile(base.add(pixel_index as usize), 0x00ff_00ff);
            }
        }
    }

    true
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text._start")]
pub extern "sysv64" fn _start(handoff_ptr: *const KernelHandoff) -> ! {
    // SAFETY: This is the first native kernel instruction path. No IDT exists
    // yet, so mask interrupts before touching any other CPU state.
    unsafe { asm!("cli", options(nomem, nostack, preserves_flags)) };

    if handoff_ptr.is_null() {
        debug_write("AW_NATIVE_KERNEL_FAIL reason=null_handoff\n");
        halt_forever();
    }

    // SAFETY: The UEFI loader constructs a KernelHandoff on its live stack and
    // transfers control without returning. The pointer remains valid here.
    let handoff = unsafe { &*handoff_ptr };
    if let Err(_error) = handoff.validate() {
        debug_write("AW_NATIVE_KERNEL_FAIL reason=invalid_handoff\n");
        halt_forever();
    }

    debug_write("AW_NATIVE_KERNEL_ENTRY_OK\n");
    validate_cpu_baseline();

    if paint_boot_marker(handoff) {
        debug_write("AW_NATIVE_FRAMEBUFFER_WRITE_OK\n");
    } else {
        debug_write("AW_NATIVE_FRAMEBUFFER_WRITE_SKIP\n");
    }

    debug_write("AW_NATIVE_KERNEL_IDLE\n");
    halt_forever();
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    debug_write("AW_NATIVE_KERNEL_PANIC\n");
    halt_forever();
}
