#!/usr/bin/env bash

# Copyright Elasticsearch B.V. and/or licensed to Elasticsearch B.V. under one
# or more contributor license agreements. Licensed under the Elastic License 2.0.

# Non-networked tests for the ESDiag Claude Code plugin: packaging, client
# configuration resolution, esdiag output parsing, and streaming event handling.
#
# Run from the repository root:
#   ./tests/plugin.sh
#   ./tests/plugin.sh --only config_applies_default_agent

set -uo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
scripts="${root}/plugin/scripts"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

only=""
[[ "${1:-}" == "--only" ]] && only="${2:-}"

passed=0
failed=0

fail() { printf '  FAIL: %s\n' "$*" >&2; failed=$((failed + 1)); return 1; }
# Assert a command fails. Written as a helper because `cmd && fail ...` returns
# non-zero on the success path, which silently drops the test from the count.
assert_fails() {
    local description="$1"; shift
    if "$@" >/dev/null 2>&1; then fail "$description"; else return 0; fi
}
assert_eq() { [[ "$1" == "$2" ]] || fail "expected '$2', got '$1'"; }
assert_contains() { printf '%s' "$1" | grep -Fq -- "$2" || fail "output does not contain: $2"; }
assert_not_contains() { ! printf '%s' "$1" | grep -Fq -- "$2" || fail "output unexpectedly contains: $2"; }

# Isolate every case from the developer's real environment and credentials.
base_env() {
    env -u ESDIAG_KIBANA_URL -u ESDIAG_KIBANA_SPACE -u ESDIAG_KIBANA_APIKEY \
        -u ESDIAG_KIBANA_APIKEY_FILE -u ESDIAG_AGENT_ID -u ESDIAG_INFERENCE_ID \
        -u ESDIAG_ELASTICSEARCH_URL -u ESDIAG_OUTPUT_URL \
        -u ESDIAG_JOB -u ESDIAG_DIAGNOSTIC_MAX_AGE \
        ESDIAG_HOME="$tmp/home" "$@"
}

cfg() { base_env "$@" "$scripts/config.sh" --json; }

# ---------------------------------------------------------------- packaging --

test_bundled_skill_matches_source() {
    "${root}/bin/sync-plugin-skill.sh" --check >/dev/null || fail "bundled skill drifted from .agents/skills/esdiag"
}

test_sync_detects_drift() {
    local isolated="$tmp/sync-repo"
    mkdir -p "$isolated/bin" "$isolated/.agents/skills" "$isolated/plugin/skills"
    cp "${root}/bin/sync-plugin-skill.sh" "$isolated/bin/"
    cp -R "${root}/.agents/skills/esdiag" "$isolated/.agents/skills/"
    cp -R "${root}/plugin/skills/esdiag" "$isolated/plugin/skills/"
    printf '\ndrift\n' >> "$isolated/plugin/skills/esdiag/SKILL.md"
    assert_fails "drift was not detected" "$isolated/bin/sync-plugin-skill.sh" --check
}

test_bundled_skill_excludes_provider_metadata() {
    [[ ! -e "${root}/plugin/skills/esdiag/agents" ]] || fail "provider-specific agents metadata was bundled"
}

test_plugin_manifest_is_valid_json() {
    jq -e . "${root}/plugin/.claude-plugin/plugin.json" >/dev/null || fail "plugin.json is not valid JSON"
    jq -e . "${root}/.claude-plugin/marketplace.json" >/dev/null || fail "marketplace.json is not valid JSON"
    assert_eq "$(jq -r '.displayName' "${root}/plugin/.claude-plugin/plugin.json")" "ESDiag"
}

test_plugin_version_matches_package_version() {
    local package_version plugin_version
    package_version="$(awk -F '"' '/^version = / {print $2; exit}' "${root}/Cargo.toml")"
    package_version="${package_version%-SNAPSHOT}"
    plugin_version="$(jq -r '.version' "${root}/plugin/.claude-plugin/plugin.json")"
    assert_eq "$plugin_version" "$package_version"
}

test_marketplace_entry_matches_plugin_name() {
    assert_eq "$(jq -r '.plugins[0].name' "${root}/.claude-plugin/marketplace.json")" \
              "$(jq -r '.name' "${root}/plugin/.claude-plugin/plugin.json")"
}

test_marketplace_source_resolves() {
    local src
    src="$(jq -r '.plugins[0].source' "${root}/.claude-plugin/marketplace.json")"
    [[ -f "${root}/${src}/.claude-plugin/plugin.json" ]] || fail "marketplace source does not resolve: $src"
}

