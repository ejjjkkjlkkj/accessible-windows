#include "screenreader_session.h"

static int sr_session_text_equal(const char *a, const char *b) {
    size_t i = 0;
    if (!a || !b) return a == b;
    for (;;) {
        if (a[i] != b[i]) return 0;
        if (a[i] == '\0') return 1;
        ++i;
    }
}

static int sr_session_item_equal(const SrItem *a, const SrItem *b) {
    if (!a || !b) return a == b;
    return a->id == b->id &&
           a->role == b->role &&
           a->state == b->state &&
           sr_session_text_equal(a->label, b->label) &&
           sr_session_text_equal(a->value, b->value);
}

static int sr_session_emit_chooser(SrScreenReaderSession *session) {
    const SrItem *item;
    SrSpeechEvent event;

    if (!session) return 0;
    item = sr_chooser_current(&session->nav);
    if (!item) return 0;

    event.key = item->id;
    event.priority = SR_SPEECH_FOCUS;
    event.interruptible = 1u;
    if (sr_format_chooser(&session->nav, event.text, sizeof(event.text)) == 0u) return 0;
    (void)sr_speech_submit(&session->speech, &event);
    return 1;
}

static int sr_session_emit_focus(SrScreenReaderSession *session) {
    const SrItem *item;
    SrSpeechEvent event;

    if (!session) return 0;
    item = sr_nav_current(&session->nav);
    if (!item) return 0;

    event.key = item->id;
    event.priority = SR_SPEECH_FOCUS;
    event.interruptible = 1u;
    if (sr_format_focus(&session->nav, event.text, sizeof(event.text)) == 0u) return 0;
    (void)sr_speech_submit(&session->speech, &event);
    return 1;
}

void sr_session_init(SrScreenReaderSession *session, size_t page_size) {
    if (!session) return;
    sr_hii_snapshot_reset(&session->snapshots[0]);
    sr_hii_snapshot_reset(&session->snapshots[1]);
    session->active_snapshot = 0u;
    session->has_snapshot = 0u;
    session->page_size = page_size ? page_size : 5u;
    sr_nav_init(&session->nav, NULL, 0u, session->page_size);
    sr_speech_init(&session->speech);
}

int sr_session_apply_hii(
    SrScreenReaderSession *session,
    const SrHiiRecord *records,
    size_t record_count,
    int announce_change
) {
    uint8_t next_index;
    SrSemanticSnapshot *target;
    const SrItem *old_item;
    const SrItem *new_item;
    uint32_t old_id = 0u;
    int semantic_change;

    if (!session) return 0;

    old_item = sr_nav_current(&session->nav);
    if (old_item) old_id = old_item->id;

    next_index = session->has_snapshot ? (uint8_t)(session->active_snapshot ^ 1u) : 0u;
    target = &session->snapshots[next_index];

    if (!sr_hii_snapshot_build(target, records, record_count)) return 0;

    if (!session->has_snapshot) {
        session->active_snapshot = next_index;
        session->has_snapshot = 1u;
        sr_nav_init(&session->nav, target->items, target->count, session->page_size);
        if (announce_change) (void)sr_session_emit_focus(session);
        return 1;
    }

    session->active_snapshot = next_index;
    (void)sr_nav_rebind_by_id(&session->nav, target->items, target->count, old_id);
    new_item = sr_nav_current(&session->nav);

    semantic_change = !sr_session_item_equal(old_item, new_item);
    if (announce_change && semantic_change) (void)sr_session_emit_focus(session);
    return 1;
}

int sr_session_navigate(
    SrScreenReaderSession *session,
    SrNavCommand command,
    char first_letter
) {
    if (!session) return 0;
    if (!sr_nav_move(&session->nav, command, first_letter)) return 0;
    (void)sr_session_emit_focus(session);
    return 1;
}

int sr_session_repeat_focus(SrScreenReaderSession *session) {
    return sr_session_emit_focus(session);
}

int sr_session_chooser_open(SrScreenReaderSession *session) {
    if (!session || !sr_chooser_open(&session->nav)) return 0;
    return sr_session_emit_chooser(session);
}

int sr_session_chooser_type(SrScreenReaderSession *session, char ch) {
    if (!session || !sr_chooser_type(&session->nav, ch)) return 0;
    if (session->nav.chooser_match_count == 0u) return 1;
    return sr_session_emit_chooser(session);
}

int sr_session_chooser_backspace(SrScreenReaderSession *session) {
    if (!session || !sr_chooser_backspace(&session->nav)) return 0;
    if (session->nav.chooser_match_count == 0u) return 1;
    return sr_session_emit_chooser(session);
}

int sr_session_chooser_next(SrScreenReaderSession *session) {
    if (!session || !sr_chooser_next(&session->nav)) return 0;
    return sr_session_emit_chooser(session);
}

int sr_session_chooser_previous(SrScreenReaderSession *session) {
    if (!session || !sr_chooser_previous(&session->nav)) return 0;
    return sr_session_emit_chooser(session);
}

int sr_session_chooser_select(SrScreenReaderSession *session) {
    if (!session || !sr_chooser_select(&session->nav)) return 0;
    return sr_session_emit_focus(session);
}

int sr_session_chooser_cancel(SrScreenReaderSession *session) {
    if (!session || !session->nav.chooser_open) return 0;
    sr_chooser_cancel(&session->nav);
    return sr_session_emit_focus(session);
}

int sr_session_speak_hint(SrScreenReaderSession *session) {
    const SrItem *item;
    SrSpeechEvent event;

    if (!session) return 0;
    item = sr_nav_current(&session->nav);
    if (!item) return 0;

    event.key = item->id;
    event.priority = SR_SPEECH_HINT;
    event.interruptible = 1u;
    if (sr_format_hint(&session->nav, event.text, sizeof(event.text)) == 0u) return 0;
    (void)sr_speech_submit(&session->speech, &event);
    return 1;
}

const SrItem *sr_session_current(const SrScreenReaderSession *session) {
    if (!session) return NULL;
    return sr_nav_current(&session->nav);
}

int sr_session_speech_complete(SrScreenReaderSession *session, SrSpeechEvent *next) {
    if (!session) return 0;
    return sr_speech_complete(&session->speech, next);
}
