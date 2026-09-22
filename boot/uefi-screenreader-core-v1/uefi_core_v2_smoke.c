#include "legacy_session_adapter.h"
#include "live_hii_adapter.h"

typedef unsigned char u8;
typedef unsigned short u16;
typedef unsigned long long u64;

static SrScreenReaderSession g_session;
static SrLiveHiiSnapshot g_live;

/*
 * One complete EFI_HII_PACKAGE_LIST containing a Forms package and an End
 * package. The Forms payload contains seven question controls with stable
 * QuestionIds. This exercises the same package-list boundary that the real
 * HII Database ExportPackageLists protocol returns.
 */
static const u8 g_package_list[] = {
    0x7bu, 0x59u, 0x10u, 0x4au, 0xc0u, 0x0du, 0x41u, 0x58u,
    0x87u, 0xffu, 0xf0u, 0x4du, 0x63u, 0x96u, 0xa9u, 0x15u,
    0x86u, 0x00u, 0x00u, 0x00u,

    0x6eu, 0x00u, 0x00u, 0x02u,

    0x01u, 0x86u, 0x34u, 0x12u, 0x01u, 0x00u,

    0x05u, 0x0eu, 0x10u, 0x00u, 0x11u, 0x00u, 0x00u, 0x01u,
    0x01u, 0x00u, 0x04u, 0x00u, 0x00u, 0x00u,

    0x06u, 0x0eu, 0x20u, 0x00u, 0x21u, 0x00u, 0x01u, 0x01u,
    0x01u, 0x00u, 0x08u, 0x00u, 0x01u, 0x00u,

    0x0fu, 0x0eu, 0x30u, 0x00u, 0x31u, 0x00u, 0x02u, 0x01u,
    0x01u, 0x00u, 0x0cu, 0x00u, 0x00u, 0x00u,

    0x1au, 0x0eu, 0x40u, 0x00u, 0x41u, 0x00u, 0x03u, 0x01u,
    0x01u, 0x00u, 0x10u, 0x00u, 0x00u, 0x00u,

    0x1bu, 0x0eu, 0x50u, 0x00u, 0x51u, 0x00u, 0x04u, 0x01u,
    0x01u, 0x00u, 0x14u, 0x00u, 0x00u, 0x00u,

    0x1cu, 0x0eu, 0x60u, 0x00u, 0x61u, 0x00u, 0x05u, 0x01u,
    0x01u, 0x00u, 0x18u, 0x00u, 0x00u, 0x00u,

    0x23u, 0x0eu, 0x70u, 0x00u, 0x71u, 0x00u, 0x06u, 0x01u,
    0x01u, 0x00u, 0x1cu, 0x00u, 0x00u, 0x00u,

    0x29u, 0x02u,

    0x04u, 0x00u, 0x00u, 0xdfu
};

static int copy_text(char *out, size_t cap, const char *text) {
    size_t i = 0u;
    if (!out || cap == 0u || !text) return 0;
    while (text[i]) {
        if (i + 1u >= cap) return 0;
        out[i] = text[i];
        ++i;
    }
    out[i] = '\0';
    return 1;
}

static int resolve_string(void *context, uint16_t token, char *out, size_t cap) {
    (void)context;
    switch (token) {
        case 0x10u: return copy_text(out, cap, "Boot mode");
        case 0x11u: return copy_text(out, cap, "Use Left or Right");
        case 0x20u: return copy_text(out, cap, "Secure Boot");
        case 0x21u: return copy_text(out, cap, "Press Enter to toggle");
        case 0x30u: return copy_text(out, cap, "Advanced");
        case 0x31u: return copy_text(out, cap, "Press Enter to open");
        case 0x40u: return copy_text(out, cap, "System date");
        case 0x41u: return copy_text(out, cap, "Date");
        case 0x50u: return copy_text(out, cap, "System time");
        case 0x51u: return copy_text(out, cap, "Time");
        case 0x60u: return copy_text(out, cap, "Asset tag");
        case 0x61u: return copy_text(out, cap, "Edit text");
        case 0x70u: return copy_text(out, cap, "Boot order");
        case 0x71u: return copy_text(out, cap, "Ordered list");
        default: return 0;
    }
}