# ----------------------------------------------------------- configuration --

test_config_applies_default_agent() {
    assert_eq "$(cfg ESDIAG_KIBANA_URL=https://kb.example | jq -r .agent_id)" "elastic-ai-agent"
}

test_config_agent_override_wins() {
    assert_eq "$(cfg ESDIAG_KIBANA_URL=https://kb.example ESDIAG_AGENT_ID=ada | jq -r .agent_id)" "ada"
}

test_config_defaults_space_to_esdiag() {
    assert_eq "$(cfg ESDIAG_KIBANA_URL=https://kb.example | jq -r .space)" "esdiag"
}

test_config_scopes_paths_to_space() {
    assert_contains "$(cfg ESDIAG_KIBANA_URL=https://kb.example | jq -r .converse_url)" "/s/esdiag/api/agent_builder/converse/async"
}

test_config_does_not_double_space_suffix() {
    # esdiag-local writes ESDIAG_KIBANA_URL with the space path already present.
    local out
    out="$(cfg ESDIAG_KIBANA_URL=http://localhost:5601/s/esdiag | jq -r .converse_url)"
    assert_contains "$out" "/s/esdiag/api/agent_builder"
    assert_not_contains "$out" "/s/esdiag/s/esdiag"
}

test_config_empty_space_means_default_space() {
    local out
    out="$(base_env ESDIAG_KIBANA_URL=https://kb.example ESDIAG_KIBANA_SPACE= "$scripts/config.sh" --json | jq -r .converse_url)"
    assert_contains "$out" "https://kb.example/api/agent_builder"
    assert_not_contains "$out" "/s/"
}

test_config_omits_model_routing_when_unset() {
    assert_eq "$(cfg ESDIAG_KIBANA_URL=https://kb.example | jq -r '.inference_id')" "null"
}

test_config_includes_model_routing_when_set() {
    assert_eq "$(cfg ESDIAG_KIBANA_URL=https://kb.example ESDIAG_INFERENCE_ID=my-endpoint | jq -r '.inference_id')" "my-endpoint"
}

test_config_reads_api_key_from_file_reference() {
    printf 'secret-key-value\n' > "$tmp/key"
    local out
    out="$(cfg ESDIAG_KIBANA_URL=https://kb.example ESDIAG_KIBANA_APIKEY_FILE="$tmp/key")"
    assert_eq "$(printf '%s' "$out" | jq -r .api_key_source)" "file"
    # The key value itself must never appear in resolved output.
    assert_not_contains "$out" "secret-key-value"
}

test_config_requires_kibana_url() {
    assert_fails "missing ESDIAG_KIBANA_URL was accepted" \
        base_env "$scripts/config.sh" --json
}

test_config_check_requires_api_key() {
    assert_fails "missing API key was accepted by --check" \
        base_env ESDIAG_KIBANA_URL=https://kb.example "$scripts/config.sh" --check
}

test_config_default_freshness_window_is_24h() {
    assert_eq "$(cfg ESDIAG_KIBANA_URL=https://kb.example | jq -r .max_age)" "24h"
}

test_config_prefers_explicit_elasticsearch_url() {
    local out
    out="$(cfg ESDIAG_KIBANA_URL=https://kb.example \
        ESDIAG_ELASTICSEARCH_URL=https://es.example \
        ESDIAG_OUTPUT_URL=https://fallback.example)"
    assert_eq "$(printf '%s' "$out" | jq -r .elasticsearch_base)" "https://es.example"
}

test_config_falls_back_to_output_url() {
    local out
    out="$(cfg ESDIAG_KIBANA_URL=https://kb.example ESDIAG_OUTPUT_URL=https://es.example/)"
    assert_eq "$(printf '%s' "$out" | jq -r .elasticsearch_base)" "https://es.example"
}

test_config_json_handles_quotes() {
    cfg ESDIAG_KIBANA_URL=https://kb.example ESDIAG_JOB='job"quoted' | jq -e . >/dev/null \
        || fail "quoted configuration produced invalid JSON"
}

# ---------------------------------------------------------- binding client --

fake_connect_clients() {
    setup_fake_bin
    cat > "$tmp/bin/curl" <<EOF
#!/usr/bin/env bash
output_file=""
url=""
payload=""
while [ \$# -gt 0 ]; do
    case "\$1" in
        -o) output_file="\$2"; shift 2 ;;
        -d) payload="\$2"; shift 2 ;;
        -w|-H|-X|-m) shift 2 ;;
        http*) url="\$1"; shift ;;
        *) shift ;;
    esac
