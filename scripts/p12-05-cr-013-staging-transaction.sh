#!/usr/bin/env bash
# Executes the one-shot, rollback-only P12-05 CR-013 Staging validation.
#
# The caller supplies five NUL-delimited values on stdin:
# base URL, effective Bearer, selected model, parsed host, parsed port.
# It never writes or emits any of those values. This script must run as root
# on the isolated Staging host and always restores the database preimage and
# original release symlink before it exits.

set -Eeuo pipefail
umask 077

artifact_revision='49f8c0f3eb6326d3f2ed6cc612ec8ffd10915938'
artifact_gateway_sha256='e6e13f3a3e69eb7d0ede11815c9b526bf8d6e33a96c5343307b8faf20eb260f4'
artifact_manifest_sha256='367eb2ff4f0dea16b3f69ce6a780e7f504cc233865cb9e73c4401dadf49388d5'
service='cpa-rust-gateway.service'
incumbent='cli-proxy-api.service'
state_dir='/var/lib/cpa-rust-gateway'
database="$state_dir/control.sqlite3"
current_link='/opt/cpa-rust-gateway/current'
release_dir="/opt/cpa-rust-gateway/releases/$artifact_revision"
release_binary="$release_dir/gateway"
evidence_root="$state_dir/.p12-05-evidence"
version_id='p12-05-cr-013'
request_id='p1-request-0'

read_nul() {
    local destination=$1
    IFS= read -r -d '' "$destination"
}

read_nul base_url
read_nul bearer
read_nul model
read_nul host
read_nul port

for value in "$base_url" "$bearer" "$model" "$host" "$port"; do
    [[ -n "$value" && "$value" != *$'\n'* && "$value" != *$'\r'* ]] || exit 64
