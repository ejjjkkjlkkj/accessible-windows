#ifndef QEVARYNOX_UEFI_SCREENREADER_LIVE_HII_ADAPTER_H
#define QEVARYNOX_UEFI_SCREENREADER_LIVE_HII_ADAPTER_H

#include "ifr_collector.h"

typedef int (*SrLiveHiiStringResolver)(
    void *context,
    uint16_t token,
    char *out,
    size_t out_capacity
);

typedef int (*SrLiveHiiValueResolver)(
    void *context,
    const SrIfrStatementMeta *meta,
    char *out,
    size_t out_capacity,
    uint32_t *flags_io
);

typedef struct {
    SrHiiRecord records[SR_HII_MAX_ITEMS];
    SrIfrStatementMeta meta[SR_HII_MAX_ITEMS];
    size_t count;
    char text[SR_HII_TEXT_CAPACITY];
    size_t text_used;
} SrLiveHiiSnapshot;

void sr_live_hii_reset(SrLiveHiiSnapshot *snapshot);

int sr_live_hii_build_package_list(
    SrLiveHiiSnapshot *snapshot,
    const uint8_t *package_list,
    size_t package_list_size,
    SrLiveHiiStringResolver string_resolver,
    SrLiveHiiValueResolver value_resolver,
    void *context
);

#endif
