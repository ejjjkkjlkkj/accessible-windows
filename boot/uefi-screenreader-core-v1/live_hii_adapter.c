#include "live_hii_adapter.h"

#define SR_HII_PACKAGE_HEADER_SIZE 4u
#define SR_HII_PACKAGE_LIST_HEADER_SIZE 20u
#define SR_HII_PACKAGE_FORMS 0x02u
#define SR_HII_PACKAGE_END 0xdfu

static uint32_t sr_live_rd32(const uint8_t *p) {
    return (uint32_t)p[0] |
           ((uint32_t)p[1] << 8) |
           ((uint32_t)p[2] << 16) |
           ((uint32_t)p[3] << 24);
}

static void sr_live_zero(SrLiveHiiSnapshot *snapshot) {
    size_t i;
    if (!snapshot) return;
    snapshot->count = 0u;
    snapshot->text_used = 0u;
    for (i = 0u; i < SR_HII_MAX_ITEMS; ++i) {
        snapshot->records[i].id = 0u;
        snapshot->records[i].opcode = SR_HII_OP_UNKNOWN;
        snapshot->records[i].flags = 0u;
        snapshot->records[i].prompt = "";
        snapshot->records[i].value = "";
        snapshot->records[i].help = "";
    }
}

void sr_live_hii_reset(SrLiveHiiSnapshot *snapshot) {
    sr_live_zero(snapshot);
}

static size_t sr_live_cstr_length(const char *text, size_t capacity) {
    size_t i;
    if (!text || capacity == 0u) return capacity;
    for (i = 0u; i < capacity; ++i) {
        if (text[i] == '\0') return i;
    }
    return capacity;
}

static char *sr_live_resolve_required(
    SrLiveHiiSnapshot *snapshot,
    SrLiveHiiStringResolver resolver,
    void *context,
    uint16_t token
) {
    char *dst;
    size_t remaining;
    size_t len;

    if (!snapshot || !resolver || token == 0u) return NULL;
    if (snapshot->text_used >= SR_HII_TEXT_CAPACITY) return NULL;

    dst = &snapshot->text[snapshot->text_used];
    remaining = SR_HII_TEXT_CAPACITY - snapshot->text_used;
    dst[0] = '\0';
    if (!resolver(context, token, dst, remaining)) return NULL;

    len = sr_live_cstr_length(dst, remaining);
    if (len >= remaining) return NULL;
    snapshot->text_used += len + 1u;
    return dst;
}

static char *sr_live_resolve_optional(
    SrLiveHiiSnapshot *snapshot,
    SrLiveHiiStringResolver resolver,
    void *context,
    uint16_t token
) {
    char *saved;
    size_t used;

    if (token == 0u || !resolver) return "";
    used = snapshot ? snapshot->text_used : 0u;
    saved = sr_live_resolve_required(snapshot, resolver, context, token);
    if (saved) return saved;
    if (snapshot) snapshot->text_used = used;
    return "";
}

static char *sr_live_resolve_value(
    SrLiveHiiSnapshot *snapshot,
    SrLiveHiiValueResolver resolver,
    void *context,
    const SrIfrStatementMeta *meta,
    uint32_t *flags_io
) {
    char *dst;
    size_t remaining;
    size_t len;
    int status;

    if (!snapshot || !meta || !flags_io) return NULL;
    if (!resolver || meta->raw_opcode == 0x08u) return "";
    if (snapshot->text_used >= SR_HII_TEXT_CAPACITY) return NULL;

    dst = &snapshot->text[snapshot->text_used];
    remaining = SR_HII_TEXT_CAPACITY - snapshot->text_used;
    dst[0] = '\0';
    status = resolver(context, meta, dst, remaining, flags_io);
    if (status < 0) return NULL;
    if (status == 0) return "";

    len = sr_live_cstr_length(dst, remaining);
    if (len >= remaining) return NULL;
    snapshot->text_used += len + 1u;
    return dst;
}