static int resolve_value(
    void *context,
    const SrIfrStatementMeta *meta,
    char *out,
    size_t cap,
    uint32_t *flags_io
) {
    (void)context;
    if (!meta || !out || !flags_io) return -1;

    switch (meta->question_id) {
        case 0x0100u:
            return copy_text(out, cap, "UEFI") ? 1 : -1;
        case 0x0101u:
            *flags_io |= SR_HII_FLAG_CHECKED;
            return copy_text(out, cap, "Enabled") ? 1 : -1;
        case 0x0102u:
            return 0;
        case 0x0103u:
            return copy_text(out, cap, "2026-09-22") ? 1 : -1;
        case 0x0104u:
            return copy_text(out, cap, "17:45") ? 1 : -1;
        case 0x0105u:
            return copy_text(out, cap, "QEV") ? 1 : -1;
        case 0x0106u:
            return copy_text(out, cap, "NVMe") ? 1 : -1;
        default:
            return 0;
    }
}

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
    unsigned int guard = 0u;
    while ((inb(0x3fdu) & 0x20u) == 0u && guard < 1000000u) ++guard;
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

    if (!check(
        sr_live_hii_build_package_list(
            &g_live,
            g_package_list,
            sizeof(g_package_list),
            resolve_string,
            resolve_value,
            NULL
        ) && g_live.count == 7u,
        "UEFI_CORE_V2_LIVE_PACKAGE=PASS",
        "UEFI_CORE_V2_LIVE_PACKAGE=FAIL"
    )) return 1u;

    if (!check(
        g_live.meta[0].stable_id != 0u &&
        g_live.meta[0].question_id == 0x0100u &&
        g_live.meta[1].question_id == 0x0101u &&
        g_live.meta[2].question_id == 0x0102u,
        "UEFI_CORE_V2_STABLE_QUESTION_ID=PASS",
        "UEFI_CORE_V2_STABLE_QUESTION_ID=FAIL"
    )) return 1u;

    sr_session_init(&g_session, 5u);
    if (!check(
        sr_session_apply_hii(&g_session, g_live.records, g_live.count, 0),
        "UEFI_CORE_V2_HII_BUILD=PASS",
        "UEFI_CORE_V2_HII_BUILD=FAIL"
    )) return 1u;

    current = sr_session_current(&g_session);
    if (!check(
        current && current->id == g_live.meta[0].stable_id,
        "UEFI_CORE_V2_INITIAL_FOCUS=PASS",
        "UEFI_CORE_V2_INITIAL_FOCUS=FAIL"
    )) return 1u;

    key.scan_code = 0x0002u;
    key.unicode_char = 0u;
    if (!check(
        sr_legacy_session_handle_key(&g_session, key, &exit_requested) &&
        sr_session_current(&g_session) &&
        sr_session_current(&g_session)->id == g_live.meta[1].stable_id,
        "UEFI_CORE_V2_NAVIGATION=PASS",
        "UEFI_CORE_V2_NAVIGATION=FAIL"
    )) return 1u;

    current = sr_session_current(&g_session);
    if (!check(
        current &&
        (current->state & SR_STATE_READ_ONLY) != 0u &&
        (current->state & SR_STATE_CHECKED) != 0u,
        "UEFI_CORE_V2_READ_ONLY_STATE=PASS",
        "UEFI_CORE_V2_READ_ONLY_STATE=FAIL"
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
    if (!sr_legacy_session_handle_key(&g_session, key, &exit_requested)) return 1u;
    key.unicode_char = (u16)'d';
    if (!sr_legacy_session_handle_key(&g_session, key, &exit_requested)) return 1u;
    key.unicode_char = (u16)'v';
    if (!check(
        sr_legacy_session_handle_key(&g_session, key, &exit_requested) &&
        g_session.nav.chooser_match_count == 1u &&
        sr_chooser_current(&g_session.nav) &&
        sr_chooser_current(&g_session.nav)->id == g_live.meta[2].stable_id,
        "UEFI_CORE_V2_CHOOSER_FILTER=PASS",
        "UEFI_CORE_V2_CHOOSER_FILTER=FAIL"
    )) return 1u;

    key.unicode_char = 0x000du;
    if (!check(
        sr_legacy_session_handle_key(&g_session, key, &exit_requested) &&
        !g_session.nav.chooser_open &&
        sr_session_current(&g_session) &&
        sr_session_current(&g_session)->id == g_live.meta[2].stable_id,
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
