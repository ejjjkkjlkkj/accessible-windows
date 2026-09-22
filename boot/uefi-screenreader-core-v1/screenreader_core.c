#include "screenreader_core.h"

static char sr_ascii_lower(char c) {
    if (c >= 'A' && c <= 'Z') return (char)(c - 'A' + 'a');
    return c;
}

static int sr_text_equal(const char *a, const char *b) {
    size_t i = 0;
    if (!a || !b) return a == b;
    for (;;) {
        if (a[i] != b[i]) return 0;
        if (a[i] == '\0') return 1;
        ++i;
    }
}

static int sr_contains_ascii_ci(const char *text, const char *query) {
    size_t i, j;
    if (!query || query[0] == '\0') return 1;
    if (!text) return 0;
    for (i = 0; text[i] != '\0'; ++i) {
        for (j = 0; query[j] != '\0'; ++j) {
            if (text[i + j] == '\0') return 0;
            if (sr_ascii_lower(text[i + j]) != sr_ascii_lower(query[j])) break;
        }
        if (query[j] == '\0') return 1;
    }
    return 0;
}

static size_t sr_copy_text(char *out, size_t cap, size_t pos, const char *text) {
    size_t i = 0;
    if (!out || cap == 0 || !text) return pos;
    while (text[i] != '\0' && pos + 1 < cap) {
        out[pos++] = text[i++];
    }
    out[pos < cap ? pos : cap - 1] = '\0';
    return pos;
}

static size_t sr_append_sep(char *out, size_t cap, size_t pos) {
    if (pos == 0) return pos;
    return sr_copy_text(out, cap, pos, ", ");
}

static size_t sr_append_u32(char *out, size_t cap, size_t pos, uint32_t value) {
    char buf[11];
    size_t n = 0, i;
    if (value == 0) return sr_copy_text(out, cap, pos, "0");
    while (value && n < sizeof(buf)) {
        buf[n++] = (char)('0' + (value % 10u));
        value /= 10u;
    }
    for (i = 0; i < n; ++i) {
        char tmp[2] = { buf[n - 1 - i], '\0' };
        pos = sr_copy_text(out, cap, pos, tmp);
    }
    return pos;
}

int sr_item_is_focusable(const SrItem *item) {
    return item && item->role != SR_ROLE_SEPARATOR;
}

const char *sr_role_name(SrRole role) {
    switch (role) {
        case SR_ROLE_MENU: return "menu";
        case SR_ROLE_ACTION: return "button";
        case SR_ROLE_TOGGLE: return "toggle";
        case SR_ROLE_CHOICE: return "choice";
        case SR_ROLE_TEXT: return "text";
        case SR_ROLE_NUMERIC: return "number";
        case SR_ROLE_PASSWORD: return "password";
        case SR_ROLE_SUBMENU: return "submenu";
        case SR_ROLE_DIALOG: return "dialog";
        case SR_ROLE_ALERT: return "alert";
        case SR_ROLE_SEPARATOR: return "separator";
        default: return "item";
    }
}

static size_t sr_find_first_focusable(const SrItem *items, size_t count) {
    size_t i;
    for (i = 0; i < count; ++i) {
        if (sr_item_is_focusable(&items[i])) return i;
    }
    return SR_NO_INDEX;
}

static size_t sr_find_last_focusable(const SrItem *items, size_t count) {
    size_t i = count;
    while (i > 0) {
        --i;
        if (sr_item_is_focusable(&items[i])) return i;
    }
    return SR_NO_INDEX;
}

void sr_nav_init(SrNavigator *nav, const SrItem *items, size_t count, size_t page_size) {
    size_t i;
    if (!nav) return;
    nav->items = items;
    nav->count = count;
    nav->focus = items ? sr_find_first_focusable(items, count) : SR_NO_INDEX;
    nav->page_size = page_size ? page_size : 5u;
    nav->chooser_open = 0;
    nav->chooser_query_len = 0;
    nav->chooser_query[0] = '\0';
    nav->chooser_match_count = 0;
    nav->chooser_cursor = 0;
    for (i = 0; i < SR_MAX_MATCHES; ++i) nav->chooser_matches[i] = SR_NO_INDEX;
}

