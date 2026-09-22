#include "screenreader_core.h"

#include <assert.h>
#include <stdio.h>
#include <string.h>

static const SrItem items[] = {
    {1, SR_ROLE_MENU, 0, "Main", "", "Firmware settings"},
    {2, SR_ROLE_SEPARATOR, 0, "---", "", ""},
    {3, SR_ROLE_TOGGLE, SR_STATE_CHECKED, "Secure Boot", "Enabled", "Press Enter to toggle"},
    {4, SR_ROLE_CHOICE, SR_STATE_CHANGED, "Boot mode", "UEFI", "Use Left or Right to change"},
    {5, SR_ROLE_SUBMENU, 0, "Advanced", "", "Press Enter to open"},
    {6, SR_ROLE_TOGGLE, SR_STATE_DISABLED, "Legacy USB", "Disabled", ""},
    {7, SR_ROLE_PASSWORD, 0, "Supervisor password", "secret-must-not-speak", "Press Enter to edit"},
    {8, SR_ROLE_ACTION, SR_STATE_DANGER, "Restore defaults", "", "Press Enter to activate"}
};

static SrSpeechEvent event(uint32_t key, SrSpeechPriority p, int interruptible, const char *text) {
    SrSpeechEvent e;
    size_t n = strlen(text);
    assert(n < sizeof(e.text));
    e.key = key;
    e.priority = p;
    e.interruptible = (uint8_t)interruptible;
    memcpy(e.text, text, n + 1);
    return e;
}

static void test_navigation(void) {
    SrNavigator nav;
    sr_nav_init(&nav, items, sizeof(items) / sizeof(items[0]), 3);
    assert(nav.focus == 0);

    assert(sr_nav_move(&nav, SR_NAV_NEXT, 0));
    assert(nav.focus == 2);
    assert(sr_nav_current(&nav)->id == 3);

    assert(sr_nav_move(&nav, SR_NAV_PREVIOUS, 0));
    assert(nav.focus == 0);

    assert(sr_nav_move(&nav, SR_NAV_END, 0));
    assert(sr_nav_current(&nav)->id == 8);

    assert(sr_nav_move(&nav, SR_NAV_HOME, 0));
    assert(sr_nav_current(&nav)->id == 1);

    assert(sr_nav_move(&nav, SR_NAV_PAGE_NEXT, 0));
    assert(sr_nav_current(&nav)->id == 5);

    assert(sr_nav_move(&nav, SR_NAV_FIRST_LETTER, 's'));
    assert(sr_nav_current(&nav)->id == 7);

    assert(!sr_nav_move(&nav, SR_NAV_FIRST_LETTER, 'z'));
    assert(sr_nav_current(&nav)->id == 7);
}

static void test_rotor(void) {
    static const SrItem rotor_items[] = {
        {10, SR_ROLE_TOGGLE, 0, "A", "", ""},
        {11, SR_ROLE_ACTION, 0, "B", "", ""},
        {12, SR_ROLE_TOGGLE, 0, "C", "", ""},
        {13, SR_ROLE_ACTION, 0, "D", "", ""}
    };
    SrNavigator nav;
    sr_nav_init(&nav, rotor_items, 4, 2);
    assert(nav.focus == 0);
    assert(sr_nav_move(&nav, SR_NAV_ROLE_NEXT, 0));
    assert(nav.focus == 2);
    assert(sr_nav_move(&nav, SR_NAV_ROLE_PREVIOUS, 0));
    assert(nav.focus == 0);
}

static void test_item_chooser(void) {
    SrNavigator nav;
    sr_nav_init(&nav, items, sizeof(items) / sizeof(items[0]), 3);

    assert(sr_chooser_open(&nav));
    assert(nav.chooser_open);
    assert(nav.chooser_match_count == 7);

    assert(sr_chooser_type(&nav, 'm'));
    assert(sr_chooser_type(&nav, 'o'));
    assert(sr_chooser_type(&nav, 'd'));
    assert(sr_chooser_type(&nav, 'e'));
    assert(nav.chooser_match_count == 1);
    assert(sr_chooser_select(&nav));
    assert(!nav.chooser_open);
    assert(sr_nav_current(&nav)->id == 4);

    assert(sr_chooser_open(&nav));
    assert(sr_chooser_type(&nav, 'u'));
    assert(nav.chooser_match_count >= 1);
    assert(sr_chooser_backspace(&nav));
    assert(nav.chooser_query_len == 0);
    assert(sr_chooser_next(&nav));
    assert(sr_chooser_previous(&nav));
    sr_chooser_cancel(&nav);
    assert(!nav.chooser_open);
}

static void test_focus_speech(void) {
    SrNavigator nav;
    char out[SR_MAX_SPEECH_TEXT];

    sr_nav_init(&nav, items, sizeof(items) / sizeof(items[0]), 3);
    assert(sr_nav_move(&nav, SR_NAV_NEXT, 0));
    sr_format_focus(&nav, out, sizeof(out));
    assert(strstr(out, "Secure Boot") != NULL);
    assert(strstr(out, "Enabled") != NULL);
    assert(strstr(out, "toggle") != NULL);
    assert(strstr(out, "checked") != NULL);
    assert(strstr(out, "item 2 of 7") != NULL);

    while (sr_nav_current(&nav)->id != 7) {
        sr_nav_move(&nav, SR_NAV_NEXT, 0);
    }
    sr_format_focus(&nav, out, sizeof(out));
    assert(strstr(out, "Supervisor password") != NULL);
    assert(strstr(out, "password") != NULL);
    assert(strstr(out, "secret-must-not-speak") == NULL);

    while (sr_nav_current(&nav)->id != 8) {
        sr_nav_move(&nav, SR_NAV_NEXT, 0);
    }
    sr_format_focus(&nav, out, sizeof(out));
    assert(strstr(out, "warning") != NULL);

    sr_format_alert("Unsaved changes", "Save before reboot", out, sizeof(out));
    assert(strcmp(out, "Alert, Unsaved changes, Save before reboot") == 0);
}

