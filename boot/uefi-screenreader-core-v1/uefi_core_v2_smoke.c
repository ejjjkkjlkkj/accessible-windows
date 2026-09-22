#include "legacy_session_adapter.h"

typedef unsigned char u8;
typedef unsigned short u16;
typedef unsigned long long u64;

static SrScreenReaderSession g_session;

static const SrHiiRecord g_records[] = {
    {10u, SR_HII_OP_ONE_OF, 0u, "Boot mode", "UEFI", "Use Left or Right"},
    {20u, SR_HII_OP_CHECKBOX, SR_HII_FLAG_CHECKED, "Secure Boot", "Enabled", "Press Enter to toggle"},
    {30u, SR_HII_OP_REF, 0u, "Advanced", "", "Press Enter to open"},
    {40u, SR_HII_OP_DATE, 0u, "System date", "2026-09-22", "Date"},
    {50u, SR_HII_OP_TIME, 0u, "System time", "17:45", "Time"},
    {60u, SR_HII_OP_STRING, 0u, "Asset tag", "QEV", "Edit text"},
    {70u, SR_HII_OP_ORDERED_LIST, 0u, "Boot order", "NVMe", "Ordered list"}
};

static inline void outb(u16 port, u8 value) {
    __asm__ volatile("outb %0, %1" :: "a"(value), "d"(port));
}

static inline u8 inb(u16 port) {
    u8 value;
    __asm__ volatile("inb %1, %0" : "=a"(value) : "d"(port));
    return value;
}

static void serial_init(void) {
    outb(0x3f9u, 0x00u);
    outb(0x3fbu, 0x80u);
    outb(0x3f8u, 0x01u);
    outb(0x3f9u, 0x00u);
    outb(0x3fbu, 0x03u);
    outb(0x3fau, 0xc7u);
    outb(0x3fcu, 0x0bu);
}

static void serial_putc(char ch) {
    u32_guard:
    {
        unsigned int guard = 0u;
        while ((inb(0x3fdu) & 0x20u) == 0u && guard < 1000000u) ++guard;
    }
    outb(0x3f8u, (u8)ch);
}

static void serial_puts(const char *text) {
    if (!text) return;
    while (*text) serial_putc(*text++);
}

static void marker(const char *text) {
    serial_puts(text);
    serial_puts("\r\n");
}

static int check(int condition, const char *pass_marker, const char *fail_marker) {
    if (condition) {
        marker(pass_marker);
        return 1;
    }
    marker(fail_marker);
    marker("STATUS=BLOCKED");
    return 0;
}

u64 efi_main(void *image_handle, void *system_table) {
    int exit_requested = 0;
    SrFirmwareKey key;
    const SrItem *current;

    (void)image_handle;
    (void)system_table;
    serial_init();
    marker("UEFI_CORE_V2_BOOT=PASS");

    sr_session_init(&g_session, 5u);
    if (!check(
        sr_session_apply_hii(&g_session, g_records, sizeof(g_records) / sizeof(g_records[0]), 0),
        "UEFI_CORE_V2_HII_BUILD=PASS",
        "UEFI_CORE_V2_HII_BUILD=FAIL"
    )) return 1u;

    current = sr_session_current(&g_session);
    if (!check(
        current && current->id == 10u,
        "UEFI_CORE_V2_INITIAL_FOCUS=PASS",
        "UEFI_CORE_V2_INITIAL_FOCUS=FAIL"
    )) return 1u;

    key.scan_code = 0x0002u;
    key.unicode_char = 0u;
    if (!check(
        sr_legacy_session_handle_key(&g_session, key, &exit_requested) &&
        sr_session_current(&g_session) &&
        sr_session_current(&g_session)->id == 20u,
        "UEFI_CORE_V2_NAVIGATION=PASS",
        "UEFI_CORE_V2_NAVIGATION=FAIL"
    )) return 1u;

    key.scan_code = 0x0012u;
    if (!check(
        sr_legacy_session_handle_key(&g_session, key, &exit_requested) &&
        g_session.nav.chooser_open,
        "UEFI_CORE_V2_CHOOSER_OPEN=PASS",
        "UEFI_CORE_V2_CHOOSER_OPEN=FAIL"
    )) return 1u;

    key.scan_code = 0u;
    key.unicode_char = (u16)'a';
    if (!check(
        sr_legacy_session_handle_key(&g_session, key, &exit_requested) &&
        g_session.nav.chooser_match_count == 1u &&
        sr_chooser_current(&g_session.nav) &&
        sr_chooser_current(&g_session.nav)->id == 30u,
        "UEFI_CORE_V2_CHOOSER_FILTER=PASS",
        "UEFI_CORE_V2_CHOOSER_FILTER=FAIL"
    )) return 1u;

    key.unicode_char = 0x000du;
    if (!check(
        sr_legacy_session_handle_key(&g_session, key, &exit_requested) &&
        !g_session.nav.chooser_open &&
        sr_session_current(&g_session) &&
        sr_session_current(&g_session)->id == 30u,
        "UEFI_CORE_V2_CHOOSER_SELECT=PASS",
        "UEFI_CORE_V2_CHOOSER_SELECT=FAIL"
    )) return 1u;

    key.unicode_char = 0x001bu;
    if (!check(
        sr_legacy_session_handle_key(&g_session, key, &exit_requested) &&
        exit_requested,
        "UEFI_CORE_V2_EXIT=PASS",
        "UEFI_CORE_V2_EXIT=FAIL"
    )) return 1u;

    marker("UEFI_CORE_V2_RUNTIME=PASS");
    marker("STATUS=PASS");
    return 0u;
}
