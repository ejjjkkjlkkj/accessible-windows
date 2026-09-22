#include "ifr_collector.h"

#include <assert.h>
#include <stdio.h>
#include <string.h>

static const uint8_t guid_a[SR_IFR_GUID_BYTES] = {
    0x7bu, 0x59u, 0x10u, 0x4au, 0xc0u, 0x0du, 0x41u, 0x58u,
    0x87u, 0xffu, 0xf0u, 0x4du, 0x63u, 0x96u, 0xa9u, 0x15u
};

static void test_collect_question_ids_and_roles(void) {
    static const uint8_t ifr[] = {
        0x01u, 0x86u, 0x34u, 0x12u, 0x01u, 0x00u,
        0x02u, 0x87u, 0x02u, 0x00u, 0x03u, 0x00u, 0x00u,
        0x05u, 0x8eu, 0x10u, 0x00u, 0x11u, 0x00u, 0x22u, 0x00u,
        0x01u, 0x00u, 0x04u, 0x00u, 0x01u, 0x00u,
        0x1au, 0x0eu, 0x20u, 0x00u, 0x21u, 0x00u, 0x23u, 0x00u,
        0x01u, 0x00u, 0x08u, 0x00u, 0x00u, 0x00u,
        0x23u, 0x0fu, 0x30u, 0x00u, 0x31u, 0x00u, 0x24u, 0x00u,
        0x01u, 0x00u, 0x0cu, 0x00u, 0x00u, 0x04u, 0x00u,
        0x29u, 0x02u
    };
    SrIfrStatementMeta meta[4];
    size_t count = 0u;

    assert(sr_ifr_collect_statements(guid_a, ifr, sizeof(ifr), meta, 4u, &count));
    assert(count == 4u);

    assert(meta[0].raw_opcode == 0x02u);
    assert(meta[0].form_id == 0x1234u);
    assert(!meta[0].has_question_id);
    assert(meta[0].prompt_token == 2u);

    assert(meta[1].raw_opcode == 0x05u);
    assert(meta[1].has_question_id);
    assert(meta[1].question_id == 0x22u);
    assert(meta[1].question_flags == 0x01u);
    assert(sr_ifr_question_flags_to_hii(meta[1].question_flags) & SR_HII_FLAG_READ_ONLY);

    assert(meta[2].raw_opcode == 0x1au);
    assert(meta[2].question_id == 0x23u);
    assert(meta[3].raw_opcode == 0x23u);
    assert(meta[3].question_id == 0x24u);

    assert(meta[0].stable_id != 0u);
    assert(meta[1].stable_id != meta[2].stable_id);
}

static void test_question_id_stays_stable_across_prompt_change(void) {
    static const uint8_t first[] = {
        0x01u, 0x06u, 0x01u, 0x00u, 0x01u, 0x00u,
        0x06u, 0x0eu, 0x10u, 0x00u, 0x11u, 0x00u, 0x44u, 0x00u,
        0x01u, 0x00u, 0x00u, 0x00u, 0x00u, 0x00u
    };
    static const uint8_t refreshed[] = {
        0x01u, 0x06u, 0x01u, 0x00u, 0x01u, 0x00u,
        0x06u, 0x0eu, 0x55u, 0x00u, 0x11u, 0x00u, 0x44u, 0x00u,
        0x01u, 0x00u, 0x00u, 0x00u, 0x00u, 0x00u
    };
    SrIfrStatementMeta a[1];
    SrIfrStatementMeta b[1];
    size_t ac = 0u;
    size_t bc = 0u;

    assert(sr_ifr_collect_statements(guid_a, first, sizeof(first), a, 1u, &ac));
    assert(sr_ifr_collect_statements(guid_a, refreshed, sizeof(refreshed), b, 1u, &bc));
    assert(ac == 1u && bc == 1u);
    assert(a[0].question_id == b[0].question_id);
    assert(a[0].prompt_token != b[0].prompt_token);
    assert(a[0].stable_id == b[0].stable_id);
}

