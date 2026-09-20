# Billing catalog capacity admission

Status: accepted and implemented by Codex for the confirmed remaining-work plan; C2C blocker B14.

The management list has an authoritative 256-catalog bound, while append previously allowed a 257th catalog and made future reads unavailable. Import and forward restore now reject capacity exhaustion in the same immediate SQLite transaction as insertion, draft revision and audit. Concurrent writers through different drafts share that global boundary. Existing catalogs and history are retained.

No schema, endpoint or input/output shape is added. Both append operations document existing `Error` response 400 with `invalid_management_request` for exhausted capacity; duplicate IDs remain 409. The frontend preflights the bound, but only the Store transaction is authoritative. Recovery by deletion or history cleanup is not introduced.

Evidence: service final-permitted/rejected-import/rejected-restore with unchanged revision/audit; concurrent Store writers allow one final append; protected HTTP keeps the 256-row list readable; fixture and UI capacity coverage. Generated artifacts follow `sync-contract` only.
