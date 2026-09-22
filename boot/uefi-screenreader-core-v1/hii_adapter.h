#ifndef QEVARYNOX_UEFI_SCREENREADER_HII_ADAPTER_H
#define QEVARYNOX_UEFI_SCREENREADER_HII_ADAPTER_H

#include "screenreader_core.h"

#define SR_HII_MAX_ITEMS 128u
#define SR_HII_TEXT_CAPACITY 16384u

typedef enum {
    SR_HII_OP_UNKNOWN = 0,
    SR_HII_OP_SUBTITLE,
    SR_HII_OP_TEXT,
    SR_HII_OP_CHECKBOX,
    SR_HII_OP_ONE_OF,
    SR_HII_OP_NUMERIC,
    SR_HII_OP_STRING,
    SR_HII_OP_PASSWORD,
    SR_HII_OP_ACTION,
    SR_HII_OP_REF,
    SR_HII_OP_DATE,
    SR_HII_OP_TIME,
    SR_HII_OP_ORDERED_LIST
} SrHiiOpcode;

enum {
    SR_HII_FLAG_NONE     = 0u,
    SR_HII_FLAG_DISABLED = 1u << 0,
    SR_HII_FLAG_CHECKED  = 1u << 1,
    SR_HII_FLAG_SELECTED   = 1u << 2,
    SR_HII_FLAG_CHANGED    = 1u << 3,
    SR_HII_FLAG_DANGER     = 1u << 4,
    SR_HII_FLAG_SUPPRESSED = 1u << 5,
    SR_HII_FLAG_GRAYED     = 1u << 6
};

typedef struct {
    uint32_t id;
    SrHiiOpcode opcode;
    uint32_t flags;
    const char *prompt;
    const char *value;
    const char *help;
} SrHiiRecord;

typedef struct {
    SrItem items[SR_HII_MAX_ITEMS];
    size_t count;
    char text[SR_HII_TEXT_CAPACITY];
    size_t text_used;
} SrSemanticSnapshot;

void sr_hii_snapshot_reset(SrSemanticSnapshot *snapshot);
int sr_hii_snapshot_build(
    SrSemanticSnapshot *snapshot,
    const SrHiiRecord *records,
    size_t record_count
);
SrRole sr_hii_role_from_opcode(SrHiiOpcode opcode);

#endif
