# Security policy

Accessible Windows is low-level firmware research. The release preview is intended for controlled development and validation systems.

## Safety boundary

The release candidate does not intentionally provide Secure Boot bypasses, unsigned-driver loading, credential extraction, arbitrary kernel/physical-memory write primitives, SPI-flash override, SMM injection or platform-security takeover mechanisms.

HII/IFR discovery is treated as untrusted input: malformed lengths and inconsistent snapshots must fail closed. Password values must never be emitted in spoken output.

Physical testing should use disposable media and systems whose owner has authorized the test. The USB image builder warns that writing an image to a device destroys existing data on that device.

Security-sensitive findings should be reported privately to the repository owner before public disclosure where practical.
