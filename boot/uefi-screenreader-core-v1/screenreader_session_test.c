#include "screenreader_session.h"

#include <assert.h>
#include <stdio.h>
#include <string.h>

static const SrHiiRecord initial_records[] = {
    {10, SR_HII_OP_ONE_OF, SR_HII_FLAG_NONE, "Boot mode", "UEFI", "Use Left or Right"},
    {20, SR_HII_OP_CHECKBOX, SR_HII_FLAG_CHECKED, "Secure Boot", "Enabled", "Press Enter to toggle"},
    {30, SR_HII_OP_REF, SR_HII_FLAG_NONE, "Advanced", "", "Press Enter to open"}
};

static void test_initial_and_refresh(void) {
    SrScreenReaderSession session;
    SrSpeechEvent next;
    SrHiiRecord refreshed[3];

    sr_session_init(&session, 3u);
    assert(sr_session_apply_hii(&session, initial_records, 3u, 1));
    assert(sr_session_current(&session)->id == 10u);
    assert(session.speech.active);
    assert(strstr(session.speech.current.text, "Boot mode") != NULL);
    assert(!sr_session_speech_complete(&session, &next));
    assert(!session.speech.active);

    assert(sr_session_apply_hii(&session, initial_records, 3u, 1));
    assert(!session.speech.active);

    memcpy(refreshed, initial_records, sizeof(refreshed));
    refreshed[0].value = "Legacy";
    assert(sr_session_apply_hii(&session, refreshed, 3u, 1));
    assert(session.speech.active);
    assert(strstr(session.speech.current.text, "Legacy") != NULL);
    sr_speech_cancel_all(&session.speech);

    refreshed[0].flags = SR_HII_FLAG_SUPPRESSED;
    assert(sr_session_apply_hii(&session, refreshed, 3u, 1));
    assert(sr_session_current(&session)->id == 20u);
    assert(session.speech.active);
    assert(strstr(session.speech.current.text, "Secure Boot") != NULL);
}

static void test_navigation_cancels_stale_hint(void) {
    SrScreenReaderSession session;

    sr_session_init(&session, 3u);
    assert(sr_session_apply_hii(&session, initial_records, 3u, 0));
    assert(sr_session_navigate(&session, SR_NAV_NEXT, 0));
    assert(sr_session_current(&session)->id == 20u);
    assert(session.speech.active);

    assert(sr_session_speak_hint(&session));
    assert(session.speech.pending_count == 1u);
    assert(session.speech.pending[0].priority == SR_SPEECH_HINT);

    assert(sr_session_navigate(&session, SR_NAV_PREVIOUS, 0));
    assert(sr_session_current(&session)->id == 10u);
    assert(session.speech.pending_count == 0u);
    assert(session.speech.current.key == 10u);
    assert(strstr(session.speech.current.text, "Boot mode") != NULL);
}

static void test_atomic_failed_refresh(void) {
    SrScreenReaderSession session;
    SrHiiRecord too_many[SR_HII_MAX_ITEMS + 1u];
    const SrItem *before;
    size_t i;

    sr_session_init(&session, 3u);
    assert(sr_session_apply_hii(&session, initial_records, 3u, 0));
    assert(sr_session_navigate(&session, SR_NAV_NEXT, 0));
    before = sr_session_current(&session);
    assert(before && before->id == 20u);

    for (i = 0; i < SR_HII_MAX_ITEMS + 1u; ++i) {
        too_many[i].id = (uint32_t)(1000u + i);
        too_many[i].opcode = SR_HII_OP_ACTION;
        too_many[i].flags = 0u;
        too_many[i].prompt = "Action";
        too_many[i].value = "";
        too_many[i].help = "";
    }

    assert(!sr_session_apply_hii(&session, too_many, SR_HII_MAX_ITEMS + 1u, 1));
    assert(sr_session_current(&session)->id == 20u);
    assert(strcmp(sr_session_current(&session)->label, "Secure Boot") == 0);
}

static void test_grayed_refresh_keeps_focus(void) {
    SrScreenReaderSession session;
    SrHiiRecord refreshed[3];

    sr_session_init(&session, 3u);
    assert(sr_session_apply_hii(&session, initial_records, 3u, 0));
    assert(sr_session_navigate(&session, SR_NAV_NEXT, 0));
    sr_speech_cancel_all(&session.speech);

    memcpy(refreshed, initial_records, sizeof(refreshed));
    refreshed[1].flags = SR_HII_FLAG_GRAYED;
    assert(sr_session_apply_hii(&session, refreshed, 3u, 1));
    assert(sr_session_current(&session)->id == 20u);
    assert(sr_session_current(&session)->state & SR_STATE_DISABLED);
    assert(session.speech.active);
    assert(strstr(session.speech.current.text, "disabled") != NULL);
}

int main(void) {
    test_initial_and_refresh();
    test_navigation_cancels_stale_hint();
    test_atomic_failed_refresh();
    test_grayed_refresh_keeps_focus();
    puts("UEFI_SCREENREADER_SESSION_TESTS=PASS");
    return 0;
}
