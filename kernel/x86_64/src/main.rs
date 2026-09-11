#![no_main]
#![no_std]

use aw_kernel_core::{HANDOFF_FLAG_FRAMEBUFFER_PRESENT, HandoffPixelFormat, KernelHandoff};
use core::arch::asm;
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