done
printf '%s\t%s\n' "\$url" "\$payload" >> "$tmp/connect-requests"
case "\$url" in
    */api/agent_builder/agents)
        printf '%s\n' '{"results":[{"id":"elastic-ai-agent","name":"ESDiag"}]}' > "\$output_file"
        ;;
    */_query)
        printf '%s\n' '{"columns":[],"values":[]}' > "\$output_file"
        ;;
esac
printf '200'
EOF
    cat > "$tmp/bin/esdiag" <<'EOF'
#!/usr/bin/env bash
case "${1:-} ${2:-}" in
    '--version ') printf 'esdiag 1.0.0\n' ;;
    'keystore status') printf 'Keystore: unlocked\n' ;;
    'host list') printf 'collect-host\n' ;;
esac
EOF
    chmod +x "$tmp/bin/curl" "$tmp/bin/esdiag"
}

test_connect_uses_direct_esql_without_conversation() {
    fake_connect_clients
    local out requests
    out="$(PATH="$tmp/bin:$PATH" base_env \
        ESDIAG_KIBANA_URL=https://kb.example \
        ESDIAG_ELASTICSEARCH_URL=https://es.example \
        ESDIAG_KIBANA_APIKEY=k "$scripts/connect.sh")"
    requests="$(cat "$tmp/connect-requests")"
    assert_contains "$out" 'Binding complete. No conversations or model calls were created.'
    assert_contains "$requests" 'https://kb.example/s/esdiag/api/agent_builder/agents'
    assert_contains "$requests" 'https://es.example/_query'
    assert_contains "$requests" 'metrics-*-esdiag*'
    assert_contains "$requests" 'settings-*-esdiag*'
    assert_not_contains "$requests" '/converse'
    assert_not_contains "$requests" 'tools/_execute'
}

# --------------------------------------------------------- output parsing --

test_extracts_diagnostic_id_and_kibana_link() {
    local out
    out="$(printf 'process complete in 4.212 seconds: 18432 documents for my-cluster@2026-08-07~ab12\nKibana Link: https://kb.example/app/dashboards#/view/report\n' \
        | "$scripts/extract-diagnostic.sh")"
    assert_eq "$(printf '%s' "$out" | jq -r .diagnostic_id)" "my-cluster@2026-08-07~ab12"
    assert_eq "$(printf '%s' "$out" | jq -r .kibana_link)" "https://kb.example/app/dashboards#/view/report"
}

test_extracts_included_diagnostics_separately() {
    local out
    out="$(printf 'process complete in 1.0 seconds: 10 documents for primary@2026-08-07~aaaa\n\nincluded diagnostic complete in 2.0 seconds: 20 documents for extra@2026-08-07~bbbb (kibana)\n' \
        | "$scripts/extract-diagnostic.sh")"
    assert_eq "$(printf '%s' "$out" | jq -r .diagnostic_id)" "primary@2026-08-07~aaaa"
    assert_eq "$(printf '%s' "$out" | jq -r '.included[0]')" "extra@2026-08-07~bbbb"
}

test_extract_reports_missing_identifier() {
    if printf 'some unrelated output\n' | "$scripts/extract-diagnostic.sh" >/dev/null 2>&1; then
        fail "missing identifier was reported as success"
    fi
}

test_extract_handles_absent_kibana_link() {
    local out
    out="$(printf 'process complete in 1.0 seconds: 10 documents for only@2026-08-07~cccc\n' | "$scripts/extract-diagnostic.sh")"
    assert_eq "$(printf '%s' "$out" | jq -r .kibana_link)" "null"
}

# ------------------------------------------------------- streaming client --

# Serve a recorded event stream from a local file so streaming behavior is
# testable without a deployment.
fake_curl_stream() {
    local fixture="$1"
    cat > "$tmp/bin/curl" <<EOF
#!/usr/bin/env bash
header_file=""
while [ \$# -gt 0 ]; do
    case "\$1" in
        --dump-header) header_file="\$2"; shift 2 ;;
        --stderr) shift 2 ;;
        *) shift ;;
    esac
done
printf 'HTTP/1.1 %s Test\r\n\r\n' "\${FAKE_CURL_STATUS:-200}" > "\$header_file"
cat "$fixture"
EOF
    chmod +x "$tmp/bin/curl"
}

