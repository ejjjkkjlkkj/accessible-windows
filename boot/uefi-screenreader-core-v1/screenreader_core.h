#ifndef QEVARYNOX_UEFI_SCREENREADER_CORE_H
#define QEVARYNOX_UEFI_SCREENREADER_CORE_H

#include <stddef.h>
#include <stdint.h>

#define SR_MAX_SPEECH_TEXT 384u
#define SR_MAX_QUERY 32u
#define SR_MAX_MATCHES 128u
#define SR_QUEUE_CAPACITY 8u
#define SR_NO_INDEX ((size_t)-1)

typedef enum {
    SR_ROLE_UNKNOWN = 0,
    SR_ROLE_MENU,
    SR_ROLE_ACTION,
    SR_ROLE_TOGGLE,
    SR_ROLE_CHOICE,
    SR_ROLE_TEXT,
    SR_ROLE_NUMERIC,
    SR_ROLE_PASSWORD,
    SR_ROLE_SUBMENU,
    SR_ROLE_DIALOG,
    SR_ROLE_ALERT,
    SR_ROLE_SEPARATOR,
    SR_ROLE_DATE,
    SR_ROLE_TIME,
    SR_ROLE_EDIT,
    SR_ROLE_ORDERED_LIST
} SrRole;

enum {
    SR_STATE_NONE     = 0u,
    SR_STATE_DISABLED = 1u << 0,
    SR_STATE_CHECKED  = 1u << 1,
    SR_STATE_SELECTED = 1u << 2,
    SR_STATE_EXPANDED = 1u << 3,
    SR_STATE_CHANGED  = 1u << 4,
    SR_STATE_DANGER   = 1u << 5
};

typedef struct {
    uint32_t id;
    SrRole role;
    uint32_t state;
    const char *label;
    const char *value;
    const char *hint;
} SrItem;

typedef enum {
    SR_NAV_NEXT = 0,
    SR_NAV_PREVIOUS,
    SR_NAV_HOME,
    SR_NAV_END,
    SR_NAV_PAGE_NEXT,
    SR_NAV_PAGE_PREVIOUS,
    SR_NAV_FIRST_LETTER,
    SR_NAV_ROLE_NEXT,
    SR_NAV_ROLE_PREVIOUS
} SrNavCommand;

typedef struct {
    const SrItem *items;
    size_t count;
    size_t focus;
    size_t page_size;
    int chooser_open;
    char chooser_query[SR_MAX_QUERY];
    size_t chooser_query_len;
    size_t chooser_matches[SR_MAX_MATCHES];
    size_t chooser_match_count;
    size_t chooser_cursor;
} SrNavigator;

typedef enum {
    SR_SPEECH_HINT = 10,
    SR_SPEECH_STATUS = 20,
    SR_SPEECH_FOCUS = 40,
    SR_SPEECH_VALUE = 50,
    SR_SPEECH_DIALOG = 80,
    SR_SPEECH_CRITICAL = 100
} SrSpeechPriority;

typedef enum {
    SR_SPEECH_START = 0,
    SR_SPEECH_QUEUE,
    SR_SPEECH_REPLACE_PENDING,
    SR_SPEECH_PREEMPT,
    SR_SPEECH_DROP_DUPLICATE
} SrSpeechDecision;

typedef struct {
    uint32_t key;
    SrSpeechPriority priority;
    uint8_t interruptible;
    char text[SR_MAX_SPEECH_TEXT];
} SrSpeechEvent;

typedef struct {
    uint8_t active;
    SrSpeechEvent current;
    SrSpeechEvent pending[SR_QUEUE_CAPACITY];
    size_t pending_count;
} SrSpeechScheduler;

void sr_nav_init(SrNavigator *nav, const SrItem *items, size_t count, size_t page_size);
int sr_nav_rebind_by_id(SrNavigator *nav, const SrItem *items, size_t count, uint32_t preferred_id);
int sr_nav_move(SrNavigator *nav, SrNavCommand command, char first_letter);
const SrItem *sr_nav_current(const SrNavigator *nav);

int sr_chooser_open(SrNavigator *nav);
int sr_chooser_type(SrNavigator *nav, char ch);
int sr_chooser_backspace(SrNavigator *nav);
int sr_chooser_next(SrNavigator *nav);
int sr_chooser_previous(SrNavigator *nav);
int sr_chooser_select(SrNavigator *nav);
void sr_chooser_cancel(SrNavigator *nav);

int sr_item_is_focusable(const SrItem *item);
const char *sr_role_name(SrRole role);

size_t sr_format_focus(const SrNavigator *nav, char *out, size_t cap);
size_t sr_format_hint(const SrNavigator *nav, char *out, size_t cap);
size_t sr_format_alert(const char *title, const char *detail, char *out, size_t cap);

void sr_speech_init(SrSpeechScheduler *scheduler);
SrSpeechDecision sr_speech_submit(SrSpeechScheduler *scheduler, const SrSpeechEvent *event);
int sr_speech_complete(SrSpeechScheduler *scheduler, SrSpeechEvent *next);
void sr_speech_cancel_all(SrSpeechScheduler *scheduler);

#endif
