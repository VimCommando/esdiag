#!/usr/bin/env bash

# Copyright Elasticsearch B.V. and/or licensed to Elasticsearch B.V. under one
# or more contributor license agreements. Licensed under the Elastic License 2.0.

# Validate client binding without creating deployments, Agent Builder
# conversations, or inference spend.

set -uo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source-path=SCRIPTDIR
. "${script_dir}/config.sh"

failures=0
warnings=0
api_status=""
api_body_file="$(mktemp)"
trap 'rm -f "$api_body_file"' EXIT

ok() { printf '  ok    %s\n' "$1"; }
bad() { printf '  FAIL  %s\n' "$1"; failures=$((failures + 1)); }
warn() { printf '  warn  %s\n' "$1"; warnings=$((warnings + 1)); }
note() { printf '        %s\n' "$1"; }
head_() { printf '\n%s\n' "$1"; }

usage() {
    printf 'Usage: connect.sh\n\n'
    printf 'Checks configuration, API authorization, diagnostic data access, and local esdiag state.\n'
    printf 'It creates no deployment resources or Agent Builder conversations.\n'
}

case "${1:-}" in
    --help|-h) usage; exit 0 ;;
    "") ;;
    *) printf 'connect.sh: unknown option: %s\n' "$1" >&2; exit 1 ;;
esac

kibana_get() {
    local url
    url="$(esdiag_config_url "$1")"
    api_status="$(curl -sS -m 30 -o "$api_body_file" -w '%{http_code}' \
        "$url" -H "Authorization: ApiKey ${ESDIAG_CFG_APIKEY}" 2>/dev/null)" || api_status="000"
}

elasticsearch_esql() {
    local query="$1"
    api_status="$(curl -sS -m 60 -o "$api_body_file" -w '%{http_code}' -X POST \
        "${ESDIAG_CFG_ELASTICSEARCH_BASE}/_query" \
        -H "Authorization: ApiKey ${ESDIAG_CFG_APIKEY}" \
        -H 'Content-Type: application/json' \
        -d "$(jq -n --arg q "$query" '{query:$q}')" 2>/dev/null)" || api_status="000"
}

printf 'ESDiag client binding\n'
head_ 'Configuration'
if ! esdiag_config_resolve; then
    printf '\nBinding incomplete: configuration could not be resolved.\n' >&2
    exit 1
fi

ok "Kibana ${ESDIAG_CFG_KIBANA_BASE}"
ok "space ${ESDIAG_CFG_SPACE:-<default>}"
ok "agent ${ESDIAG_CFG_AGENT_ID}"
if [ -n "$ESDIAG_CFG_INFERENCE_ID" ]; then
    ok "inference ${ESDIAG_CFG_INFERENCE_ID}"
else
    note 'inference routing unset; the agent uses its configured model'
fi
if [ -n "$ESDIAG_CFG_JOB" ]; then
    ok "saved job ${ESDIAG_CFG_JOB}"
else
    note 'no saved job configured; collection will offer guided setup'
fi
if esdiag_config_require_elasticsearch; then
    ok "Elasticsearch ${ESDIAG_CFG_ELASTICSEARCH_BASE}"
else
    failures=$((failures + 1))
fi

head_ 'Agent Builder'
kibana_get 'api/agent_builder/agents'
case "$api_status" in
    200)
        ok 'Kibana reachable and API key accepted'
        if jq -e --arg id "$ESDIAG_CFG_AGENT_ID" '[(.results // .)[]? | .id] | index($id)' "$api_body_file" >/dev/null 2>&1; then
            ok "agent '${ESDIAG_CFG_AGENT_ID}' exists in this space"
        else
            bad "agent '${ESDIAG_CFG_AGENT_ID}' not found in space '${ESDIAG_CFG_SPACE}'"
            note 'Available agents:'
            jq -r '(.results // .)[]? | "          \(.id)  (\(.name))"' "$api_body_file" 2>/dev/null
        fi
        ;;
    401|403)
        bad "API key rejected for Agent Builder (HTTP ${api_status})"
        note 'Grant feature_agentBuilder.read and feature_actions.read in this space.'
        ;;
    404)
        bad 'Agent Builder is unavailable at the configured space path'
        note 'Provision the deployment and install ESDiag assets before binding.'
        ;;
    000|"") bad "Kibana unreachable at ${ESDIAG_CFG_KIBANA_BASE}" ;;
    *) bad "unexpected Agent Builder response (HTTP ${api_status})" ;;
esac

if [ -n "$ESDIAG_CFG_ELASTICSEARCH_BASE" ]; then
    head_ 'Diagnostic data access'
    for pattern in 'metrics-*-esdiag*' 'settings-*-esdiag*'; do
        elasticsearch_esql "FROM ${pattern} | LIMIT 1"
        es_error="$(jq -r '(.error.reason // .error.root_cause[0].reason // .message // .error // "") | tostring' \
            "$api_body_file" 2>/dev/null)"
        case "$api_status" in
            200) ok "readable ${pattern}" ;;
            401|403)
                bad "API key cannot query ${pattern} through Elasticsearch _query (HTTP ${api_status})"
                note 'Client configuration: grant read and view_index_metadata on the ESDiag data streams.'
                ;;
            400|404)
                case "$es_error" in
                    *Unknown*index*|*unknown*index*|*no\ such\ index*|*No\ matching\ indices*)
                        bad "no ESDiag data stream matches ${pattern}"
                        note 'Cluster provisioning: run esdiag setup and process a diagnostic before binding.'
                        ;;
                    *)
                        bad "Elasticsearch rejected the ${pattern} access check (HTTP ${api_status})"
                        [ -n "$es_error" ] && note "$es_error"
                        ;;
                esac
                ;;
            000|"") bad "Elasticsearch unreachable at ${ESDIAG_CFG_ELASTICSEARCH_BASE}" ;;
            *)
                bad "unexpected Elasticsearch response for ${pattern} (HTTP ${api_status})"
                [ -n "$es_error" ] && note "$es_error"
                ;;
        esac
    done
fi

head_ 'Local collection tooling'
if command -v esdiag >/dev/null 2>&1; then
    ok "esdiag on PATH ($(esdiag --version 2>/dev/null | head -1))"
    keystore_status="$(esdiag keystore status 2>&1 | head -1)"
    case "$keystore_status" in
        *unlocked*) ok "$keystore_status" ;;
        *locked*) warn "$keystore_status"; note 'Unlock before collection with: esdiag keystore unlock' ;;
        *) warn "keystore status: ${keystore_status}" ;;
    esac
    host_count="$(esdiag host list 2>/dev/null | grep -c . || true)"
    if [ "${host_count:-0}" -gt 0 ]; then
        ok "${host_count} saved host(s)"
    else
        warn 'no saved hosts; collecting a new diagnostic will require setup'
    fi
else
    warn 'esdiag not found on PATH; existing diagnostics can still be analyzed'
    note 'Install esdiag only when this machine must collect or process diagnostics.'
fi

printf '\n'
if [ "$failures" -gt 0 ]; then
    printf 'Binding incomplete: %d problem(s), %d warning(s).\n' "$failures" "$warnings"
    exit 1
fi
if [ "$warnings" -gt 0 ]; then
    printf 'Binding usable with %d warning(s).\n' "$warnings"
    exit 0
fi
printf 'Binding complete. No conversations or model calls were created.\n'