setup_fake_bin() { mkdir -p "$tmp/bin"; }

analyze_with_fixture() {
    local fixture="$1"; shift
    setup_fake_bin
    fake_curl_stream "$fixture"
    PATH="$tmp/bin:$PATH" base_env ESDIAG_KIBANA_URL=https://kb.example ESDIAG_KIBANA_APIKEY=k \
        "$scripts/analyze.sh" --diagnostic 'd@1~1' --question 'q' "$@"
}

write_complete_stream() {
    cat > "$tmp/complete.sse" <<'EOF'
event: conversation_id_set
data: {"data":{"conversation_id":"conv-123"}}

: 0000000000000000

event: reasoning
data: {"data":{"reasoning":"Consulting my tools","transient":true}}

event: tool_call
data: {"data":{"tool_id":"platform.core.execute_esql","tool_call_id":"t1"}}

event: tool_result
data: {"data":{"tool_id":"platform.core.execute_esql","tool_call_id":"t1","results":[]}}

event: message_complete
data: {"data":{"message_id":"m1","message_content":"**Verdict: healthy**\n\nRelevant Dashboards:\n- [Data Summary](</s/esdiag/app/dashboards#/view/data-summary>)"}}

event: round_complete
data: {"data":{"round":{"model_usage":{"input_tokens":109622,"output_tokens":3589}}}}
EOF
}

write_interrupted_stream() {
    cat > "$tmp/interrupted.sse" <<'EOF'
event: conversation_id_set
data: {"data":{"conversation_id":"conv-orphan"}}

event: reasoning
data: {"data":{"reasoning":"Consulting my tools","transient":true}}
EOF
}

test_stream_emits_analysis_on_stdout() {
    write_complete_stream
    local out
    out="$(analyze_with_fixture "$tmp/complete.sse" 2>/dev/null)"
    assert_contains "$out" "Verdict: healthy"
}

test_stream_reports_progress_on_stderr() {
    write_complete_stream
    local err
    err="$(analyze_with_fixture "$tmp/complete.sse" 2>&1 >/dev/null)"
    assert_contains "$err" "Consulting my tools"
    assert_contains "$err" "platform.core.execute_esql"
}

test_stream_resolves_relative_dashboard_links() {
    write_complete_stream
    local out
    out="$(analyze_with_fixture "$tmp/complete.sse" 2>/dev/null)"
    assert_contains "$out" "https://kb.example/s/esdiag/app/dashboards"
    assert_not_contains "$out" "](</s/esdiag"
}

test_stream_ignores_keepalive_padding() {
    write_complete_stream
    local out
    out="$(analyze_with_fixture "$tmp/complete.sse" 2>/dev/null)"
    assert_not_contains "$out" "0000000000000000"
}

test_stream_persists_conversation_for_reuse() {
    write_complete_stream
    analyze_with_fixture "$tmp/complete.sse" >/dev/null 2>&1
    assert_contains "$(cat "$tmp/home/plugin-conversations.tsv")" "conv-123"
}

test_stream_keys_conversations_by_diagnostic() {
    write_complete_stream
    setup_fake_bin; fake_curl_stream "$tmp/complete.sse"
    PATH="$tmp/bin:$PATH" base_env ESDIAG_KIBANA_URL=https://kb.example ESDIAG_KIBANA_APIKEY=k \
        "$scripts/analyze.sh" --diagnostic 'other@2~2' --question 'q' >/dev/null 2>&1
    local rows
    rows="$(grep -c . "$tmp/home/plugin-conversations.tsv")"
    [[ "$rows" -ge 2 ]] || fail "expected a separate conversation row per diagnostic, got $rows"
}

test_stream_keys_conversations_by_deployment() {
    write_complete_stream
    setup_fake_bin; fake_curl_stream "$tmp/complete.sse"
    local before after
    before="$(grep -c . "$tmp/home/plugin-conversations.tsv" 2>/dev/null || true)"
    PATH="$tmp/bin:$PATH" base_env ESDIAG_KIBANA_URL=https://other-kb.example \
        ESDIAG_KIBANA_APIKEY=k "$scripts/analyze.sh" \
        --diagnostic 'd@1~1' --question 'q' >/dev/null 2>&1
    after="$(grep -c . "$tmp/home/plugin-conversations.tsv")"
    assert_eq "$after" "$((before + 1))"
    assert_contains "$(cat "$tmp/home/plugin-conversations.tsv")" "https://other-kb.example"
}

