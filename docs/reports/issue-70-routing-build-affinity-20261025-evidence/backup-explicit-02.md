# Explicit encrypted backup receipt

Source: `731c318320ae6f9df69cc77a708ea67c468a2695` / tree `0ca2706201f55a1a6d12e89cb7b34fb3c23aeeb7`.

Command: `cargo test --locked -p gateway-store --test encrypted_backup -- --include-ignored`

Full original log SHA-256: `2f1d28d2075c2d0f690e856b878b443c14d63743e992a2a1ec651ca883acae80`. The following excerpt is the completed result, not an additional run.

```text
running 7 tests
test chunked_backup_write_failure_and_restore_interruption_preserve_source ... ok
test chunked_backup_keeps_embedding_read_snapshot_and_transaction ... ok
test malformed_and_oversized_material_are_rejected_before_staging_or_restore ... ok
test wrong_key_tampering_and_existing_target_fail_without_creating_or_replacing_a_database ... ok
test encrypted_snapshot_round_trips_to_a_new_target_with_schema_and_configuration_intact ... ok
test chunked_backup_rejects_oversized_source_before_artifact_publication ... ok
test chunked_backup_authenticates_order_length_and_legacy_compatibility ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.19s
```
