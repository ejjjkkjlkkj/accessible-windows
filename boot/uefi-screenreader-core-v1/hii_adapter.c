#include "hii_adapter.h"

static const char sr_empty_text[] = "";

void sr_hii_snapshot_reset(SrSemanticSnapshot *snapshot) {
    if (!snapshot) return;
    snapshot->count = 0;
    snapshot->text_used = 0;
    snapshot->text[0] = '\0';
}

SrRole sr_hii_role_from_opcode(SrHiiOpcode opcode) {
    switch (opcode) {
        case SR_HII_OP_SUBTITLE: return SR_ROLE_SEPARATOR;
        case SR_HII_OP_TEXT: return SR_ROLE_TEXT;
        case SR_HII_OP_CHECKBOX: return SR_ROLE_TOGGLE;
        case SR_HII_OP_ONE_OF: return SR_ROLE_CHOICE;
        case SR_HII_OP_NUMERIC: return SR_ROLE_NUMERIC;
        case SR_HII_OP_STRING: return SR_ROLE_TEXT;
        case SR_HII_OP_PASSWORD: return SR_ROLE_PASSWORD;
        case SR_HII_OP_ACTION: return SR_ROLE_ACTION;
        case SR_HII_OP_REF: return SR_ROLE_SUBMENU;
        default: return SR_ROLE_UNKNOWN;
    }
}

static const char *sr_hii_copy_text(SrSemanticSnapshot *snapshot, const char *src) {
    size_t start;
    size_t i = 0;

    if (!src || src[0] == '\0') return sr_empty_text;
    start = snapshot->text_used;

    while (src[i] != '\0') {
        unsigned char ch = (unsigned char)src[i++];
        if (snapshot->text_used + 1u >= SR_HII_TEXT_CAPACITY) return NULL;
        if (ch < 0x20u || ch == 0x7fu) ch = (unsigned char)' ';
        snapshot->text[snapshot->text_used++] = (char)ch;
    }

    if (snapshot->text_used + 1u > SR_HII_TEXT_CAPACITY) return NULL;
    snapshot->text[snapshot->text_used++] = '\0';
    return &snapshot->text[start];
}

static uint32_t sr_hii_state(uint32_t flags) {
    uint32_t state = SR_STATE_NONE;
    if (flags & (SR_HII_FLAG_DISABLED | SR_HII_FLAG_GRAYED)) state |= SR_STATE_DISABLED;
    if (flags & SR_HII_FLAG_CHECKED) state |= SR_STATE_CHECKED;
    if (flags & SR_HII_FLAG_SELECTED) state |= SR_STATE_SELECTED;
    if (flags & SR_HII_FLAG_CHANGED) state |= SR_STATE_CHANGED;
    if (flags & SR_HII_FLAG_DANGER) state |= SR_STATE_DANGER;
    return state;
}

int sr_hii_snapshot_build(
    SrSemanticSnapshot *snapshot,
    const SrHiiRecord *records,
    size_t record_count
) {
    size_t i;

    if (!snapshot) return 0;
    sr_hii_snapshot_reset(snapshot);
    if (record_count == 0) return 1;
    if (!records) return 0;

    for (i = 0; i < record_count; ++i) {
        SrItem *item;

        if (records[i].flags & SR_HII_FLAG_SUPPRESSED) continue;
        if (snapshot->count >= SR_HII_MAX_ITEMS) {
            sr_hii_snapshot_reset(snapshot);
            return 0;
        }

        item = &snapshot->items[snapshot->count];
        const char *label;
        const char *value;
        const char *hint;
        SrRole role = sr_hii_role_from_opcode(records[i].opcode);

        label = sr_hii_copy_text(snapshot, records[i].prompt);
        if (!label) {
            sr_hii_snapshot_reset(snapshot);
            return 0;
        }

        if (role == SR_ROLE_PASSWORD) {
            value = sr_empty_text;
        } else {
            value = sr_hii_copy_text(snapshot, records[i].value);
            if (!value) {
                sr_hii_snapshot_reset(snapshot);
                return 0;
            }
        }

        hint = sr_hii_copy_text(snapshot, records[i].help);
        if (!hint) {
            sr_hii_snapshot_reset(snapshot);
            return 0;
        }

        item->id = records[i].id;
        item->role = role;
        item->state = sr_hii_state(records[i].flags);
        item->label = label;
        item->value = value;
        item->hint = hint;
        ++snapshot->count;
    }

    return 1;
}
