#ifndef QEVARYNOX_UEFI_SCREENREADER_LEGACY_SESSION_ADAPTER_H
#define QEVARYNOX_UEFI_SCREENREADER_LEGACY_SESSION_ADAPTER_H

#include "legacy_bridge.h"
#include "screenreader_session.h"

int sr_legacy_session_handle_key(
    SrScreenReaderSession *session,
    SrFirmwareKey key,
    int *exit_requested
);

#endif