test_stream_attributes_authorization_failure() {
    printf '%s\n' '{"message":"forbidden"}' > "$tmp/forbidden.json"
    local err status
    err="$(FAKE_CURL_STATUS=403 analyze_with_fixture "$tmp/forbidden.json" 2>&1 >/dev/null)"
    FAKE_CURL_STATUS=403 analyze_with_fixture "$tmp/forbidden.json" >/dev/null 2>&1
    status=$?
    assert_eq "$status" "2"
    assert_contains "$err" "authorization rejected (HTTP 403)"
    assert_contains "$err" "forbidden"
}

test_interrupted_stream_exits_three_and_reports_conversation() {
    write_interrupted_stream
    local err status
    err="$(analyze_with_fixture "$tmp/interrupted.sse" 2>&1 >/dev/null)"
    analyze_with_fixture "$tmp/interrupted.sse" >/dev/null 2>&1
    status=$?
    # Exit 3 signals "a conversation exists, do not re-run" as distinct from a
    # plain failure, so the cluster is not billed twice for the same analysis.
    assert_eq "$status" "3"
    assert_contains "$err" "conv-orphan"
    assert_contains "$err" "billed twice"
}

test_analyze_requires_diagnostic_and_question() {
    assert_fails "missing --diagnostic was accepted" \
        base_env ESDIAG_KIBANA_URL=https://kb.example ESDIAG_KIBANA_APIKEY=k \
        "$scripts/analyze.sh" --question q
}

# -------------------------------------------------------- freshness parsing --

# Exercise the actual client against recorded Elasticsearch response bodies.
# The fake also records the URL and JSON payload so this guards the direct
# _query boundary rather than a duplicated parser embedded in the tests.
fake_curl_json() {
    local fixture="$1" status="$2"
    cat > "$tmp/bin/curl" <<EOF
#!/usr/bin/env bash
output_file=""
payload=""
url=""
while [ \$# -gt 0 ]; do
    case "\$1" in
        -o) output_file="\$2"; shift 2 ;;
        -d) payload="\$2"; shift 2 ;;
        -w|-H|-X|-m) shift 2 ;;
        http*) url="\$1"; shift ;;
        *) shift ;;
    esac
done
cp "$fixture" "\$output_file"
printf '%s\n%s\n' "\$url" "\$payload" > "$tmp/curl-request"
printf '%s' '$status'
EOF
    chmod +x "$tmp/bin/curl"
}

latest_with_fixture() {
    local fixture="$1" status="$2"; shift 2
    setup_fake_bin
    fake_curl_json "$fixture" "$status"
    PATH="$tmp/bin:$PATH" base_env \
        ESDIAG_KIBANA_URL=https://kb.example \
        ESDIAG_ELASTICSEARCH_URL=https://es.example \
        ESDIAG_KIBANA_APIKEY=k \
        "$scripts/latest-diagnostic.sh" "$@"
}

write_latest_response() {
    cat > "$tmp/latest.json" <<'EOF'
{"is_partial":false,"columns":[{"name":"diagnostic.id"},{"name":"event.ingested"},{"name":"age_minutes"},{"name":"fresh"}],"values":[["d@1~1","2026-08-08T16:56:44.895Z",5,true]]}
EOF
}

test_freshness_uses_direct_esql_query() {
    write_latest_response
    local out request
    out="$(latest_with_fixture "$tmp/latest.json" 200 --window 90m)"
    request="$(cat "$tmp/curl-request")"
    assert_eq "$(printf '%s' "$out" | jq -r .diagnostic_id)" "d@1~1"
    assert_eq "$(printf '%s' "$out" | jq -r .fresh)" "true"
    assert_contains "$request" "https://es.example/_query"
    assert_contains "$request" '"max_age_minutes": 90'
    assert_not_contains "$request" "agent_builder"
    assert_not_contains "$request" "diagnostic.user"
}

test_freshness_reports_not_found_for_empty_values() {
    printf '%s\n' '{"is_partial":false,"columns":[{"name":"diagnostic.id"}],"values":[]}' > "$tmp/empty.json"
    local out
    out="$(latest_with_fixture "$tmp/empty.json" 200)"
    assert_eq "$(printf '%s' "$out" | jq -r .found)" "false"
    assert_eq "$(printf '%s' "$out" | jq -r .fresh)" "false"
}

test_freshness_treats_missing_id_as_unknown() {
    printf '%s\n' '{"is_partial":false,"columns":[{"name":"age_minutes"}],"values":[[0]]}' > "$tmp/no-id.json"
    assert_fails "missing diagnostic.id was treated as not found" \
        latest_with_fixture "$tmp/no-id.json" 200
}