int sr_nav_rebind_by_id(SrNavigator *nav, const SrItem *items, size_t count, uint32_t preferred_id) {
    size_t i;
    size_t page_size;

    if (!nav) return 0;
    page_size = nav->page_size ? nav->page_size : 5u;
    sr_nav_init(nav, items, count, page_size);

    if (!items || count == 0 || preferred_id == 0u) return 0;
    for (i = 0; i < count; ++i) {
        if (items[i].id == preferred_id && sr_item_is_focusable(&items[i])) {
            nav->focus = i;
            return 1;
        }
    }
    return 0;
}

const SrItem *sr_nav_current(const SrNavigator *nav) {
    if (!nav || !nav->items || nav->focus == SR_NO_INDEX || nav->focus >= nav->count) return NULL;
    return &nav->items[nav->focus];
}

static size_t sr_step_focusable(const SrNavigator *nav, size_t start, int direction) {
    size_t i = start;
    size_t seen = 0;
    if (!nav || nav->count == 0 || start == SR_NO_INDEX) return SR_NO_INDEX;
    while (seen < nav->count) {
        if (direction > 0) i = (i + 1u) % nav->count;
        else i = (i == 0u) ? nav->count - 1u : i - 1u;
        if (sr_item_is_focusable(&nav->items[i])) return i;
        ++seen;
    }
    return start;
}

static size_t sr_role_step(const SrNavigator *nav, size_t start, int direction) {
    size_t i = start;
    size_t seen = 0;
    SrRole role;
    if (!nav || start == SR_NO_INDEX || start >= nav->count) return SR_NO_INDEX;
    role = nav->items[start].role;
    while (seen < nav->count) {
        i = sr_step_focusable(nav, i, direction);
        if (i == SR_NO_INDEX) return start;
        if (nav->items[i].role == role) return i;
        ++seen;
    }
    return start;
}

static size_t sr_first_letter_step(const SrNavigator *nav, size_t start, char first_letter) {
    size_t i = start;
    size_t seen = 0;
    char target = sr_ascii_lower(first_letter);
    if (target < 'a' || target > 'z') return start;
    while (seen < nav->count) {
        const char *label;
        i = sr_step_focusable(nav, i, 1);
        if (i == SR_NO_INDEX) return start;
        label = nav->items[i].label;
        if (label && sr_ascii_lower(label[0]) == target) return i;
        ++seen;
    }
    return start;
}

int sr_nav_move(SrNavigator *nav, SrNavCommand command, char first_letter) {
    size_t old, next, steps;
    if (!nav || nav->focus == SR_NO_INDEX || nav->count == 0) return 0;
    old = nav->focus;
    next = old;
    switch (command) {
        case SR_NAV_NEXT:
            next = sr_step_focusable(nav, old, 1);
            break;
        case SR_NAV_PREVIOUS:
            next = sr_step_focusable(nav, old, -1);
            break;
        case SR_NAV_HOME:
            next = sr_find_first_focusable(nav->items, nav->count);
            break;
        case SR_NAV_END:
            next = sr_find_last_focusable(nav->items, nav->count);
            break;
        case SR_NAV_PAGE_NEXT:
            next = old;
            for (steps = 0; steps < nav->page_size; ++steps) next = sr_step_focusable(nav, next, 1);
            break;
        case SR_NAV_PAGE_PREVIOUS:
            next = old;
            for (steps = 0; steps < nav->page_size; ++steps) next = sr_step_focusable(nav, next, -1);
            break;
        case SR_NAV_FIRST_LETTER:
            next = sr_first_letter_step(nav, old, first_letter);
            break;
        case SR_NAV_ROLE_NEXT:
            next = sr_role_step(nav, old, 1);
            break;
        case SR_NAV_ROLE_PREVIOUS:
            next = sr_role_step(nav, old, -1);
            break;
        default:
            break;
    }
    if (next == SR_NO_INDEX) next = old;
    nav->focus = next;
    return nav->focus != old;
}

