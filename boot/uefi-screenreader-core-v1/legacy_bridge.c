#include "legacy_bridge.h"

static char sr_bridge_ascii_lower(uint16_t ch) {
    if (ch >= (uint16_t)'A' && ch <= (uint16_t)'Z') {
        ch = (uint16_t)(ch - (uint16_t)'A' + (uint16_t)'a');
    }
    if (ch > 0x7fu) return 0;
    return (char)ch;
}

int sr_legacy_ifr_is_prompt_opcode(uint8_t raw_opcode) {
    return sr_legacy_ifr_opcode(raw_opcode) != SR_HII_OP_UNKNOWN;
}

SrHiiOpcode sr_legacy_ifr_opcode(uint8_t raw_opcode) {
    switch (raw_opcode) {
        case 0x02u: return SR_HII_OP_SUBTITLE;
        case 0x03u: return SR_HII_OP_TEXT;
        case 0x05u: return SR_HII_OP_ONE_OF;
        case 0x06u: return SR_HII_OP_CHECKBOX;
        case 0x07u: return SR_HII_OP_NUMERIC;
        case 0x08u: return SR_HII_OP_PASSWORD;
        case 0x0cu: return SR_HII_OP_ACTION;
        case 0x0du: return SR_HII_OP_ACTION;
        case 0x0fu: return SR_HII_OP_REF;
        case 0x1au: return SR_HII_OP_DATE;
        case 0x1bu: return SR_HII_OP_TIME;
        case 0x1cu: return SR_HII_OP_STRING;
        case 0x23u: return SR_HII_OP_ORDERED_LIST;
        default: return SR_HII_OP_UNKNOWN;
    }
}

int sr_legacy_ifr_convert(
    const SrLegacyIfrRecord *source,
    size_t source_count,
    SrHiiRecord *destination,
    size_t destination_capacity,
    size_t *destination_count
) {
    size_t needed = 0u;
    size_t i;
    size_t out = 0u;

    if (destination_count) *destination_count = 0u;
    if (source_count && !source) return 0;

    for (i = 0u; i < source_count; ++i) {
        if (sr_legacy_ifr_is_prompt_opcode(source[i].raw_opcode)) ++needed;
    }
    if (needed > destination_capacity) return 0;
    if (needed && !destination) return 0;

    for (i = 0u; i < source_count; ++i) {
        SrHiiOpcode opcode = sr_legacy_ifr_opcode(source[i].raw_opcode);
        if (opcode == SR_HII_OP_UNKNOWN) continue;
        destination[out].id = source[i].id;
        destination[out].opcode = opcode;
        destination[out].flags = source[i].flags;
        destination[out].prompt = source[i].prompt;
        destination[out].value = source[i].value;
        destination[out].help = source[i].help;
        ++out;
    }

    if (destination_count) *destination_count = out;
    return 1;
}

int sr_legacy_key_decode(
    SrFirmwareKey key,
    int chooser_open,
    SrFirmwareCommand *command
) {
    char ch;

    if (!command) return 0;
    command->kind = SR_FW_CMD_NONE;
    command->nav = SR_NAV_NEXT;
    command->ch = 0;

    if (chooser_open) {
        if (key.unicode_char == 0x001bu || key.scan_code == 0x0017u ||
            key.scan_code == 0x0012u) {
            command->kind = SR_FW_CMD_CHOOSER_CANCEL;
            return 1;
        }
        if (key.unicode_char == 0x000du) {
            command->kind = SR_FW_CMD_CHOOSER_SELECT;
            return 1;
        }
        if (key.scan_code == 0x0001u) {
            command->kind = SR_FW_CMD_CHOOSER_PREVIOUS;
            return 1;
        }
        if (key.scan_code == 0x0002u) {
            command->kind = SR_FW_CMD_CHOOSER_NEXT;
            return 1;
        }
        if (key.unicode_char == 0x0008u) {
            command->kind = SR_FW_CMD_CHOOSER_BACKSPACE;
            return 1;
        }
        ch = sr_bridge_ascii_lower(key.unicode_char);
        if ((ch >= 'a' && ch <= 'z') || (ch >= '0' && ch <= '9')) {
            command->kind = SR_FW_CMD_CHOOSER_TYPE;
            command->ch = ch;
            return 1;
        }
        return 0;
    }

    if (key.unicode_char == 0x001bu || key.scan_code == 0x0017u) {
        command->kind = SR_FW_CMD_EXIT;
        return 1;
    }
    if (key.scan_code == 0x0012u) {
        command->kind = SR_FW_CMD_CHOOSER_OPEN;
        return 1;
    }
    if (key.unicode_char == (uint16_t)'r' || key.unicode_char == (uint16_t)'R') {
        command->kind = SR_FW_CMD_REPEAT;
        return 1;
    }
    if (key.scan_code == 0x0010u) {
        command->kind = SR_FW_CMD_NAVIGATE;
        command->nav = SR_NAV_ROLE_NEXT;
        return 1;
    }
    if (key.scan_code == 0x0011u) {
        command->kind = SR_FW_CMD_NAVIGATE;
        command->nav = SR_NAV_ROLE_PREVIOUS;
        return 1;
    }

    ch = sr_bridge_ascii_lower(key.unicode_char);
    if (ch >= 'a' && ch <= 'z') {
        command->kind = SR_FW_CMD_NAVIGATE;
        command->nav = SR_NAV_FIRST_LETTER;
        command->ch = ch;
        return 1;
    }

    switch (key.scan_code) {
        case 0x0001u:
            command->kind = SR_FW_CMD_NAVIGATE;
            command->nav = SR_NAV_PREVIOUS;
            return 1;
        case 0x0002u:
            command->kind = SR_FW_CMD_NAVIGATE;
            command->nav = SR_NAV_NEXT;
            return 1;
        case 0x0005u:
            command->kind = SR_FW_CMD_NAVIGATE;
            command->nav = SR_NAV_HOME;
            return 1;
        case 0x0006u:
            command->kind = SR_FW_CMD_NAVIGATE;
            command->nav = SR_NAV_END;
            return 1;
        case 0x0009u:
            command->kind = SR_FW_CMD_NAVIGATE;
            command->nav = SR_NAV_PAGE_PREVIOUS;
            return 1;
        case 0x000au:
            command->kind = SR_FW_CMD_NAVIGATE;
            command->nav = SR_NAV_PAGE_NEXT;
            return 1;
        default:
            return 0;
    }
}
