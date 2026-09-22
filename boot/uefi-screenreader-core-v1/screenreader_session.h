#ifndef QEVARYNOX_UEFI_SCREENREADER_SESSION_H
#define QEVARYNOX_UEFI_SCREENREADER_SESSION_H

#include "hii_adapter.h"

typedef struct {
    SrSemanticSnapshot snapshots[2];
    uint8_t active_snapshot;
    uint8_t has_snapshot;
    size_t page_size;
    SrNavigator nav;
    SrSpeechScheduler speech;
} SrScreenReaderSession;

void sr_session_init(SrScreenReaderSession *session, size_t page_size);
int sr_session_apply_hii(
    SrScreenReaderSession *session,
    const SrHiiRecord *records,
    size_t record_count,
    int announce_change
);
int sr_session_navigate(
    SrScreenReaderSession *session,
    SrNavCommand command,
    char first_letter
);
int sr_session_speak_hint(SrScreenReaderSession *session);
int sr_session_repeat_focus(SrScreenReaderSession *session);
int sr_session_chooser_open(SrScreenReaderSession *session);
int sr_session_chooser_type(SrScreenReaderSession *session, char ch);
int sr_session_chooser_backspace(SrScreenReaderSession *session);
int sr_session_chooser_next(SrScreenReaderSession *session);
int sr_session_chooser_previous(SrScreenReaderSession *session);
int sr_session_chooser_select(SrScreenReaderSession *session);
int sr_session_chooser_cancel(SrScreenReaderSession *session);
const SrItem *sr_session_current(const SrScreenReaderSession *session);
int sr_session_speech_complete(SrScreenReaderSession *session, SrSpeechEvent *next);

#endif
