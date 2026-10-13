"""Only normal same-authority material expiry shortage can enter bounded waiting."""
ALLOWED_REFRESH_FIELDS = {'credential_ciphertext', 'refresh_due_at_ms', 'last_refresh_at_ms',
                         'revision', 'import_batch_id', 'updated_at_ms', 'continuation_current_revision'}


def classify(observation):
    assert observation['status'] == 'PASS; read-only observation'
    assert observation['authorization_continuity_status'] == 'PASS'
    assert observation['identity_digest_matches_authorized_source'] is True
    assert observation['account_evidence_id'] == '5beff1b67bd35a0547fe42e4b72970a80f0c2c28a77596c29703d83f397c3d54'
    account = observation['source_account']
    assert account['provider'] == 'build' and account['enabled'] == 1 and account['auth_status'] == 'active'
    assert account['worker_claim_kind'] is None
    assert account['revision'] >= observation['authorized_native_revision']
    assert account['last_refresh_at_ms'] >= observation['authorized_last_refresh_at_ms']
    assert account['continuation_floor_revision'] == observation['authorized_continuation_floor_revision']
    assert account['continuation_current_revision'] == account['revision'] + 1
    assert (account['continuation_current_revision'] - observation['authorized_continuation_current_revision']
            == account['revision'] - observation['authorized_native_revision'])
    assert set(observation['source_changed_field_names']).issubset(ALLOWED_REFRESH_FIELDS)
    assert observation['source_schema'] == 31
    pool = observation['source_pool']
    assert pool['runtime_status'] == 'available' and pool['active_leases'] == 0
    assert pool['credential_revision'] == account['continuation_current_revision']
    assert type(pool['expires_at_ms']) is int
    assert pool['expires_at_ms'] > observation['observed_now_ms']
    assert observation['historical_budget_intents'] == 35
    assert observation['historical_gateway_pin_confirmed_sends'] == 34
    assert not observation['ports_listening'] and not observation['competing_processes']
    assert observation['unit']['MainPID'] == '0' and observation['unit']['LoadState'] == 'not-found'
    return ('PREPARE_WINDOW_AVAILABLE' if pool['expires_at_ms'] > observation['observed_now_ms'] + 3600000
            else 'WAITING_FOR_QUALIFICATION')


def may_observe(previous, now_ms):
    assert len(previous) < 7
    if previous:
        assert previous[-1]['classification'] == 'WAITING_FOR_QUALIFICATION'
        assert now_ms - previous[-1]['local_started_at_ms'] >= 600000
        assert now_ms - previous[0]['local_started_at_ms'] <= 3600000
