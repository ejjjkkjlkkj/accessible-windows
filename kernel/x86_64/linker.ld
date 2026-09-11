ENTRY(_start)

KERNEL_LOAD_ADDRESS = 0x01000000;

SECTIONS
{
    . = KERNEL_LOAD_ADDRESS;

    .text : ALIGN(0x1000)
    {
        KEEP(*(.text._start))
        *(.text .text.*)
    }

    .rodata : ALIGN(0x1000)
    {
        *(.rodata .rodata.*)
    }

    .data : ALIGN(0x1000)
    {
        *(.data .data.*)
        *(.data.rel.ro .data.rel.ro.*)
    }

    .bss (NOLOAD) : ALIGN(0x1000)
    {
        *(.bss .bss.*)
        *(COMMON)
    }

    /DISCARD/ :
    {
        *(.eh_frame .eh_frame.*)
        *(.comment)
        *(.note .note.*)
    }
}
