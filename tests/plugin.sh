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
        -u ESDIAG_JOB -u ESDIAG_DIAGNOSTIC_MAX_AGE -u ESDIAG_USER \
        ESDIAG_HOME="$tmp/home" "$@"
}

cfg() { base_env "$@" "$scripts/config.sh" --json; }

# ---------------------------------------------------------------- packaging --

test_bundled_skill_matches_source() {
    "${root}/bin/sync-plugin-skill.sh" --check >/dev/null || fail "bundled skill drifted from .agents/skills/esdiag"
}

test_sync_detects_drift() {
    local backup="$tmp/skill-backup"
    cp -R "${root}/plugin/skills/esdiag" "$backup"
    printf '\ndrift\n' >> "${root}/plugin/skills/esdiag/SKILL.md"
    if "${root}/bin/sync-plugin-skill.sh" --check >/dev/null 2>&1; then
        rm -rf "${root}/plugin/skills/esdiag"; cp -R "$backup" "${root}/plugin/skills/esdiag"
        fail "drift was not detected"
        return 1
    fi
    rm -rf "${root}/plugin/skills/esdiag"; cp -R "$backup" "${root}/plugin/skills/esdiag"
}

test_bundled_skill_carries_provenance_banner() {
    assert_contains "$(cat "${root}/plugin/skills/esdiag/SKILL.md")" "Generated from .agents/skills/esdiag/"
}

test_plugin_manifest_is_valid_json() {
    jq -e . "${root}/plugin/.claude-plugin/plugin.json" >/dev/null || fail "plugin.json is not valid JSON"
    jq -e . "${root}/.claude-plugin/marketplace.json" >/dev/null || fail "marketplace.json is not valid JSON"
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
    assert_contains "$err" "paid for twice"
}

test_analyze_requires_diagnostic_and_question() {
    assert_fails "missing --diagnostic was accepted" \
        base_env ESDIAG_KIBANA_URL=https://kb.example ESDIAG_KIBANA_APIKEY=k \
        "$scripts/analyze.sh" --question q
}

# -------------------------------------------------------- freshness parsing --

# ES|QL omits columns for fields that do not exist, so a column named in KEEP
# may be absent from the response. diagnostic.user is absent whenever no user
# metadata was attached at process time, which is the common case.
freshness_jq() {
    jq -c --arg window "24h" --argjson scoped "null" '
        def colval($cols; $row; $name):
            ($cols | index($name)) as $i
            | if $i == null then null else $row[$i] end;
        [.results[]? | select(.type == "esql_results") | .data] as $r
        | if ($r | length) == 0 or (($r[0].values // []) | length) == 0 then
            {found: false, window: $window, scoped_to: $scoped}
          else
            ($r[0].columns | map(.name)) as $cols
            | ($r[0].values[0]) as $row
            | colval($cols; $row; "diagnostic.id") as $id
            | if $id == null then {found: false, window: $window, scoped_to: $scoped}
              else {found: true, window: $window, scoped_to: $scoped,
                    diagnostic_id: $id,
                    ingested: colval($cols; $row; "event.ingested"),
                    user: colval($cols; $row; "diagnostic.user"),
                    age_minutes: colval($cols; $row; "age_minutes")}
              end
          end'
}

test_freshness_tolerates_missing_user_column() {
    local out
    out="$(printf '%s' '{"results":[{"type":"esql_results","data":{"columns":[{"name":"diagnostic.id"},{"name":"event.ingested"},{"name":"age_minutes"}],"values":[["d@1~1","2026-08-08T16:56:44.895Z",0]]}}]}' | freshness_jq)"
    assert_eq "$(printf '%s' "$out" | jq -r .found)" "true"
    assert_eq "$(printf '%s' "$out" | jq -r .diagnostic_id)" "d@1~1"
    assert_eq "$(printf '%s' "$out" | jq -r .user)" "null"
}

test_freshness_reports_not_found_for_empty_values() {
    local out
    out="$(printf '%s' '{"results":[{"type":"esql_results","data":{"columns":[{"name":"diagnostic.id"}],"values":[]}}]}' | freshness_jq)"
    assert_eq "$(printf '%s' "$out" | jq -r .found)" "false"
}

test_freshness_reports_not_found_when_id_column_absent() {
    local out
    out="$(printf '%s' '{"results":[{"type":"esql_results","data":{"columns":[{"name":"age_minutes"}],"values":[[0]]}}]}' | freshness_jq)"
    assert_eq "$(printf '%s' "$out" | jq -r .found)" "false"
}

# ------------------------------------------------------- command contracts --

# Intent classification and the confirm-before-collecting rule live in the
# command prompts, so they are judgment, not code. These are structural guards
# that the load-bearing rules have not been dropped; they do not evaluate
# behavior. Behavioral evaluation needs the eval harness.

check_md() { assert_contains "$(cat "${root}/plugin/commands/$1")" "$2"; }

test_check_command_states_all_three_intents() {
    check_md check.md '**Reference**'
    check_md check.md '**Collection**'
    check_md check.md '**Ambiguous**'
}

test_check_command_requires_asking_before_inferred_collection() {
    check_md check.md 'ask before collecting'
}

test_check_command_treats_followups_as_reference() {
    check_md check.md 'any follow-up in an ongoing analysis'
}

test_check_command_forbids_collecting_on_unknown_freshness() {
    check_md check.md 'Exit code 2'
    check_md check.md 'Do not collect on the assumption that nothing exists'
}

test_check_command_forbids_local_reanalysis() {
    check_md check.md 'Do not reproduce it locally'
}

test_check_command_forbids_rerun_after_interruption() {
    check_md check.md 'Do not re-run'
}

test_check_command_states_keystore_is_terminal() {
    check_md check.md 'esdiag keystore unlock'
}

test_first_job_command_offers_rather_than_falling_back() {
    check_md first-job.md 'Offer it — do not fall back to an ad-hoc collect without saying so'
}

test_first_job_command_persists_during_first_run() {
    check_md first-job.md '--save-job'
    check_md first-job.md 'Do not configure and then run separately'
}

test_first_job_command_orders_prerequisites() {
    check_md first-job.md '1. Keystore access'
    check_md first-job.md '2. A host to collect from'
    check_md first-job.md '3. An output target to send to'
}

test_first_job_command_supports_declining() {
    check_md first-job.md 'do not ask again this session'
}

test_connect_command_separates_failure_classes() {
    check_md connect.md '**Client configuration**'
    check_md connect.md '**Deployment prerequisite**'
    check_md connect.md '**Cluster provisioning**'
}

# ------------------------------------------------------------------- driver --

run_test() {
    local name="$1"
    if [[ -n "$only" && "$only" != "$name" ]]; then return 0; fi
    if "$name"; then
        passed=$((passed + 1))
        printf '  ok    %s\n' "$name"
    else
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
