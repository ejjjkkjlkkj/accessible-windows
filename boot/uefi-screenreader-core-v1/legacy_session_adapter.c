#include "legacy_session_adapter.h"

int sr_legacy_session_handle_key(
    SrScreenReaderSession *session,
    SrFirmwareKey key,
    int *exit_requested
) {
    SrFirmwareCommand command;

    if (exit_requested) *exit_requested = 0;
    if (!session) return 0;
    if (!sr_legacy_key_decode(key, session->nav.chooser_open, &command)) return 0;

    switch (command.kind) {
        case SR_FW_CMD_NAVIGATE:
            return sr_session_navigate(session, command.nav, command.ch);
        case SR_FW_CMD_REPEAT:
            return sr_session_repeat_focus(session);
        case SR_FW_CMD_CHOOSER_OPEN:
            return sr_session_chooser_open(session);
        case SR_FW_CMD_CHOOSER_CANCEL:
            return sr_session_chooser_cancel(session);
        case SR_FW_CMD_CHOOSER_SELECT:
            return sr_session_chooser_select(session);
        case SR_FW_CMD_CHOOSER_NEXT:
            return sr_session_chooser_next(session);
        case SR_FW_CMD_CHOOSER_PREVIOUS:
            return sr_session_chooser_previous(session);
        case SR_FW_CMD_CHOOSER_BACKSPACE:
            return sr_session_chooser_backspace(session);
        case SR_FW_CMD_CHOOSER_TYPE:
            return sr_session_chooser_type(session, command.ch);
        case SR_FW_CMD_EXIT:
            if (exit_requested) *exit_requested = 1;
            return 1;
        default:
            return 0;
    }
}
