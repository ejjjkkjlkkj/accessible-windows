#include "ifr_collector.h"

#define SR_IFR_FORM_OP 0x01u
#define SR_IFR_FLAG_READ_ONLY 0x01u

static uint16_t sr_ifr_rd16(const uint8_t *p) {
    return (uint16_t)((uint16_t)p[0] | ((uint16_t)p[1] << 8));
}

static int sr_ifr_question_opcode(uint8_t op) {
    switch (op) {
        case 0x05u:
        case 0x06u:
        case 0x07u:
        case 0x08u:
        case 0x0cu:
        case 0x0fu:
        case 0x1au:
        case 0x1bu:
        case 0x1cu:
        case 0x23u:
            return 1;
        default:
            return 0;
    }
}

static uint32_t sr_ifr_hash_byte(uint32_t hash, uint8_t value) {
    hash ^= (uint32_t)value;
    hash *= 16777619u;
    return hash;
}

static uint32_t sr_ifr_stable_id(
    const uint8_t package_guid[SR_IFR_GUID_BYTES],
    uint8_t raw_opcode,
    uint16_t form_id,
    uint16_t prompt_token,
    uint16_t question_id,
    int has_question_id
) {
    uint32_t hash = 2166136261u;
    size_t i;

    if (package_guid) {
        for (i = 0u; i < SR_IFR_GUID_BYTES; ++i) {
            hash = sr_ifr_hash_byte(hash, package_guid[i]);
        }
    }

    if (has_question_id) {
        hash = sr_ifr_hash_byte(hash, (uint8_t)'Q');
        hash = sr_ifr_hash_byte(hash, (uint8_t)(form_id & 0xffu));
        hash = sr_ifr_hash_byte(hash, (uint8_t)(form_id >> 8));
        hash = sr_ifr_hash_byte(hash, (uint8_t)(question_id & 0xffu));
        hash = sr_ifr_hash_byte(hash, (uint8_t)(question_id >> 8));
    } else {
        hash = sr_ifr_hash_byte(hash, (uint8_t)'S');
        hash = sr_ifr_hash_byte(hash, raw_opcode);
        hash = sr_ifr_hash_byte(hash, (uint8_t)(form_id & 0xffu));
        hash = sr_ifr_hash_byte(hash, (uint8_t)(form_id >> 8));
        hash = sr_ifr_hash_byte(hash, (uint8_t)(prompt_token & 0xffu));
        hash = sr_ifr_hash_byte(hash, (uint8_t)(prompt_token >> 8));
    }

    return hash ? hash : 1u;
}

static int sr_ifr_validate_and_count(
    const uint8_t *ifr,
    size_t ifr_size,
    size_t *statement_count
) {
    size_t offset = 0u;
    size_t count = 0u;

    if (statement_count) *statement_count = 0u;
    if (ifr_size && !ifr) return 0;

    while (offset < ifr_size) {
        uint8_t op;
        size_t length;

        if (ifr_size - offset < 2u) return 0;
        op = ifr[offset];
        length = (size_t)(ifr[offset + 1u] & 0x7fu);
        if (length < 2u || length > ifr_size - offset) return 0;

        if (sr_legacy_ifr_is_prompt_opcode(op)) {
            if (length < 6u) return 0;
            if (sr_ifr_question_opcode(op) && length < 13u) return 0;
            ++count;
        }

        offset += length;
    }

    if (statement_count) *statement_count = count;
    return 1;
}

int sr_ifr_collect_statements(
    const uint8_t package_guid[SR_IFR_GUID_BYTES],
    const uint8_t *ifr,
    size_t ifr_size,
    SrIfrStatementMeta *out,
    size_t out_capacity,
    size_t *out_count
) {
    size_t needed = 0u;
    size_t offset = 0u;
    size_t index = 0u;
    uint16_t form_id = 0u;

    if (out_count) *out_count = 0u;
    if (!sr_ifr_validate_and_count(ifr, ifr_size, &needed)) return 0;
    if (needed > out_capacity) return 0;
    if (needed && !out) return 0;

    while (offset < ifr_size) {
        uint8_t op = ifr[offset];
        size_t length = (size_t)(ifr[offset + 1u] & 0x7fu);

        if (op == SR_IFR_FORM_OP && length >= 6u) {
            form_id = sr_ifr_rd16(ifr + offset + 2u);
        }

        if (sr_legacy_ifr_is_prompt_opcode(op)) {
            SrIfrStatementMeta *meta = &out[index++];
            int has_question = sr_ifr_question_opcode(op);

            meta->raw_opcode = op;
            meta->prompt_token = sr_ifr_rd16(ifr + offset + 2u);
            meta->help_token = sr_ifr_rd16(ifr + offset + 4u);
            meta->form_id = form_id;
            meta->has_question_id = (uint8_t)(has_question ? 1u : 0u);
            meta->question_id = has_question ? sr_ifr_rd16(ifr + offset + 6u) : 0u;
            meta->question_flags = has_question ? ifr[offset + 12u] : 0u;
            meta->stable_id = sr_ifr_stable_id(
                package_guid,
                op,
                form_id,
                meta->prompt_token,
                meta->question_id,
                has_question
            );
        }

        offset += length;
    }

    if (out_count) *out_count = index;
    return 1;
}

uint32_t sr_ifr_question_flags_to_hii(uint8_t question_flags) {
    uint32_t flags = 0u;
    if (question_flags & SR_IFR_FLAG_READ_ONLY) flags |= SR_HII_FLAG_READ_ONLY;
    return flags;
}

void sr_ifr_bind_legacy_record(
    const SrIfrStatementMeta *meta,
    const char *prompt,
    const char *value,
    const char *help,
    SrLegacyIfrRecord *record
) {
    if (!record) return;

    if (!meta) {
        record->id = 0u;
        record->raw_opcode = 0u;
        record->flags = 0u;
        record->prompt = prompt;
        record->value = value;
        record->help = help;
        return;
    }

    record->id = meta->stable_id;
    record->raw_opcode = meta->raw_opcode;
    record->flags = sr_ifr_question_flags_to_hii(meta->question_flags);
    record->prompt = prompt;
    record->value = value;
    record->help = help;
}
