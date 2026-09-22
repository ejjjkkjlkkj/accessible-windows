#include "legacy_bridge.h"

#include <assert.h>
#include <stdio.h>

static void test_ifr_opcode_parity(void) {
    static const uint8_t raw[] = {
        0x02u, 0x03u, 0x05u, 0x06u, 0x07u, 0x08u, 0x0cu,
        0x0du, 0x0fu, 0x1au, 0x1bu, 0x1cu, 0x23u
    };
    static const SrRole roles[] = {
        SR_ROLE_SEPARATOR, SR_ROLE_TEXT, SR_ROLE_CHOICE, SR_ROLE_TOGGLE,
        SR_ROLE_NUMERIC, SR_ROLE_PASSWORD, SR_ROLE_ACTION, SR_ROLE_ACTION,
        SR_ROLE_SUBMENU, SR_ROLE_DATE, SR_ROLE_TIME, SR_ROLE_EDIT,
        SR_ROLE_ORDERED_LIST
    };
    size_t i;

    for (i = 0u; i < sizeof(raw) / sizeof(raw[0]); ++i) {
        SrHiiOpcode opcode = sr_legacy_ifr_opcode(raw[i]);
        assert(opcode != SR_HII_OP_UNKNOWN);
        assert(sr_legacy_ifr_is_prompt_opcode(raw[i]));
        assert(sr_hii_role_from_opcode(opcode) == roles[i]);
    }
    assert(!sr_legacy_ifr_is_prompt_opcode(0x04u));
    assert(sr_legacy_ifr_opcode(0xffu) == SR_HII_OP_UNKNOWN);
}

static void test_ifr_conversion(void) {
    static const SrLegacyIfrRecord source[] = {
        {1u, 0x04u, 0u, "scope", "", ""},
        {2u, 0x05u, 0u, "Boot mode", "UEFI", ""},
        {3u, 0x1au, 0u, "Date", "2026-09-22", ""},
        {4u, 0x23u, 0u, "Boot order", "NVMe", ""}
    };
    SrHiiRecord out[3];
    size_t count = 99u;

    assert(sr_legacy_ifr_convert(source, 4u, out, 3u, &count));
    assert(count == 3u);
    assert(out[0].id == 2u && out[0].opcode == SR_HII_OP_ONE_OF);
    assert(out[1].id == 3u && out[1].opcode == SR_HII_OP_DATE);
    assert(out[2].id == 4u && out[2].opcode == SR_HII_OP_ORDERED_LIST);

    count = 99u;
    assert(!sr_legacy_ifr_convert(source, 4u, out, 2u, &count));
    assert(count == 0u);
}

static void assert_nav(uint16_t scan, uint16_t unicode, SrNavCommand nav, char ch) {
    SrFirmwareCommand command;
    SrFirmwareKey key = {scan, unicode};
    assert(sr_legacy_key_decode(key, 0, &command));
    assert(command.kind == SR_FW_CMD_NAVIGATE);
    assert(command.nav == nav);
    assert(command.ch == ch);
}

static void test_key_parity(void) {
    SrFirmwareCommand command;
    SrFirmwareKey key;

    assert_nav(0x0001u, 0u, SR_NAV_PREVIOUS, 0);
    assert_nav(0x0002u, 0u, SR_NAV_NEXT, 0);
    assert_nav(0x0005u, 0u, SR_NAV_HOME, 0);
    assert_nav(0x0006u, 0u, SR_NAV_END, 0);
    assert_nav(0x0009u, 0u, SR_NAV_PAGE_PREVIOUS, 0);
    assert_nav(0x000au, 0u, SR_NAV_PAGE_NEXT, 0);
    assert_nav(0x0010u, 0u, SR_NAV_ROLE_NEXT, 0);
    assert_nav(0x0011u, 0u, SR_NAV_ROLE_PREVIOUS, 0);
    assert_nav(0u, (uint16_t)'S', SR_NAV_FIRST_LETTER, 's');

    key.scan_code = 0x0012u; key.unicode_char = 0u;
    assert(sr_legacy_key_decode(key, 0, &command));
    assert(command.kind == SR_FW_CMD_CHOOSER_OPEN);

    key.scan_code = 0u; key.unicode_char = (uint16_t)'R';
    assert(sr_legacy_key_decode(key, 0, &command));
    assert(command.kind == SR_FW_CMD_REPEAT);

    key.scan_code = 0u; key.unicode_char = 0x001bu;
    assert(sr_legacy_key_decode(key, 0, &command));
    assert(command.kind == SR_FW_CMD_EXIT);
}

static void test_chooser_key_parity(void) {
    SrFirmwareCommand command;
    SrFirmwareKey key;

    key.scan_code = 0x0001u; key.unicode_char = 0u;
    assert(sr_legacy_key_decode(key, 1, &command));
    assert(command.kind == SR_FW_CMD_CHOOSER_PREVIOUS);

    key.scan_code = 0x0002u;
    assert(sr_legacy_key_decode(key, 1, &command));
    assert(command.kind == SR_FW_CMD_CHOOSER_NEXT);

    key.scan_code = 0u; key.unicode_char = 0x0008u;
    assert(sr_legacy_key_decode(key, 1, &command));
    assert(command.kind == SR_FW_CMD_CHOOSER_BACKSPACE);

    key.unicode_char = (uint16_t)'A';
    assert(sr_legacy_key_decode(key, 1, &command));
    assert(command.kind == SR_FW_CMD_CHOOSER_TYPE);
    assert(command.ch == 'a');

    key.unicode_char = 0x000du;
    assert(sr_legacy_key_decode(key, 1, &command));
    assert(command.kind == SR_FW_CMD_CHOOSER_SELECT);

    key.unicode_char = 0x001bu;
    assert(sr_legacy_key_decode(key, 1, &command));
    assert(command.kind == SR_FW_CMD_CHOOSER_CANCEL);
}

int main(void) {
    test_ifr_opcode_parity();
    test_ifr_conversion();
    test_key_parity();
    test_chooser_key_parity();
    puts("UEFI_LEGACY_BRIDGE_TESTS=PASS");
    return 0;
}