static void sr_chooser_rebuild(SrNavigator *nav) {
    size_t i;
    nav->chooser_match_count = 0;
    nav->chooser_cursor = 0;
    for (i = 0; i < nav->count && nav->chooser_match_count < SR_MAX_MATCHES; ++i) {
        const SrItem *item = &nav->items[i];
        if (!sr_item_is_focusable(item)) continue;
        if (sr_contains_ascii_ci(item->label, nav->chooser_query) ||
            sr_contains_ascii_ci(item->value, nav->chooser_query)) {
            nav->chooser_matches[nav->chooser_match_count++] = i;
        }
    }
}

int sr_chooser_open(SrNavigator *nav) {
    size_t i;
    if (!nav || !nav->items || nav->count == 0) return 0;
    nav->chooser_open = 1;
    nav->chooser_query_len = 0;
    nav->chooser_query[0] = '\0';
    sr_chooser_rebuild(nav);
    for (i = 0; i < nav->chooser_match_count; ++i) {
        if (nav->chooser_matches[i] == nav->focus) {
            nav->chooser_cursor = i;
            break;
        }
    }
    return nav->chooser_match_count > 0;
}

int sr_chooser_type(SrNavigator *nav, char ch) {
    if (!nav || !nav->chooser_open) return 0;
    if (ch < 32 || ch > 126) return 0;
    if (nav->chooser_query_len + 1 >= SR_MAX_QUERY) return 0;
    nav->chooser_query[nav->chooser_query_len++] = ch;
    nav->chooser_query[nav->chooser_query_len] = '\0';
    sr_chooser_rebuild(nav);
    return 1;
}

int sr_chooser_backspace(SrNavigator *nav) {
    if (!nav || !nav->chooser_open || nav->chooser_query_len == 0) return 0;
    --nav->chooser_query_len;
    nav->chooser_query[nav->chooser_query_len] = '\0';
    sr_chooser_rebuild(nav);
    return 1;
}

int sr_chooser_next(SrNavigator *nav) {
    if (!nav || !nav->chooser_open || nav->chooser_match_count == 0) return 0;
    nav->chooser_cursor = (nav->chooser_cursor + 1u) % nav->chooser_match_count;
    return 1;
}

int sr_chooser_previous(SrNavigator *nav) {
    if (!nav || !nav->chooser_open || nav->chooser_match_count == 0) return 0;
    nav->chooser_cursor = nav->chooser_cursor == 0 ? nav->chooser_match_count - 1u : nav->chooser_cursor - 1u;
    return 1;
}

int sr_chooser_select(SrNavigator *nav) {
    size_t selected;
    if (!nav || !nav->chooser_open || nav->chooser_match_count == 0) return 0;
    selected = nav->chooser_matches[nav->chooser_cursor];
    if (selected >= nav->count || !sr_item_is_focusable(&nav->items[selected])) return 0;
    nav->focus = selected;
    nav->chooser_open = 0;
    nav->chooser_query_len = 0;
    nav->chooser_query[0] = '\0';
    return 1;
}

void sr_chooser_cancel(SrNavigator *nav) {
    if (!nav) return;
    nav->chooser_open = 0;
    nav->chooser_query_len = 0;
    nav->chooser_query[0] = '\0';
    nav->chooser_match_count = 0;
    nav->chooser_cursor = 0;
}

