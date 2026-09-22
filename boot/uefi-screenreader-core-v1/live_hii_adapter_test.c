#include "live_hii_adapter.h"
#include "screenreader_session.h"

#include <assert.h>
#include <stdio.h>
#include <string.h>

typedef struct {
    int value_calls;
    int password_value_calls;
    int fail_value_question;
} TestContext;

static void wr32(uint8_t *p, uint32_t value) {
    p[0] = (uint8_t)(value & 0xffu);
    p[1] = (uint8_t)((value >> 8) & 0xffu);
    p[2] = (uint8_t)((value >> 16) & 0xffu);
    p[3] = (uint8_t)((value >> 24) & 0xffu);
}

static size_t put_question(
    uint8_t *p,
    uint8_t opcode,
    uint16_t prompt,
    uint16_t help,
    uint16_t question_id,
    uint8_t flags
) {
    p[0] = opcode;
    p[1] = 0x0eu;
    p[2] = (uint8_t)prompt;
    p[3] = (uint8_t)(prompt >> 8);
    p[4] = (uint8_t)help;
    p[5] = (uint8_t)(help >> 8);
    p[6] = (uint8_t)question_id;
    p[7] = (uint8_t)(question_id >> 8);
    p[8] = 0x01u;
    p[9] = 0x00u;
    p[10] = (uint8_t)(question_id & 0xffu);
    p[11] = 0x00u;
    p[12] = flags;
    p[13] = 0x00u;
    return 14u;
}

static size_t build_package(uint8_t *out, size_t cap, uint16_t boot_prompt) {
    static const uint8_t guid[16] = {
        0x7bu,0x59u,0x10u,0x4au,0xc0u,0x0du,0x41u,0x58u,
        0x87u,0xffu,0xf0u,0x4du,0x63u,0x96u,0xa9u,0x15u
    };
    uint8_t ifr[96];
    size_t n = 0u;
    size_t forms_len;
    size_t total;

    assert(cap >= 160u);
    memset(out, 0, cap);
    memcpy(out, guid, sizeof(guid));

    ifr[n++] = 0x01u; ifr[n++] = 0x06u;
    ifr[n++] = 0x34u; ifr[n++] = 0x12u;
    ifr[n++] = 0x01u; ifr[n++] = 0x00u;

    n += put_question(ifr + n, 0x05u, boot_prompt, 0x11u, 0x0100u, 0u);
    n += put_question(ifr + n, 0x06u, 0x20u, 0x21u, 0x0101u, 0x01u);
    n += put_question(ifr + n, 0x0fu, 0x30u, 0x31u, 0x0102u, 0u);
    n += put_question(ifr + n, 0x08u, 0x40u, 0x41u, 0x0103u, 0u);
    ifr[n++] = 0x29u; ifr[n++] = 0x02u;

    forms_len = 4u + n;
    total = 20u + forms_len + 4u;
    memcpy(out + 20u + 4u, ifr, n);

    wr32(out + 16u, (uint32_t)total);
    wr32(out + 20u, ((uint32_t)0x02u << 24) | (uint32_t)forms_len);
    wr32(out + 20u + forms_len, ((uint32_t)0xdfu << 24) | 4u);
    return total;
}

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

static int resolve_string(void *ctx, uint16_t token, char *out, size_t cap) {
    (void)ctx;
    switch (token) {
        case 0x10u: return copy_text(out, cap, "Boot mode");
        case 0x55u: return copy_text(out, cap, "Boot mode refreshed");
        case 0x11u: return copy_text(out, cap, "Select boot mode");
        case 0x20u: return copy_text(out, cap, "Secure Boot");
        case 0x21u: return copy_text(out, cap, "Security policy");
        case 0x30u: return copy_text(out, cap, "Advanced");
        case 0x31u: return copy_text(out, cap, "Open advanced settings");
        case 0x40u: return copy_text(out, cap, "Administrator password");
        case 0x41u: return copy_text(out, cap, "Password value must remain private");
        default: return 0;
    }
}

static int resolve_value(
    void *opaque,
    const SrIfrStatementMeta *meta,
    char *out,
    size_t cap,
    uint32_t *flags_io
) {
    TestContext *ctx = (TestContext *)opaque;
    assert(ctx && meta && out && flags_io);
    ++ctx->value_calls;

    if (meta->raw_opcode == 0x08u) {
        ++ctx->password_value_calls;
        return copy_text(out, cap, "SECRET") ? 1 : -1;
    }
    if ((int)meta->question_id == ctx->fail_value_question) return -1;

    switch (meta->question_id) {
        case 0x0100u:
            return copy_text(out, cap, "UEFI") ? 1 : -1;
        case 0x0101u:
            *flags_io |= SR_HII_FLAG_CHECKED;
            return copy_text(out, cap, "Enabled") ? 1 : -1;
        case 0x0102u:
            return 0;
        default:
            return 0;
    }
}

