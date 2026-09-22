#include "ifr_collector.h"
#include "legacy_session_adapter.h"

typedef unsigned char u8;
typedef unsigned short u16;
typedef unsigned long long u64;

#define SMOKE_ITEM_COUNT 7u

static SrScreenReaderSession g_session;
static SrIfrStatementMeta g_meta[SMOKE_ITEM_COUNT];
static SrLegacyIfrRecord g_legacy[SMOKE_ITEM_COUNT];
static SrHiiRecord g_records[SMOKE_ITEM_COUNT];

static const u8 g_package_guid[SR_IFR_GUID_BYTES] = {
    0x7bu, 0x59u, 0x10u, 0x4au, 0xc0u, 0x0du, 0x41u, 0x58u,
    0x87u, 0xffu, 0xf0u, 0x4du, 0x63u, 0x96u, 0xa9u, 0x15u
};

/*
 * Synthetic but structurally valid IFR stream for exercising the parser in
 * a real PE/COFF UEFI application. Each question carries a stable QuestionId.
 */
static const u8 g_ifr[] = {
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

    0x29u, 0x02u
};

static const char *const g_labels[SMOKE_ITEM_COUNT] = {
    "Boot mode",
    "Secure Boot",
    "Advanced",
    "System date",
    "System time",
    "Asset tag",
    "Boot order"
};

static const char *const g_values[SMOKE_ITEM_COUNT] = {
    "UEFI",
    "Enabled",
    "",
    "2026-09-22",
    "17:45",
    "QEV",
    "NVMe"
};

static const char *const g_hints[SMOKE_ITEM_COUNT] = {
    "Use Left or Right",
    "Press Enter to toggle",
    "Press Enter to open",
    "Date",
    "Time",
    "Edit text",
    "Ordered list"
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

static int build_hii_records(void) {
    size_t meta_count = 0u;
    size_t record_count = 0u;
    size_t i;

    if (!sr_ifr_collect_statements(
        g_package_guid,
        g_ifr,
        sizeof(g_ifr),
        g_meta,
        SMOKE_ITEM_COUNT,
        &meta_count
    )) return 0;
    if (meta_count != SMOKE_ITEM_COUNT) return 0;

    for (i = 0u; i < SMOKE_ITEM_COUNT; ++i) {
        sr_ifr_bind_legacy_record(
            &g_meta[i],
            g_labels[i],
            g_values[i],
            g_hints[i],
            &g_legacy[i]
        );
    }
    g_legacy[1].flags |= SR_HII_FLAG_CHECKED;

    if (!sr_legacy_ifr_convert(
        g_legacy,
        SMOKE_ITEM_COUNT,
        g_records,
        SMOKE_ITEM_COUNT,
        &record_count
    )) return 0;

    return record_count == SMOKE_ITEM_COUNT;
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
        build_hii_records(),
        "UEFI_CORE_V2_IFR_COLLECTOR=PASS",
        "UEFI_CORE_V2_IFR_COLLECTOR=FAIL"
    )) return 1u;

    if (!check(
        g_meta[0].stable_id != 0u &&
        g_meta[0].question_id == 0x0100u &&
        g_meta[1].question_id == 0x0101u &&
        g_meta[2].question_id == 0x0102u,
        "UEFI_CORE_V2_STABLE_QUESTION_ID=PASS",
        "UEFI_CORE_V2_STABLE_QUESTION_ID=FAIL"
    )) return 1u;

    sr_session_init(&g_session, 5u);
    if (!check(
        sr_session_apply_hii(&g_session, g_records, SMOKE_ITEM_COUNT, 0),
        "UEFI_CORE_V2_HII_BUILD=PASS",
        "UEFI_CORE_V2_HII_BUILD=FAIL"
    )) return 1u;

    current = sr_session_current(&g_session);
    if (!check(
        current && current->id == g_meta[0].stable_id,
        "UEFI_CORE_V2_INITIAL_FOCUS=PASS",
        "UEFI_CORE_V2_INITIAL_FOCUS=FAIL"
    )) return 1u;

    key.scan_code = 0x0002u;
    key.unicode_char = 0u;
    if (!check(
        sr_legacy_session_handle_key(&g_session, key, &exit_requested) &&
        sr_session_current(&g_session) &&
        sr_session_current(&g_session)->id == g_meta[1].stable_id,
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
        sr_chooser_current(&g_session.nav)->id == g_meta[2].stable_id,
        "UEFI_CORE_V2_CHOOSER_FILTER=PASS",
        "UEFI_CORE_V2_CHOOSER_FILTER=FAIL"
    )) return 1u;

    key.unicode_char = 0x000du;
    if (!check(
        sr_legacy_session_handle_key(&g_session, key, &exit_requested) &&
        !g_session.nav.chooser_open &&
        sr_session_current(&g_session) &&
        sr_session_current(&g_session)->id == g_meta[2].stable_id,
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