size_t sr_format_focus(const SrNavigator *nav, char *out, size_t cap) {
    const SrItem *item;
    size_t pos = 0;
    size_t ordinal = 0;
    size_t total = 0;
    size_t i;
    if (!out || cap == 0) return 0;
    out[0] = '\0';
    item = sr_nav_current(nav);
    if (!item) return 0;

    if (item->label && item->label[0]) pos = sr_copy_text(out, cap, pos, item->label);
    if (item->value && item->value[0] && item->role != SR_ROLE_PASSWORD) {
        pos = sr_append_sep(out, cap, pos);
        pos = sr_copy_text(out, cap, pos, item->value);
    }
    pos = sr_append_sep(out, cap, pos);
    pos = sr_copy_text(out, cap, pos, sr_role_name(item->role));

    if (item->state & SR_STATE_CHECKED) {
        pos = sr_append_sep(out, cap, pos);
        pos = sr_copy_text(out, cap, pos, "checked");
    }
    if (item->state & SR_STATE_SELECTED) {
        pos = sr_append_sep(out, cap, pos);
        pos = sr_copy_text(out, cap, pos, "selected");
    }
    if (item->state & SR_STATE_EXPANDED) {
        pos = sr_append_sep(out, cap, pos);
        pos = sr_copy_text(out, cap, pos, "expanded");
    }
    if (item->state & SR_STATE_CHANGED) {
        pos = sr_append_sep(out, cap, pos);
        pos = sr_copy_text(out, cap, pos, "changed");
    }
    if (item->state & SR_STATE_DISABLED) {
        pos = sr_append_sep(out, cap, pos);
        pos = sr_copy_text(out, cap, pos, "disabled");
    }
    if (item->state & SR_STATE_DANGER) {
        pos = sr_append_sep(out, cap, pos);
        pos = sr_copy_text(out, cap, pos, "warning");
    }

    for (i = 0; i < nav->count; ++i) {
        if (!sr_item_is_focusable(&nav->items[i])) continue;
        ++total;
        if (i <= nav->focus) ++ordinal;
    }
    if (total > 1) {
        pos = sr_append_sep(out, cap, pos);
        pos = sr_copy_text(out, cap, pos, "item ");
        pos = sr_append_u32(out, cap, pos, (uint32_t)ordinal);
        pos = sr_copy_text(out, cap, pos, " of ");
        pos = sr_append_u32(out, cap, pos, (uint32_t)total);
    }
    return pos;
}

size_t sr_format_hint(const SrNavigator *nav, char *out, size_t cap) {
    const SrItem *item;
    if (!out || cap == 0) return 0;
    out[0] = '\0';
    item = sr_nav_current(nav);
    if (!item || !item->hint || !item->hint[0]) return 0;
    return sr_copy_text(out, cap, 0, item->hint);
}

size_t sr_format_alert(const char *title, const char *detail, char *out, size_t cap) {
    size_t pos = 0;
    if (!out || cap == 0) return 0;
    out[0] = '\0';
    pos = sr_copy_text(out, cap, pos, "Alert");
    if (title && title[0]) {
        pos = sr_append_sep(out, cap, pos);
        pos = sr_copy_text(out, cap, pos, title);
    }
    if (detail && detail[0]) {
        pos = sr_append_sep(out, cap, pos);
        pos = sr_copy_text(out, cap, pos, detail);
    }
    return pos;
}

void sr_speech_init(SrSpeechScheduler *scheduler) {
    size_t i;
    if (!scheduler) return;
    scheduler->active = 0;
    scheduler->pending_count = 0;
    scheduler->current.key = 0;
    scheduler->current.text[0] = '\0';
    for (i = 0; i < SR_QUEUE_CAPACITY; ++i) scheduler->pending[i].text[0] = '\0';
}

static void sr_event_copy(SrSpeechEvent *dst, const SrSpeechEvent *src) {
    size_t i = 0;
    dst->key = src->key;
    dst->priority = src->priority;
    dst->interruptible = src->interruptible;
    while (i + 1 < SR_MAX_SPEECH_TEXT && src->text[i] != '\0') {
        dst->text[i] = src->text[i];
        ++i;
    }
    dst->text[i] = '\0';
}

