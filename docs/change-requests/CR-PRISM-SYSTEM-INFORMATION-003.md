# Safe system information for Prism

2026-09-13. Accepted and implemented by Codex under the current joint frontend/backend task.

Add authenticated, same-origin `GET /admin/system`. It returns the running binary's package
version, revision, target, Rust compiler version, supported/migrated schema, elapsed service
uptime and the live configuration application mode. Build identity comes from existing signed
release environment fields compiled into the binary; development builds explicitly say
`development`. No environment dump, paths, listener addresses, secrets, provider calls or
configuration writes. Without the explicit runtime attachment it returns 503.

The settings page consumes the authoritative DTO after contract synchronization. Runtime,
egress and advanced configuration remain their existing task-specific tools. No unsupported
server-setting switches are introduced.

`accepting_requests` reports the controller's actual new-request admission flag. It is nullable
when no runtime observer is attached, rather than treating an unobserved value as healthy.
The settings card distinguishes paused admission from the capability to apply configuration live,
and retains access to the existing safe runtime-apply operation after an operator closes a failed
native-account dialog. It does not claim provider availability or cancel captured requests.
