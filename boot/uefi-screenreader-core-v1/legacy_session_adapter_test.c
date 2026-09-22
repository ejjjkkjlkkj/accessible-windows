#include "legacy_session_adapter.h"

#include <assert.h>
#include <stdio.h>
#include <string.h>

static const SrHiiRecord records[] = {
    {10u, SR_HII_OP_ONE_OF, 0u, "Boot mode", "UEFI", "Use Left or Right"},
    {20u, SR_HII_OP_CHECKBOX, SR_HII_FLAG_CHECKED, "Secure Boot", "Enabled", "Press Enter to toggle"},
    {30u, SR_HII_OP_REF, 0u, "Advanced", "", "Press Enter to open"}
};

static SrFirmwareKey key(uint16_t scan, uint16_t unicode) {
    SrFirmwareKey value;
    value.scan_code = scan;
    value.unicode_char = unicode;
    return value;
}

static void test_navigation_and_repeat(void) {
    SrScreenReaderSession session;
    int exit_requested = 0;

    sr_session_init(&session, 3u);
    assert(sr_session_apply_hii(&session, records, 3u, 0));
    assert(sr_session_current(&session)->id == 10u);

    assert(sr_legacy_session_handle_key(&session, key(0x0002u, 0u), &exit_requested));
    assert(!exit_requested);
    assert(sr_session_current(&session)->id == 20u);
    assert(strstr(session.speech.current.text, "Secure Boot") != NULL);

    assert(sr_legacy_session_handle_key(&session, key(0u, (uint16_t)'R'), &exit_requested));
    assert(strstr(session.speech.current.text, "Secure Boot") != NULL);

    assert(sr_legacy_session_handle_key(&session, key(0x0001u, 0u), &exit_requested));
    assert(sr_session_current(&session)->id == 10u);
}

static void test_chooser_end_to_end(void) {
    SrScreenReaderSession session;
    int exit_requested = 0;

    sr_session_init(&session, 3u);
    assert(sr_session_apply_hii(&session, records, 3u, 0));

    assert(sr_legacy_session_handle_key(&session, key(0x0012u, 0u), &exit_requested));
    assert(session.nav.chooser_open);
    assert(strstr(session.speech.current.text, "Item chooser") != NULL);

    assert(sr_legacy_session_handle_key(&session, key(0u, (uint16_t)'s'), &exit_requested));
    assert(session.nav.chooser_match_count == 1u);
    assert(strstr(session.speech.current.text, "Secure Boot") != NULL);

    assert(sr_legacy_session_handle_key(&session, key(0u, 0x000du), &exit_requested));
    assert(!session.nav.chooser_open);
    assert(sr_session_current(&session)->id == 20u);
}

static void test_exit(void) {
    SrScreenReaderSession session;
    int exit_requested = 0;

    sr_session_init(&session, 3u);
    assert(sr_session_apply_hii(&session, records, 3u, 0));
    assert(sr_legacy_session_handle_key(&session, key(0u, 0x001bu), &exit_requested));
    assert(exit_requested);
    assert(sr_session_current(&session)->id == 10u);
}

int main(void) {
    test_navigation_and_repeat();
    test_chooser_end_to_end();
    test_exit();
    puts("UEFI_LEGACY_SESSION_ADAPTER_TESTS=PASS");
    return 0;
}