static int sr_event_same(const SrSpeechEvent *a, const SrSpeechEvent *b) {
    return a->key == b->key && a->priority == b->priority && sr_text_equal(a->text, b->text);
}

static void sr_queue_remove(SrSpeechScheduler *scheduler, size_t index) {
    size_t i;
    if (!scheduler || index >= scheduler->pending_count) return;
    for (i = index + 1; i < scheduler->pending_count; ++i) {
        sr_event_copy(&scheduler->pending[i - 1], &scheduler->pending[i]);
    }
    --scheduler->pending_count;
}

static void sr_queue_drop_hints(SrSpeechScheduler *scheduler) {
    size_t i = 0;
    if (!scheduler) return;
    while (i < scheduler->pending_count) {
        if (scheduler->pending[i].priority == SR_SPEECH_HINT) {
            sr_queue_remove(scheduler, i);
        } else {
            ++i;
        }
    }
}

SrSpeechDecision sr_speech_submit(SrSpeechScheduler *scheduler, const SrSpeechEvent *event) {
    size_t i;
    size_t lowest = 0;
    if (!scheduler || !event || event->text[0] == '\0') return SR_SPEECH_DROP_DUPLICATE;

    if (event->priority >= SR_SPEECH_FOCUS) sr_queue_drop_hints(scheduler);

    if (!scheduler->active) {
        sr_event_copy(&scheduler->current, event);
        scheduler->active = 1;
        return SR_SPEECH_START;
    }

    if (sr_event_same(&scheduler->current, event)) return SR_SPEECH_DROP_DUPLICATE;
    for (i = 0; i < scheduler->pending_count; ++i) {
        if (sr_event_same(&scheduler->pending[i], event)) return SR_SPEECH_DROP_DUPLICATE;
    }

    if (scheduler->current.interruptible &&
        (event->priority > scheduler->current.priority ||
         (event->key == scheduler->current.key &&
          event->priority >= scheduler->current.priority))) {
        sr_event_copy(&scheduler->current, event);
        return SR_SPEECH_PREEMPT;
    }

    for (i = 0; i < scheduler->pending_count; ++i) {
        if (scheduler->pending[i].key == event->key) {
            sr_event_copy(&scheduler->pending[i], event);
            return SR_SPEECH_REPLACE_PENDING;
        }
    }

    if (scheduler->pending_count < SR_QUEUE_CAPACITY) {
        sr_event_copy(&scheduler->pending[scheduler->pending_count++], event);
        return SR_SPEECH_QUEUE;
    }

    for (i = 1; i < scheduler->pending_count; ++i) {
        if (scheduler->pending[i].priority < scheduler->pending[lowest].priority) lowest = i;
    }
    if (event->priority > scheduler->pending[lowest].priority) {
        sr_event_copy(&scheduler->pending[lowest], event);
        return SR_SPEECH_REPLACE_PENDING;
    }
    return SR_SPEECH_DROP_DUPLICATE;
}

int sr_speech_complete(SrSpeechScheduler *scheduler, SrSpeechEvent *next) {
    size_t i, best;
    if (!scheduler || !scheduler->active) return 0;
    if (scheduler->pending_count == 0) {
        scheduler->active = 0;
        scheduler->current.text[0] = '\0';
        return 0;
    }

    best = 0;
    for (i = 1; i < scheduler->pending_count; ++i) {
        if (scheduler->pending[i].priority > scheduler->pending[best].priority) best = i;
    }
    sr_event_copy(&scheduler->current, &scheduler->pending[best]);
    if (next) sr_event_copy(next, &scheduler->current);
    sr_queue_remove(scheduler, best);
    return 1;
}

void sr_speech_cancel_all(SrSpeechScheduler *scheduler) {
    if (!scheduler) return;
    scheduler->active = 0;
    scheduler->pending_count = 0;
    scheduler->current.text[0] = '\0';
}
