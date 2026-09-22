#ifndef QEVARYNOX_UEFI_SCREENREADER_IFR_COLLECTOR_H
#define QEVARYNOX_UEFI_SCREENREADER_IFR_COLLECTOR_H

#include "legacy_bridge.h"

#define SR_IFR_GUID_BYTES 16u

typedef struct {
    uint8_t raw_opcode;
    uint16_t prompt_token;
    uint16_t help_token;
    uint16_t question_id;
    uint16_t form_id;
    uint8_t question_flags;
    uint8_t has_question_id;
    uint32_t stable_id;
} SrIfrStatementMeta;

int sr_ifr_collect_statements(
    const uint8_t package_guid[SR_IFR_GUID_BYTES],
    const uint8_t *ifr,
    size_t ifr_size,
    SrIfrStatementMeta *out,
    size_t out_capacity,
    size_t *out_count
);

uint32_t sr_ifr_question_flags_to_hii(uint8_t question_flags);

void sr_ifr_bind_legacy_record(
    const SrIfrStatementMeta *meta,
    const char *prompt,
    const char *value,
    const char *help,
    SrLegacyIfrRecord *record
);

#endif