static void test_scheduler(void) {
    SrSpeechScheduler s;
    SrSpeechEvent focus1 = event(100, SR_SPEECH_FOCUS, 1, "Secure Boot, Enabled");
    SrSpeechEvent focus2 = event(100, SR_SPEECH_FOCUS, 1, "Boot mode, UEFI");
    SrSpeechEvent hint = event(200, SR_SPEECH_HINT, 1, "Press Enter");
    SrSpeechEvent dialog = event(300, SR_SPEECH_DIALOG, 1, "Save changes dialog");
    SrSpeechEvent critical = event(400, SR_SPEECH_CRITICAL, 0, "Critical firmware error");
    SrSpeechEvent next;

    sr_speech_init(&s);
    assert(sr_speech_submit(&s, &focus1) == SR_SPEECH_START);
    assert(sr_speech_submit(&s, &focus1) == SR_SPEECH_DROP_DUPLICATE);
    assert(sr_speech_submit(&s, &hint) == SR_SPEECH_QUEUE);
    assert(sr_speech_submit(&s, &focus2) == SR_SPEECH_PREEMPT);
    assert(strcmp(s.current.text, "Boot mode, UEFI") == 0);
    assert(sr_speech_submit(&s, &dialog) == SR_SPEECH_PREEMPT);
    assert(strcmp(s.current.text, "Save changes dialog") == 0);

    assert(sr_speech_submit(&s, &critical) == SR_SPEECH_PREEMPT);
    assert(strcmp(s.current.text, "Critical firmware error") == 0);
    assert(!s.current.interruptible);

    assert(sr_speech_complete(&s, &next));
    assert(next.priority == SR_SPEECH_HINT);
    assert(strcmp(next.text, "Press Enter") == 0);

    sr_speech_cancel_all(&s);
    assert(!s.active);
    assert(s.pending_count == 0);
}

static void test_bounded_output(void) {
    SrNavigator nav;
    char tiny[24];
    sr_nav_init(&nav, items, sizeof(items) / sizeof(items[0]), 3);
    sr_format_focus(&nav, tiny, sizeof(tiny));
    assert(tiny[sizeof(tiny) - 1] == '\0');
}

static void test_stress(void) {
    SrNavigator nav;
    SrSpeechScheduler scheduler;
    uint32_t x = 0x51A7E55u;
    size_t i;

    sr_nav_init(&nav, items, sizeof(items) / sizeof(items[0]), 3);
    sr_speech_init(&scheduler);

    for (i = 0; i < 50000u; ++i) {
        SrSpeechEvent e;
        char spoken[SR_MAX_SPEECH_TEXT];
        size_t j;

        x = x * 1664525u + 1013904223u;
        (void)sr_nav_move(
            &nav,
            (SrNavCommand)(x % 9u),
            (char)('a' + ((x >> 8) % 26u))
        );

        if ((x & 31u) == 0u) (void)sr_chooser_open(&nav);
        if (nav.chooser_open) {
            switch ((x >> 5) % 6u) {
                case 0: (void)sr_chooser_type(&nav, (char)('a' + ((x >> 12) % 26u))); break;
                case 1: (void)sr_chooser_backspace(&nav); break;
                case 2: (void)sr_chooser_next(&nav); break;
                case 3: (void)sr_chooser_previous(&nav); break;
                case 4: (void)sr_chooser_select(&nav); break;
                default: sr_chooser_cancel(&nav); break;
            }
        }

        if (nav.focus != SR_NO_INDEX) assert(nav.focus < nav.count);
        assert(nav.chooser_query_len < SR_MAX_QUERY);
        assert(nav.chooser_match_count <= SR_MAX_MATCHES);
        assert(nav.chooser_match_count <= nav.count);
        for (j = 0; j < nav.chooser_match_count; ++j) {
            assert(nav.chooser_matches[j] < nav.count);
        }

        sr_format_focus(&nav, spoken, sizeof(spoken));
        assert(spoken[sizeof(spoken) - 1] == '\0' || strlen(spoken) < sizeof(spoken));

        e = event(
            (uint32_t)(x % 4u),
            (SrSpeechPriority)(((x >> 16) & 1u) ? SR_SPEECH_FOCUS : SR_SPEECH_HINT),
            1,
            ((x >> 17) & 1u) ? "focus update" : "hint update"
        );
        (void)sr_speech_submit(&scheduler, &e);
        if ((x & 7u) == 0u) (void)sr_speech_complete(&scheduler, NULL);
        assert(scheduler.pending_count <= SR_QUEUE_CAPACITY);
        if (scheduler.active) {
            assert(scheduler.current.text[SR_MAX_SPEECH_TEXT - 1] == '\0' ||
                   strlen(scheduler.current.text) < SR_MAX_SPEECH_TEXT);
        }
    }

    sr_speech_cancel_all(&scheduler);
}

int main(void) {
    test_navigation();
    test_rotor();
    test_item_chooser();
    test_focus_speech();
    test_scheduler();
    test_bounded_output();
    test_stress();
    puts("UEFI_SCREENREADER_CORE_TESTS=PASS");
    return 0;
}