static void test_form_id_separates_question_ids(void) {
    static const uint8_t form_a[] = {
        0x01u, 0x06u, 0x01u, 0x00u, 0x01u, 0x00u,
        0x06u, 0x0eu, 0x10u, 0x00u, 0x11u, 0x00u, 0x44u, 0x00u,
        0x01u, 0x00u, 0x00u, 0x00u, 0x00u, 0x00u
    };
    static const uint8_t form_b[] = {
        0x01u, 0x06u, 0x02u, 0x00u, 0x01u, 0x00u,
        0x06u, 0x0eu, 0x10u, 0x00u, 0x11u, 0x00u, 0x44u, 0x00u,
        0x01u, 0x00u, 0x00u, 0x00u, 0x00u, 0x00u
    };
    SrIfrStatementMeta a[1], b[1];
    size_t count = 0u;

    assert(sr_ifr_collect_statements(guid_a, form_a, sizeof(form_a), a, 1u, &count));
    assert(sr_ifr_collect_statements(guid_a, form_b, sizeof(form_b), b, 1u, &count));
    assert(a[0].question_id == b[0].question_id);
    assert(a[0].form_id != b[0].form_id);
    assert(a[0].stable_id != b[0].stable_id);
}

static void test_package_guid_separates_question_ids(void) {
    uint8_t guid_b[SR_IFR_GUID_BYTES];
    static const uint8_t ifr[] = {
        0x06u, 0x0eu, 0x10u, 0x00u, 0x11u, 0x00u, 0x44u, 0x00u,
        0x01u, 0x00u, 0x00u, 0x00u, 0x00u, 0x00u
    };
    SrIfrStatementMeta a[1], b[1];
    size_t count = 0u;

    memcpy(guid_b, guid_a, sizeof(guid_b));
    guid_b[0] ^= 0xffu;

    assert(sr_ifr_collect_statements(guid_a, ifr, sizeof(ifr), a, 1u, &count));
    assert(sr_ifr_collect_statements(guid_b, ifr, sizeof(ifr), b, 1u, &count));
    assert(a[0].stable_id != b[0].stable_id);
}

static void test_transactional_malformed_input(void) {
    static const uint8_t bad_zero_length[] = {0x05u, 0x00u};
    static const uint8_t bad_question_short[] = {
        0x05u, 0x06u, 0x10u, 0x00u, 0x11u, 0x00u
    };
    SrIfrStatementMeta out[2];
    size_t count = 99u;

    assert(!sr_ifr_collect_statements(guid_a, bad_zero_length, sizeof(bad_zero_length), out, 2u, &count));
    assert(count == 0u);
    count = 99u;
    assert(!sr_ifr_collect_statements(guid_a, bad_question_short, sizeof(bad_question_short), out, 2u, &count));
    assert(count == 0u);
}

static void test_bind_record_propagates_read_only(void) {
    SrIfrStatementMeta meta;
    SrLegacyIfrRecord record;

    memset(&meta, 0, sizeof(meta));
    meta.raw_opcode = 0x05u;
    meta.stable_id = 0x12345678u;
    meta.question_flags = 0x01u;

    sr_ifr_bind_legacy_record(&meta, "Boot mode", "UEFI", "Help", &record);
    assert(record.id == 0x12345678u);
    assert(record.raw_opcode == 0x05u);
    assert(record.flags & SR_HII_FLAG_READ_ONLY);
    assert(strcmp(record.prompt, "Boot mode") == 0);
}

int main(void) {
    test_collect_question_ids_and_roles();
    test_question_id_stays_stable_across_prompt_change();
    test_form_id_separates_question_ids();
    test_package_guid_separates_question_ids();
    test_transactional_malformed_input();
    test_bind_record_propagates_read_only();
    puts("UEFI_IFR_COLLECTOR_TESTS=PASS");
    return 0;
}