done
[[ "$base_url" == https://* ]] || exit 64
[[ "$host" =~ ^[A-Za-z0-9.-]+$ ]] || exit 64
[[ "$port" =~ ^[0-9]+$ ]] && ((port >= 1 && port <= 65535)) || exit 64
[[ "$bearer" =~ ^[[:graph:]]+$ ]] || exit 64

wait_ready() {
    local body attempt
    for attempt in $(seq 1 15); do
        body=$(curl --noproxy '*' --silent --max-time 2 --output - \
            'http://127.0.0.1:18180/healthz' 2>/dev/null) || body=''
        [[ "$body" == '{"status":"ok"}' ]] && return 0
        sleep 1
    done
    return 1
}

loopback_boundary_ok() {
    local observed
    observed=$(ss -ltnH | awk '$4 == "127.0.0.1:18180" || $4 == "127.0.0.1:18181" { print $4 }' | sort)
    [[ "$observed" == $'127.0.0.1:18180\n127.0.0.1:18181' ]] || return 1
    if ss -ltnH | awk '$4 ~ /:18180$/ || $4 ~ /:18181$/ { if ($4 != "127.0.0.1:18180" && $4 != "127.0.0.1:18181") found = 1 } END { exit found ? 0 : 1 }'; then
        return 1
    fi
}

management_curl_config() {
    local path=$1 config_version=$2 revision=$3
    printf 'url = "http://127.0.0.1:18181%s"\n' "$path"
    printf 'header = "X-Management-Key: %s"\n' "$management_key"
    printf '%s\n' 'header = "Content-Type: application/json"'
    if [[ -n "$config_version" ]]; then
        printf 'header = "X-Config-Version: %s"\n' "$config_version"
    fi
    if [[ -n "$revision" ]]; then
        printf 'header = "If-Match: %s"\n' "$revision"
    fi
    printf '%s\n' 'noproxy = "*"' 'connect-timeout = 3' 'max-time = 15' 'silent'
}

management_post_expect() {
    local expected=$1 path=$2 config_version=$3 revision=$4 status rc
    exec 3< <(management_curl_config "$path" "$config_version" "$revision")
    set +e
    status=$(curl --config /dev/fd/3 --request POST --data-binary @- \
        --output /dev/null --write-out '%{http_code}' 2>/dev/null)
    rc=$?
    exec 3<&-
    set -e
    [[ $rc -eq 0 && "$status" == "$expected" ]]
}

management_post_capture() {
    local expected=$1 path=$2 config_version=$3 revision=$4 combined marker status payload rc
    marker=$'\n__P12_HTTP_STATUS__'
    exec 3< <(management_curl_config "$path" "$config_version" "$revision")
    set +e
    combined=$(curl --config /dev/fd/3 --request POST --data-binary @- \
        --output - --write-out "${marker}%{http_code}" 2>/dev/null)
    rc=$?
    exec 3<&-
    set -e
    [[ $rc -eq 0 ]] || return 1
    status=${combined##*"$marker"}
    payload=${combined%"$marker"*}
    [[ "$status" == "$expected" ]] || return 1
    printf '%s' "$payload"
}

management_get_capture() {
    local expected=$1 path=$2 config_version=$3 combined marker status payload rc
    marker=$'\n__P12_HTTP_STATUS__'
    exec 3< <(management_curl_config "$path" "$config_version" '')
    set +e
    combined=$(curl --config /dev/fd/3 --request GET --output - \
        --write-out "${marker}%{http_code}" </dev/null 2>/dev/null)
    rc=$?
    exec 3<&-
    set -e
    [[ $rc -eq 0 ]] || return 1
    status=${combined##*"$marker"}
    payload=${combined%"$marker"*}
    [[ "$status" == "$expected" ]] || return 1
    printf '%s' "$payload"
}

payload() {
    local kind=$1
    printf '%s\0%s\0%s\0%s\0%s\0' "$base_url" "$bearer" "$model" "$host" "$port" |
        perl -MJSON::PP -e '
            my $kind = shift @ARGV;
            my $input = do { local $/; <STDIN> };
            my @parts = split /\0/, $input, -1;
            pop @parts if @parts && $parts[-1] eq q{};
            die q{missing input} unless @parts == 5;
            my ($base_url, $bearer, $model, $host, $port) = @parts;
            my $value;
            if ($kind eq q{version}) {
                $value = { id => q{p12-05-cr-013}, description => q{P12-05 CR-013 isolated Messages lifecycle validation} };
            } elsif ($kind eq q{egress}) {
                $value = { id => q{p12-krill-egress}, name => q{P12 Krill staging egress}, allowed_schemes => [q{https}], allowed_hosts => [$host], allowed_ports => [0 + $port], allowed_cidrs => [], redirect_mode => q{deny}, max_redirects => 0 };
            } elsif ($kind eq q{upstream}) {
                $value = { id => q{p12-krill-upstream}, name => q{P12 Krill staging upstream}, kind => q{openai-compatible}, enabled => JSON::PP::true, tags => [], egress_policy_id => q{p12-krill-egress} };
            } elsif ($kind eq q{endpoint}) {
                $value = { id => q{p12-krill-endpoint}, adapter_id => q{openai-compatible.responses}, api_format => q{openai/responses}, base_url => $base_url, inference_path => q{/responses}, models_path => undef, transport => q{https}, enabled => JSON::PP::true };
            } elsif ($kind eq q{credential}) {
                $value = { id => q{p12-krill-credential}, kind => q{bearer}, secret => $bearer, status => q{active} };
            } elsif ($kind eq q{binding}) {
                $value = { credential_id => q{p12-krill-credential}, enabled => JSON::PP::true, priority => 0, weight => 1, concurrency => 1 };
            } elsif ($kind eq q{public_model}) {
                $value = { id => q{p12-krill-model}, model_name => q{p12-krill-staging}, status => q{active}, display_name => q{P12 Krill staging}, capabilities => {} };
            } elsif ($kind eq q{route}) {
                $value = { id => q{p12-krill-route}, policy => q{smooth_weighted_round_robin}, max_attempts => 1, bootstrap_timeout_ms => 15000 };
            } elsif ($kind eq q{candidate}) {
                $value = { id => q{p12-krill-candidate}, endpoint_id => q{p12-krill-endpoint}, upstream_model => $model, credential_scope => q{all_active}, transform_mode => q{canonical}, enabled => JSON::PP::true, priority => 0, weight => 1, capability_override => { allow_unlisted_model => JSON::PP::true } };
            } elsif ($kind eq q{access_group}) {
                $value = { id => q{p12-krill-group}, name => q{P12 Krill staging group}, status => q{active}, limits => {} };
            } elsif ($kind eq q{grant}) {
                $value = { route_id => q{p12-krill-route}, enabled => JSON::PP::true };
            } elsif ($kind eq q{client_key}) {
                $value = { id => q{p12-krill-client}, access_group_id => q{p12-krill-group}, status => q{active} };
            } else {
                die q{unknown payload};
            }
            print JSON::PP::encode_json($value);
        ' "$kind"
}

data_curl_config() {
    local path=$1
    printf 'url = "http://127.0.0.1:18180%s"\n' "$path"
    printf 'header = "Authorization: Bearer %s"\n' "$client_key"
    printf '%s\n' 'header = "Content-Type: application/json"' 'noproxy = "*"' 'connect-timeout = 3' 'max-time = 20' 'silent'
}

data_get_capture() {
    local expected=$1 path=$2 combined marker status payload rc
    marker=$'\n__P12_HTTP_STATUS__'
    exec 3< <(data_curl_config "$path")
    set +e
    combined=$(curl --config /dev/fd/3 --request GET --output - \
        --write-out "${marker}%{http_code}" </dev/null 2>/dev/null)
    rc=$?
    exec 3<&-
    set -e
    [[ $rc -eq 0 ]] || return 1
    status=${combined##*"$marker"}
    payload=${combined%"$marker"*}
    [[ "$status" == "$expected" ]] || return 1
    printf '%s' "$payload"
}

failure_stage='none'
models_result='not_run'
message_status_class='not_sent'
message_lifecycle='not_checked'
attempt_projection='not_queried'
attempt_outcome='not_available'
attempt_stage='not_available'
external_request='no'
transaction_result='hard_stop'
rollback_result='not_started'
progress_stage='preflight'
original_target=''
preimage=''
preimage_created=0

rollback() {
    local prior_status=$?
    local rollback_ok=1 restored_db=0 restarted=0
    trap - EXIT
    set +e
    mkdir -p "$evidence_root"
    chown root:root "$evidence_root"
    chmod 0700 "$evidence_root"
    systemctl stop "$service" >/dev/null 2>&1 || rollback_ok=0
    if [[ -n "$original_target" ]]; then
        rm -f "$current_link.next"
        ln -s "$original_target" "$current_link.next" >/dev/null 2>&1 || rollback_ok=0
        mv -Tf "$current_link.next" "$current_link" >/dev/null 2>&1 || rollback_ok=0
    else
        rollback_ok=0
    fi
    if [[ $preimage_created -eq 1 && -f "$preimage" ]]; then
        rm -f "$database-wal" "$database-shm" "$database-journal"
        install -o cpa-gateway -g cpa-gateway -m 0600 "$preimage" "$database" >/dev/null 2>&1 || rollback_ok=0
        cmp -s "$preimage" "$database" || rollback_ok=0
        restored_db=1
    else
        rollback_ok=0
    fi
    systemctl start "$service" >/dev/null 2>&1 || rollback_ok=0
    if wait_ready && loopback_boundary_ok && systemctl is-active --quiet "$incumbent"; then
        restarted=1
    else
        rollback_ok=0
    fi
    if [[ $rollback_ok -eq 1 ]]; then
        rollback_result='restored'
    else
        rollback_result='failed'
    fi
    local receipt="$evidence_root/$run_id-cr-013-receipt.env"
    {
        printf 'task=P12-05\n'
        printf 'change=CR-P12-05-013\n'
        printf 'artifact_revision=%s\n' "$artifact_revision"
        printf 'artifact_gateway_sha256=%s\n' "$artifact_gateway_sha256"
        printf 'artifact_manifest_sha256=%s\n' "$artifact_manifest_sha256"
        printf 'models=%s\n' "$models_result"
        printf 'external_messages_request=%s\n' "$external_request"
        printf 'messages_status_class=%s\n' "$message_status_class"
        printf 'messages_lifecycle=%s\n' "$message_lifecycle"
        printf 'attempt_projection=%s\n' "$attempt_projection"
        printf 'attempt_outcome=%s\n' "$attempt_outcome"
        printf 'attempt_stage=%s\n' "$attempt_stage"
        printf 'transaction_result=%s\n' "$transaction_result"
        printf 'progress_stage=%s\n' "$progress_stage"
        printf 'exit_status=%s\n' "$prior_status"
        printf 'failure_stage=%s\n' "$failure_stage"
        printf 'database_preimage_restored=%s\n' "$restored_db"
        printf 'staging_restart_after_rollback=%s\n' "$restarted"
        printf 'incumbent_continuity=%s\n' "$restarted"
        printf 'rollback=%s\n' "$rollback_result"
    } > "$receipt"
    chown root:root "$receipt"
    chmod 0600 "$receipt"
    unset bearer client_key management_key base_url model host port
    printf 'P12-05 CR-013: models=%s messages=%s lifecycle=%s attempt=%s/%s rollback=%s\n' \
        "$models_result" "$message_status_class" "$message_lifecycle" "$attempt_outcome" "$attempt_stage" "$rollback_result"
    if [[ $prior_status -ne 0 || $rollback_ok -ne 1 ]]; then
        exit 1
    fi
    exit 0
}

[[ "$(uname -m)" == 'x86_64' ]] || exit 64
systemctl is-active --quiet "$service" || exit 64
systemctl is-active --quiet "$incumbent" || exit 64
enabled_state=$(systemctl is-enabled "$service" 2>/dev/null || true)
[[ "$enabled_state" == 'disabled' ]] || exit 64
loopback_boundary_ok || exit 64
proxy_environment=$(systemctl show "$service" -p Environment --value)
if [[ "$proxy_environment" == *HTTP_PROXY* || "$proxy_environment" == *HTTPS_PROXY* || "$proxy_environment" == *ALL_PROXY* || "$proxy_environment" == *http_proxy* || "$proxy_environment" == *https_proxy* || "$proxy_environment" == *all_proxy* ]]; then
    exit 64
fi
[[ -x "$release_binary" ]] || exit 64
[[ "$(sha256sum "$release_binary" | awk '{print $1}')" == "$artifact_gateway_sha256" ]] || exit 64
original_target=$(readlink -f "$current_link")
[[ -n "$original_target" && -x "$original_target/gateway" ]] || exit 64
[[ "$original_target" != "$release_dir" ]] || exit 64
counts=$(sqlite3 -readonly "$database" 'select (select count(*) from config_versions), (select count(*) from egress_policies), (select count(*) from upstreams), (select count(*) from upstream_endpoints), (select count(*) from upstream_credentials), (select count(*) from endpoint_credential_bindings), (select count(*) from public_models), (select count(*) from model_routes), (select count(*) from route_candidates), (select count(*) from access_groups), (select count(*) from access_group_routes), (select count(*) from client_keys);')
[[ "$counts" == '0|0|0|0|0|0|0|0|0|0|0|0' ]] || exit 64

run_id="p12-05-cr-013-$(date -u +%Y%m%dT%H%M%SZ)"
preimage="$evidence_root/$run_id-control.sqlite3.preimage"
preimage_manifest="$evidence_root/$run_id-preimage-manifest.env"
progress_file="$evidence_root/$run_id-progress.env"
trap rollback EXIT

mark() {
    progress_stage=$1
    printf '%s\n' "$progress_stage" >> "$progress_file"
}

progress_stage='database_snapshot'
mkdir -p "$evidence_root"
chown root:root "$evidence_root"
chmod 0700 "$evidence_root"
mark 'snapshot_before_stop'
systemctl stop "$service"
! systemctl is-active --quiet "$service" || { failure_stage='staging_stop'; exit 1; }
mark 'snapshot_before_backup'
sqlite3 -readonly "$database" ".backup $preimage"
chown root:root "$preimage"
chmod 0600 "$preimage"
test -s "$preimage" || { failure_stage='database_snapshot'; exit 1; }
preimage_created=1
mark 'snapshot_created'
{
    printf 'task=P12-05\n'
    printf 'change=CR-P12-05-013\n'
    printf 'artifact_revision=%s\n' "$artifact_revision"
    printf 'original_release=%s\n' "$(basename "$original_target")"
    printf 'preimage_sha256=%s\n' "$(sha256sum "$preimage" | awk '{print $1}')"
} > "$preimage_manifest"
chown root:root "$preimage_manifest"
chmod 0600 "$preimage_manifest"

progress_stage='artifact_switch'
mark 'artifact_before_link'
rm -f "$current_link.next"
ln -s "$release_dir" "$current_link.next"
mv -Tf "$current_link.next" "$current_link"
systemctl start "$service"
wait_ready || { failure_stage='staging_start_new_artifact'; exit 1; }
loopback_boundary_ok || { failure_stage='listener_boundary_new_artifact'; exit 1; }
systemctl is-active --quiet "$incumbent" || { failure_stage='incumbent_continuity_before_graph'; exit 1; }
mark 'artifact_started'

progress_stage='management_graph'
mark 'graph_before_management_key'
credential_path="/run/credentials/$service/management-key"
[[ -r "$credential_path" ]] || { failure_stage='management_credential'; exit 1; }
management_key=''
# systemd LoadCredential files are permitted to omit a trailing newline.  In
# that case `read` assigns the complete non-empty value but returns 1 at EOF;
# handle that documented shell behavior explicitly rather than letting
# `set -e` bypass the value-free failure classification and rollback receipt.
if IFS= read -r management_key < "$credential_path"; then
    :
elif [[ -n "$management_key" ]]; then
    :
else
    failure_stage='management_credential'
    exit 1
fi
[[ "$management_key" =~ ^[A-Za-z0-9_-]+$ ]] || { failure_stage='management_credential'; exit 1; }
mark 'graph_before_config_version'

config_body=$(payload version | management_post_capture 201 '/admin/config-versions' '' '') || { failure_stage='config_version'; exit 1; }
printf '%s' "$config_body" | jq -e '.id == "p12-05-cr-013" and .status == "draft" and .revision == "rev-0"' >/dev/null || { failure_stage='config_version_shape'; exit 1; }
unset config_body
mark 'graph_before_resources'

payload egress | management_post_expect 201 '/admin/egress-policies' "$version_id" 'rev-0' || { failure_stage='egress_policy'; exit 1; }
payload upstream | management_post_expect 201 '/admin/upstreams' "$version_id" 'rev-1' || { failure_stage='upstream'; exit 1; }
payload endpoint | management_post_expect 201 '/admin/upstreams/p12-krill-upstream/endpoints' "$version_id" 'rev-2' || { failure_stage='endpoint'; exit 1; }
payload credential | management_post_expect 201 '/admin/upstreams/p12-krill-upstream/credentials' "$version_id" 'rev-3' || { failure_stage='credential'; exit 1; }
payload binding | management_post_expect 201 '/admin/endpoints/p12-krill-endpoint/credential-bindings' "$version_id" 'rev-4' || { failure_stage='binding'; exit 1; }
payload public_model | management_post_expect 201 '/admin/public-models' "$version_id" 'rev-5' || { failure_stage='public_model'; exit 1; }
payload route | management_post_expect 201 '/admin/public-models/p12-krill-model/routes' "$version_id" 'rev-6' || { failure_stage='route'; exit 1; }
payload candidate | management_post_expect 201 '/admin/routes/p12-krill-route/candidates' "$version_id" 'rev-7' || { failure_stage='candidate'; exit 1; }
payload access_group | management_post_expect 201 '/admin/access-groups' "$version_id" 'rev-8' || { failure_stage='access_group'; exit 1; }
payload grant | management_post_expect 201 '/admin/access-groups/p12-krill-group/routes' "$version_id" 'rev-9' || { failure_stage='access_grant'; exit 1; }
client_body=$(payload client_key | management_post_capture 201 '/admin/client-keys' "$version_id" 'rev-10') || { failure_stage='client_key'; exit 1; }
client_key=$(printf '%s' "$client_body" | jq -er 'if .id == "p12-krill-client" and .status == "active" and (.key | type == "string" and length > 0) then .key else error("invalid client key response") end' 2>/dev/null) || { failure_stage='client_key_shape'; exit 1; }
unset client_body
[[ "$client_key" =~ ^rgw_[0-9a-f]{16}_[0-9a-f]{64}$ ]] || { failure_stage='client_key_shape'; exit 1; }
mark 'graph_resources_created'

graph_counts=$(sqlite3 -readonly "$database" "select (select count(*) from egress_policies where config_version_id='$version_id'), (select count(*) from upstreams where config_version_id='$version_id'), (select count(*) from upstream_endpoints where config_version_id='$version_id'), (select count(*) from upstream_credentials where config_version_id='$version_id'), (select count(*) from endpoint_credential_bindings where config_version_id='$version_id'), (select count(*) from public_models where config_version_id='$version_id'), (select count(*) from model_routes where config_version_id='$version_id'), (select count(*) from route_candidates where config_version_id='$version_id'), (select count(*) from access_groups where config_version_id='$version_id'), (select count(*) from access_group_routes where config_version_id='$version_id'), (select count(*) from client_keys where config_version_id='$version_id');")
[[ "$graph_counts" == '1|1|1|1|1|1|1|1|1|1|1' ]] || { failure_stage='singleton_graph'; exit 1; }
credential_metadata=$(sqlite3 -readonly "$database" "select count(*), sum(length(ciphertext) > 0), min(key_version > 0) from upstream_credentials where config_version_id='$version_id';")
[[ "$credential_metadata" == '1|1|1' ]] || { failure_stage='credential_envelope'; exit 1; }
client_key_metadata=$(sqlite3 -readonly "$database" "select count(*), sum(length(secret_digest) = 32) from client_keys where config_version_id='$version_id';")
[[ "$client_key_metadata" == '1|1' ]] || { failure_stage='client_key_envelope'; exit 1; }
mark 'graph_before_validate'

validation_body=$(printf '' | management_post_capture 200 "/admin/config-versions/$version_id/validate" "$version_id" '') || { failure_stage='configuration_validate'; exit 1; }
printf '%s' "$validation_body" | jq -e '.valid == true and .error_codes == []' >/dev/null || { failure_stage='configuration_validate_shape'; exit 1; }
unset validation_body
publish_body=$(printf '' | management_post_capture 200 "/admin/config-versions/$version_id/publish" "$version_id" 'rev-11') || { failure_stage='configuration_publish'; exit 1; }
printf '%s' "$publish_body" | jq -e '.active_config_version_id == "p12-05-cr-013" and ((has("replaced_config_version_id") | not) or .replaced_config_version_id == null)' >/dev/null || { failure_stage='configuration_publish_shape'; exit 1; }
unset publish_body
mark 'graph_published'

progress_stage='active_graph_restart'
mark 'active_before_restart'
systemctl restart "$service"
wait_ready || { failure_stage='staging_restart_active_graph'; exit 1; }
loopback_boundary_ok || { failure_stage='listener_boundary_active_graph'; exit 1; }
systemctl is-active --quiet "$incumbent" || { failure_stage='incumbent_continuity_before_models'; exit 1; }
mark 'active_restart_complete'

progress_stage='models'
mark 'models_before_get'
models_body=$(data_get_capture 200 '/v1/models') || { models_result='fail'; failure_stage='models'; exit 1; }
printf '%s' "$models_body" | jq -e '.data | type == "array" and length == 1 and .[0].id == "p12-krill-staging"' >/dev/null || { models_result='fail'; failure_stage='models_shape'; exit 1; }
unset models_body
models_result='pass'
mark 'models_passed'

progress_stage='messages'
mark 'messages_before_post'
message_payload='{"model":"p12-krill-staging","max_tokens":1,"messages":[{"role":"user","content":"ok"}],"stream":false}'
external_request='one'
exec 3< <(data_curl_config '/v1/messages')
set +e
message_marker=$'\n__P12_HTTP_STATUS__'
message_combined=$(printf '%s' "$message_payload" | curl --config /dev/fd/3 --request POST --data-binary @- \
    --output - --write-out "${message_marker}%{http_code}" 2>/dev/null)
message_rc=$?
exec 3<&-
set -e
message_status=${message_combined##*"$message_marker"}
message_body=${message_combined%"$message_marker"*}
unset message_payload message_combined
if [[ $message_rc -ne 0 ]]; then
    message_status_class='transport_error'
elif [[ "$message_status" == 2* ]]; then
    message_status_class='2xx'
    if printf '%s' "$message_body" | jq -e '
        type == "object" and
        .type == "message" and
        .role == "assistant" and
        (.content | type == "array") and
        .stop_reason == "end_turn" and
        .stop_sequence == null and
        (.usage | type == "object") and
        (.usage.input_tokens | (type == "number" and . >= 0)) and
        (.usage.output_tokens | (type == "number" and . >= 0))
    ' >/dev/null; then
        message_lifecycle='valid'
    else
        message_lifecycle='invalid'
        unset message_body
        failure_stage='messages_lifecycle'
        exit 1
    fi
elif [[ "$message_status" == 4* ]]; then
    message_status_class='4xx'
elif [[ "$message_status" == 5* ]]; then
    message_status_class='5xx'
else
    message_status_class='other_http'
fi
unset message_body
mark 'messages_post_complete'

progress_stage='attempt_projection'
mark 'attempt_before_get'
attempts_body=$(management_get_capture 200 "/admin/requests/$request_id/attempts" "$version_id") || { attempt_projection='unavailable'; failure_stage='attempt_projection'; exit 1; }
printf '%s' "$attempts_body" | jq -e '
    type == "array" and length == 1 and
    (.[0] | (keys | sort) == ["attempt_id", "outcome", "stage"]) and
    (.[0].outcome == "succeeded" or .[0].outcome == "failed") and
    (.[0].stage == "request_conversion" or .[0].stage == "egress_admission" or .[0].stage == "http_transport" or .[0].stage == "http_status" or .[0].stage == "content_type" or .[0].stage == "body_read" or .[0].stage == "decoder" or .[0].stage == "sse_bootstrap")
' >/dev/null || { attempt_projection='invalid'; failure_stage='attempt_projection'; exit 1; }
attempt_outcome=$(printf '%s' "$attempts_body" | jq -er '.[0].outcome')
attempt_stage=$(printf '%s' "$attempts_body" | jq -er '.[0].stage')
unset attempts_body
attempt_projection='valid'
transaction_result='completed'
progress_stage='completed'
mark 'completed'
exit 0