test_freshness_treats_partial_response_as_unknown() {
    printf '%s\n' '{"is_partial":true,"columns":[],"values":[]}' > "$tmp/partial.json"
    assert_fails "partial results were treated as authoritative" \
        latest_with_fixture "$tmp/partial.json" 200
}

test_freshness_surfaces_http_failure() {
    printf '%s\n' '{"error":{"reason":"forbidden"}}' > "$tmp/es-forbidden.json"
    local err status
    err="$(latest_with_fixture "$tmp/es-forbidden.json" 403 2>&1 >/dev/null)"
    latest_with_fixture "$tmp/es-forbidden.json" 403 >/dev/null 2>&1
    status=$?
    assert_eq "$status" "2"
    assert_contains "$err" "HTTP 403"
    assert_contains "$err" "do not infer collection"
}

# --------------------------------------------------------- skill contracts --

# Intent classification and the confirm-before-collecting rule live in the
# skill instructions, so they are judgment, not code. These are structural guards
# that the load-bearing rules have not been dropped; they do not evaluate
# behavior. Behavioral evaluation needs the eval harness.

check_skill() { assert_contains "$(cat "${root}/plugin/skills/$1/SKILL.md")" "$2"; }

test_check_skill_states_all_three_intents() {
    check_skill check '**Reference**'
    check_skill check '**Collection**'
    check_skill check '**Ambiguous**'
}

test_check_skill_requires_asking_before_inferred_collection() {
    check_skill check 'ask before collecting'
}

test_check_skill_treats_followups_as_reference() {
    check_skill check 'or a follow-up'
}

test_check_skill_forbids_collecting_on_unknown_freshness() {
    check_skill check "Exit \`2\`"
    check_skill check 'do not infer collection'
}

test_check_skill_forbids_local_reanalysis() {
    check_skill check 'Do not reproduce its metrics or thresholds locally'
}

test_check_skill_forbids_rerun_after_interruption() {
    check_skill check 'Do not re-run'
}

test_check_skill_states_keystore_is_terminal() {
    check_skill check 'esdiag keystore unlock'
}

test_first_job_guidance_offers_rather_than_falling_back() {
    check_skill check 'offer to create one'
}

test_first_job_guidance_persists_during_first_run() {
    check_skill check '--save-job'
    check_skill check 'as part of its first run'
}

test_first_job_guidance_orders_prerequisites() {
    check_skill check '1. An unlocked keystore'
    check_skill check "2. A saved host with the \`collect\` role"
    check_skill check "3. A saved output host with the \`send\` role"
}

test_first_job_guidance_supports_declining() {
    check_skill check 'do not persist a job or repeat the offer this session'
}

test_connect_skill_separates_failure_classes() {
    check_skill connect '**Client configuration**'
    check_skill connect '**Deployment model prerequisite**'
    check_skill connect '**Cluster provisioning**'
}

test_analysis_contract_preserves_kibana_history() {
    check_skill check 'saved in Kibana Agent Builder history'
    check_skill check 'automatically reuse the conversation associated with this deployment, space, agent, and diagnostic'
    check_skill connect 'conversation-free binding check'
}

# ------------------------------------------------------------------- driver --

run_test() {
    local name="$1" failures_before="$failed" status=0
    if [[ -n "$only" && "$only" != "$name" ]]; then return 0; fi
    "$name" || status=$?
    if [[ "$status" -eq 0 && "$failed" -eq "$failures_before" ]]; then
        passed=$((passed + 1))
        printf '  ok    %s\n' "$name"
    else
        if [[ "$failed" -eq "$failures_before" ]]; then
            fail "$name returned status $status without an assertion message" || true
        fi
        printf '  FAIL  %s\n' "$name"
    fi
}

for test_name in $(declare -F | awk '{print $3}' | grep '^test_' | sort); do
    run_test "$test_name"
done

selected=$((passed + failed))
printf '\n%d passed, %d failed (%d run)\n' "$passed" "$failed" "$selected"
# Guard against a test that returns non-zero without reporting a failure, which
# would otherwise silently vanish from both counts.
if [[ "$failed" -eq 0 && "$selected" -ne "$passed" ]]; then
    printf 'inconsistent counts: some test returned non-zero without calling fail\n' >&2
    exit 1
fi
[[ "$failed" -eq 0 ]]
