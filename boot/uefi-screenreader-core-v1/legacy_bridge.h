#ifndef QEVARYNOX_UEFI_SCREENREADER_LEGACY_BRIDGE_H
#define QEVARYNOX_UEFI_SCREENREADER_LEGACY_BRIDGE_H

#include "hii_adapter.h"

typedef struct {
    uint32_t id;
    uint8_t raw_opcode;
    uint32_t flags;
    const char *prompt;
    const char *value;
    const char *help;
} SrLegacyIfrRecord;

typedef struct {
    uint16_t scan_code;
    uint16_t unicode_char;
} SrFirmwareKey;

typedef enum {
    SR_FW_CMD_NONE = 0,
    SR_FW_CMD_NAVIGATE,
    SR_FW_CMD_REPEAT,
    SR_FW_CMD_CHOOSER_OPEN,
    SR_FW_CMD_CHOOSER_CANCEL,
    SR_FW_CMD_CHOOSER_SELECT,
    SR_FW_CMD_CHOOSER_NEXT,
    SR_FW_CMD_CHOOSER_PREVIOUS,
    SR_FW_CMD_CHOOSER_BACKSPACE,
    SR_FW_CMD_CHOOSER_TYPE,
    SR_FW_CMD_EXIT
} SrFirmwareCommandKind;

typedef struct {
    SrFirmwareCommandKind kind;
    SrNavCommand nav;
    char ch;
} SrFirmwareCommand;

int sr_legacy_ifr_is_prompt_opcode(uint8_t raw_opcode);
SrHiiOpcode sr_legacy_ifr_opcode(uint8_t raw_opcode);
int sr_legacy_ifr_convert(
    const SrLegacyIfrRecord *source,
    size_t source_count,
    SrHiiRecord *destination,
    size_t destination_capacity,
    size_t *destination_count
);
int sr_legacy_key_decode(
    SrFirmwareKey key,
    int chooser_open,
    SrFirmwareCommand *command
);

#endif