static int sr_live_append_forms(
    SrLiveHiiSnapshot *snapshot,
    const uint8_t package_guid[SR_IFR_GUID_BYTES],
    const uint8_t *ifr,
    size_t ifr_size,
    SrLiveHiiStringResolver string_resolver,
    SrLiveHiiValueResolver value_resolver,
    void *context
) {
    SrIfrStatementMeta local[SR_HII_MAX_ITEMS];
    size_t local_count = 0u;
    size_t i;

    if (!snapshot) return 0;
    if (!sr_ifr_collect_statements(
        package_guid,
        ifr,
        ifr_size,
        local,
        SR_HII_MAX_ITEMS,
        &local_count
    )) return 0;
    if (local_count > SR_HII_MAX_ITEMS - snapshot->count) return 0;

    for (i = 0u; i < local_count; ++i) {
        SrLegacyIfrRecord legacy;
        SrHiiRecord *record = &snapshot->records[snapshot->count];
        SrIfrStatementMeta *meta = &snapshot->meta[snapshot->count];
        char *prompt;
        char *help;
        char *value;
        uint32_t flags;
        size_t converted = 0u;
        size_t j;

        *meta = local[i];
        for (j = 0u; j < snapshot->count; ++j) {
            if (snapshot->records[j].id == meta->stable_id) return 0;
        }

        prompt = sr_live_resolve_required(
            snapshot,
            string_resolver,
            context,
            meta->prompt_token
        );
        if (!prompt) return 0;

        help = sr_live_resolve_optional(
            snapshot,
            string_resolver,
            context,
            meta->help_token
        );

        flags = sr_ifr_question_flags_to_hii(meta->question_flags);
        value = sr_live_resolve_value(
            snapshot,
            value_resolver,
            context,
            meta,
            &flags
        );
        if (!value) return 0;

        sr_ifr_bind_legacy_record(meta, prompt, value, help, &legacy);
        legacy.flags = flags;

        if (!sr_legacy_ifr_convert(&legacy, 1u, record, 1u, &converted) ||
            converted != 1u) {
            return 0;
        }
        ++snapshot->count;
    }

    return 1;
}

int sr_live_hii_build_package_list(
    SrLiveHiiSnapshot *snapshot,
    const uint8_t *package_list,
    size_t package_list_size,
    SrLiveHiiStringResolver string_resolver,
    SrLiveHiiValueResolver value_resolver,
    void *context
) {
    const uint8_t *package_guid;
    size_t list_length;
    size_t offset;
    int saw_forms = 0;

    if (!snapshot) return 0;
    sr_live_zero(snapshot);

    if (!package_list ||
        package_list_size < SR_HII_PACKAGE_LIST_HEADER_SIZE ||
        !string_resolver) {
        return 0;
    }

    list_length = (size_t)sr_live_rd32(package_list + 16u);
    if (list_length < SR_HII_PACKAGE_LIST_HEADER_SIZE ||
        list_length > package_list_size) {
        return 0;
    }

    package_guid = package_list;
    offset = SR_HII_PACKAGE_LIST_HEADER_SIZE;

    while (offset < list_length) {
        uint32_t header;
        size_t length;
        uint8_t type;

        if (list_length - offset < SR_HII_PACKAGE_HEADER_SIZE) {
            sr_live_zero(snapshot);
            return 0;
        }

        header = sr_live_rd32(package_list + offset);
        length = (size_t)(header & 0x00ffffffu);
        type = (uint8_t)(header >> 24);

        if (length < SR_HII_PACKAGE_HEADER_SIZE ||
            length > list_length - offset) {
            sr_live_zero(snapshot);
            return 0;
        }

        if (type == SR_HII_PACKAGE_END) {
            if (length != SR_HII_PACKAGE_HEADER_SIZE ||
                offset + length != list_length) {
                sr_live_zero(snapshot);
                return 0;
            }
            return saw_forms && snapshot->count != 0u;
        }

        if (type == SR_HII_PACKAGE_FORMS) {
            saw_forms = 1;
            if (!sr_live_append_forms(
                snapshot,
                package_guid,
                package_list + offset + SR_HII_PACKAGE_HEADER_SIZE,
                length - SR_HII_PACKAGE_HEADER_SIZE,
                string_resolver,
                value_resolver,
                context
            )) {
                sr_live_zero(snapshot);
                return 0;
            }
        }

        offset += length;
    }

    sr_live_zero(snapshot);
    return 0;
}