static void test_live_package_build(void) {
    uint8_t package[160];
    size_t bytes = build_package(package, sizeof(package), 0x10u);
    SrLiveHiiSnapshot live;
    TestContext ctx = {0,0,-1};

    assert(sr_live_hii_build_package_list(
        &live, package, bytes, resolve_string, resolve_value, &ctx
    ));
    assert(live.count == 4u);
    assert(strcmp(live.records[0].prompt, "Boot mode") == 0);
    assert(strcmp(live.records[0].value, "UEFI") == 0);
    assert(live.meta[0].question_id == 0x0100u);
    assert(live.meta[1].question_id == 0x0101u);
    assert(live.records[1].flags & SR_HII_FLAG_READ_ONLY);
    assert(live.records[1].flags & SR_HII_FLAG_CHECKED);
    assert(live.records[3].opcode == SR_HII_OP_PASSWORD);
    assert(strcmp(live.records[3].value, "") == 0);
    assert(ctx.password_value_calls == 0);
    assert(ctx.value_calls == 3);
}

static void test_focus_survives_prompt_token_refresh(void) {
    uint8_t first[160], refreshed[160];
    size_t first_bytes = build_package(first, sizeof(first), 0x10u);
    size_t refreshed_bytes = build_package(refreshed, sizeof(refreshed), 0x55u);
    SrLiveHiiSnapshot a, b;
    SrScreenReaderSession session;
    TestContext ctx = {0,0,-1};
    uint32_t secure_id;

    assert(sr_live_hii_build_package_list(
        &a, first, first_bytes, resolve_string, resolve_value, &ctx
    ));
    assert(sr_live_hii_build_package_list(
        &b, refreshed, refreshed_bytes, resolve_string, resolve_value, &ctx
    ));
    assert(a.meta[0].stable_id == b.meta[0].stable_id);

    sr_session_init(&session, 4u);
    assert(sr_session_apply_hii(&session, a.records, a.count, 0));
    assert(sr_session_navigate(&session, SR_NAV_NEXT, 0));
    secure_id = sr_session_current(&session)->id;
    assert(secure_id == a.meta[1].stable_id);

    assert(sr_session_apply_hii(&session, b.records, b.count, 1));
    assert(sr_session_current(&session)->id == secure_id);
    assert(sr_session_current(&session)->state & SR_STATE_READ_ONLY);
}

static void test_malformed_package_is_transactional(void) {
    uint8_t package[160];
    size_t bytes = build_package(package, sizeof(package), 0x10u);
    SrLiveHiiSnapshot live;
    TestContext ctx = {0,0,-1};

    assert(sr_live_hii_build_package_list(
        &live, package, bytes, resolve_string, resolve_value, &ctx
    ));
    assert(live.count == 4u);

    wr32(package + 20u, ((uint32_t)0x02u << 24) | 0x00ffffu);
    assert(!sr_live_hii_build_package_list(
        &live, package, bytes, resolve_string, resolve_value, &ctx
    ));
    assert(live.count == 0u);
    assert(live.text_used == 0u);
}

static void test_missing_end_package_is_rejected(void) {
    uint8_t package[160];
    size_t bytes = build_package(package, sizeof(package), 0x10u);
    SrLiveHiiSnapshot live;
    TestContext ctx = {0,0,-1};

    assert(bytes > 4u);
    wr32(package + 16u, (uint32_t)(bytes - 4u));
    assert(!sr_live_hii_build_package_list(
        &live, package, bytes - 4u, resolve_string, resolve_value, &ctx
    ));
    assert(live.count == 0u);
}

static void test_duplicate_stable_id_is_rejected(void) {
    uint8_t package[160];
    size_t bytes = build_package(package, sizeof(package), 0x10u);
    SrLiveHiiSnapshot live;
    TestContext ctx = {0,0,-1};

    package[64u] = 0x00u;
    package[65u] = 0x01u;
    assert(!sr_live_hii_build_package_list(
        &live, package, bytes, resolve_string, resolve_value, &ctx
    ));
    assert(live.count == 0u);
}

static void test_value_failure_rejects_snapshot(void) {
    uint8_t package[160];
    size_t bytes = build_package(package, sizeof(package), 0x10u);
    SrLiveHiiSnapshot live;
    TestContext ctx = {0,0,0x0101};

    assert(!sr_live_hii_build_package_list(
        &live, package, bytes, resolve_string, resolve_value, &ctx
    ));
    assert(live.count == 0u);
    assert(live.text_used == 0u);
}

int main(void) {
    test_live_package_build();
    test_focus_survives_prompt_token_refresh();
    test_malformed_package_is_transactional();
    test_missing_end_package_is_rejected();
    test_duplicate_stable_id_is_rejected();
    test_value_failure_rejects_snapshot();
    puts("UEFI_LIVE_HII_ADAPTER_TESTS=PASS");
    return 0;
}
