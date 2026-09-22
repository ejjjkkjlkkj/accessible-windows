#include "hii_adapter.h"

#include <assert.h>
#include <string.h>
#include <stdio.h>

static void test_semantic_mapping(void) {
    static const SrHiiRecord records[] = {
        {1, SR_HII_OP_SUBTITLE, SR_HII_FLAG_NONE, "Boot", "", ""},
        {2, SR_HII_OP_CHECKBOX, SR_HII_FLAG_CHECKED, "Secure Boot", "Enabled", "Press Enter to toggle"},
        {3, SR_HII_OP_ONE_OF, SR_HII_FLAG_CHANGED, "Boot mode", "UEFI", "Use Left or Right"},
        {4, SR_HII_OP_PASSWORD, SR_HII_FLAG_NONE, "Supervisor password", "must-never-escape", "Press Enter to edit"},
        {5, SR_HII_OP_ACTION, SR_HII_FLAG_DANGER, "Restore defaults", "", "Resets\nfirmware settings"}
    };
    SrSemanticSnapshot snapshot;
    SrNavigator nav;
    char spoken[SR_MAX_SPEECH_TEXT];
    char hint[SR_MAX_SPEECH_TEXT];

    assert(sr_hii_snapshot_build(&snapshot, records, sizeof(records) / sizeof(records[0])));
    assert(snapshot.count == 5);
    assert(snapshot.items[0].role == SR_ROLE_SEPARATOR);
    assert(snapshot.items[1].role == SR_ROLE_TOGGLE);
    assert(snapshot.items[1].state & SR_STATE_CHECKED);
    assert(snapshot.items[2].role == SR_ROLE_CHOICE);
    assert(snapshot.items[2].state & SR_STATE_CHANGED);
    assert(snapshot.items[3].role == SR_ROLE_PASSWORD);
    assert(strcmp(snapshot.items[3].value, "") == 0);
    assert(snapshot.items[4].state & SR_STATE_DANGER);
    assert(strcmp(snapshot.items[4].hint, "Resets firmware settings") == 0);

    sr_nav_init(&nav, snapshot.items, snapshot.count, 3);
    assert(sr_nav_current(&nav)->id == 2);

    sr_format_focus(&nav, spoken, sizeof(spoken));
    assert(strstr(spoken, "Secure Boot") != NULL);
    assert(strstr(spoken, "Press Enter to toggle") == NULL);

    sr_format_hint(&nav, hint, sizeof(hint));
    assert(strcmp(hint, "Press Enter to toggle") == 0);

    while (sr_nav_current(&nav)->id != 4) {
        assert(sr_nav_move(&nav, SR_NAV_NEXT, 0));
    }
    sr_format_focus(&nav, spoken, sizeof(spoken));
    assert(strstr(spoken, "must-never-escape") == NULL);
}

static void test_transactional_capacity_failure(void) {
    SrSemanticSnapshot snapshot;
    SrHiiRecord too_many[SR_HII_MAX_ITEMS + 1u];
    size_t i;

    for (i = 0; i < SR_HII_MAX_ITEMS + 1u; ++i) {
        too_many[i].id = (uint32_t)i;
        too_many[i].opcode = SR_HII_OP_ACTION;
        too_many[i].flags = 0;
        too_many[i].prompt = "Action";
        too_many[i].value = "";
        too_many[i].help = "";
    }

    assert(!sr_hii_snapshot_build(&snapshot, too_many, SR_HII_MAX_ITEMS + 1u));
    assert(snapshot.count == 0);
    assert(snapshot.text_used == 0);
}

static void test_transactional_text_failure(void) {
    SrSemanticSnapshot snapshot;
    SrHiiRecord record;
    char huge[SR_HII_TEXT_CAPACITY + 32u];
    size_t i;

    for (i = 0; i + 1u < sizeof(huge); ++i) huge[i] = 'A';
    huge[sizeof(huge) - 1u] = '\0';

    record.id = 1;
    record.opcode = SR_HII_OP_ACTION;
    record.flags = 0;
    record.prompt = huge;
    record.value = "";
    record.help = "";

    assert(!sr_hii_snapshot_build(&snapshot, &record, 1));
    assert(snapshot.count == 0);
    assert(snapshot.text_used == 0);
}

int main(void) {
    test_semantic_mapping();
    test_transactional_capacity_failure();
    test_transactional_text_failure();
    puts("UEFI_HII_ADAPTER_TESTS=PASS");
    return 0;
}
