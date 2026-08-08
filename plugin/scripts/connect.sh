#!/usr/bin/env bash

# Copyright Elasticsearch B.V. and/or licensed to Elasticsearch B.V. under one
# or more contributor license agreements. Licensed under the Elastic License 2.0.

# Bind this workstation to an already-provisioned ESDiag deployment.
#
# This command is read-only against the deployment. It never starts, creates, or
# reconfigures anything, and never requires a container runtime. Provisioning a
# cluster is a separate concern.

set -uo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./config.sh
. "${script_dir}/config.sh"

failures=0
warnings=0
model_check=true

# The model probe issues one real request, which costs a small number of tokens
# on the deployment. Allow skipping it for a zero-cost run.
while [ $# -gt 0 ]; do
    case "$1" in
        --no-model-check) model_check=false; shift ;;
        --help|-h)
            printf 'Usage: connect.sh [--no-model-check]\n\n'
            printf '  --no-model-check  Skip the model availability probe (costs no tokens)\n'
            exit 0
            ;;
        *) printf 'connect.sh: unknown option: %s\n' "$1" >&2; exit 1 ;;
    esac
done

ok()   { printf '  ok    %s\n' "$1"; }
bad()  { printf '  FAIL  %s\n' "$1"; failures=$((failures + 1)); }
warn() { printf '  warn  %s\n' "$1"; warnings=$((warnings + 1)); }
note() { printf '        %s\n' "$1"; }
head_() { printf '\n%s\n' "$1"; }

api_status=""
api_body_file="$(mktemp)"
trap 'rm -f "$api_body_file"' EXIT

# Status must be set in the parent shell, so the body goes to a file rather than
# through command substitution, which would run these in a subshell.
api() {
    local url
    url="$(esdiag_config_url "$1")"
    api_status="$(curl -sS -m 30 -o "$api_body_file" -w '%{http_code}' \
        "$url" -H "Authorization: ApiKey ${ESDIAG_CFG_APIKEY}" 2>/dev/null)" || api_status="000"
}

esql() {
    # Run a read-only ES|QL query through the tool execution endpoint.
    local query="$1" url payload
    url="$(esdiag_config_url 'api/agent_builder/tools/_execute')"
    payload="$(printf '{"tool_id":"platform.core.execute_esql","tool_params":{"query":%s}}' "$(printf '%s' "$query" | jq -Rs .)")"
    api_status="$(curl -sS -m 60 -o "$api_body_file" -w '%{http_code}' -X POST "$url" \
        -H "Authorization: ApiKey ${ESDIAG_CFG_APIKEY}" \
        -H 'kbn-xsrf: true' -H 'Content-Type: application/json' \
        -d "$payload" 2>/dev/null)" || api_status="000"
}

printf 'ESDiag client binding\n'

# ---- Configuration ---------------------------------------------------------
head_ 'Configuration'
if ! esdiag_config_resolve; then
    printf '\nBinding incomplete: configuration could not be resolved.\n' >&2
    exit 1
fi
ok "Kibana ${ESDIAG_CFG_KIBANA_BASE}"
ok "space ${ESDIAG_CFG_SPACE:-<default>}"
ok "agent ${ESDIAG_CFG_AGENT_ID}"
[ -n "$ESDIAG_CFG_INFERENCE_ID" ] && ok "inference ${ESDIAG_CFG_INFERENCE_ID}" || note "inference routing unset; the agent uses its own model"
[ -n "$ESDIAG_CFG_JOB" ] && ok "saved job ${ESDIAG_CFG_JOB}" || note "no saved job configured; the first review will offer to create one"

# ---- Reachability and authorization ---------------------------------------
head_ 'Deployment'
# Version is worth reporting because Agent Builder surface area moves between
# releases, so a support conversation starts from a known version.
api 'api/stats'
if [ "$api_status" = "200" ]; then
    ok "Kibana $(jq -r '.kibana.version // "unknown"' "$api_body_file" 2>/dev/null) ($(jq -r '.kibana.status // "unknown"' "$api_body_file" 2>/dev/null))"
fi

api 'api/agent_builder/agents'
agents_reachable=false
case "$api_status" in
    200) ok 'Kibana reachable and API key accepted'; agents_reachable=true; cp "$api_body_file" "${api_body_file}.agents" ;;
    401|403)
        bad "API key rejected for Agent Builder (HTTP ${api_status})"
        note 'Needs feature_agentBuilder.read and feature_actions.read on this space.'
        note 'This is client configuration.'
        ;;
    000|"")
        bad "Kibana unreachable at ${ESDIAG_CFG_KIBANA_BASE}"
        note 'If no deployment exists yet, provision one first. This command does not create deployments.'
        ;;
    404)
        bad "Agent Builder not available at this path (HTTP 404)"
        note 'Check the space, and that Agent Builder is enabled on this deployment.'
        ;;
    *) bad "Unexpected response from Agent Builder (HTTP ${api_status})" ;;
esac

# ---- Configured agent exists ----------------------------------------------
if [ "$agents_reachable" = true ]; then
    if jq -e --arg id "$ESDIAG_CFG_AGENT_ID" '[(.results//.)[]?|.id]|index($id)' "${api_body_file}.agents" >/dev/null 2>&1; then
        ok "agent '${ESDIAG_CFG_AGENT_ID}' exists in this space"
    else
        bad "agent '${ESDIAG_CFG_AGENT_ID}' not found in space '${ESDIAG_CFG_SPACE}'"
        note 'This is client configuration. Available agents:'
        jq -r '(.results//.)[]?|"          \(.id)  (\(.name))"' "${api_body_file}.agents" 2>/dev/null
        note 'Set ESDIAG_AGENT_ID to the agent carrying the diagnostic skill.'
    fi
fi

# ---- Model availability (deployment prerequisite) -------------------------
# Enumerating models does not work from a Kibana URL alone. Elastic Inference
# Service models are Elasticsearch inference endpoints, not Kibana action
# connectors, and no Kibana route exposes them. So probe the actual capability
# with one minimal request instead of inferring it from a listing.
if [ "$agents_reachable" = true ] && [ "$model_check" = true ]; then
    url="$(esdiag_config_url 'api/agent_builder/converse')"
    probe_status="$(curl -sS -m 120 -o "$api_body_file" -w '%{http_code}' -X POST "$url" \
        -H "Authorization: ApiKey ${ESDIAG_CFG_APIKEY}" \
        -H 'kbn-xsrf: true' -H 'Content-Type: application/json' \
        -d "$(jq -n --arg a "$ESDIAG_CFG_AGENT_ID" '{agent_id:$a,input:"Reply with exactly: OK"}')" 2>/dev/null)" || probe_status="000"

    probe_error="$(jq -r '(.message // .error.message // .error // "") | tostring' "$api_body_file" 2>/dev/null)"
    if [ "$probe_status" = "200" ] && [ "$(jq -r '.status // ""' "$api_body_file" 2>/dev/null)" = "completed" ]; then
        ok "model reachable ($(jq -r '.model_usage.model // "unknown"' "$api_body_file" 2>/dev/null))"
        note "probe cost $(jq -r '.model_usage.input_tokens // 0' "$api_body_file" 2>/dev/null) input tokens on the deployment"
    else
        case "$probe_error" in
            *onnector*|*odel*|*nference*|*LLM*)
                bad 'the agent has no usable model'
                note "${probe_error}"
                note 'This is a deployment prerequisite, not an ESDiag or client problem.'
                note 'Activate the Elastic Inference Service through Cloud Connect, or configure'
                note 'an LLM provider and connector. No esdiag command provisions this.'
                ;;
            *)
                bad "model probe failed (HTTP ${probe_status})"
                [ -n "$probe_error" ] && note "${probe_error}"
                ;;
        esac
    fi
fi

# ---- Diagnostic data access ------------------------------------------------
# Agent Builder tools query Elasticsearch as the calling identity, so a key that
# can chat but cannot read the data yields empty analysis rather than an error.
if [ "$agents_reachable" = true ]; then
    head_ 'Diagnostic data access'
    for pattern in 'metrics-diagnostic-esdiag*' 'settings-node-esdiag*'; do
        esql "FROM ${pattern} | LIMIT 1"
        if [ "$api_status" != "200" ]; then
            bad "cannot query ${pattern} (HTTP ${api_status})"
            note 'This is client configuration: the API key needs read and view_index_metadata'
            note 'on metrics-*-esdiag* and settings-*-esdiag*.'
        elif jq -e '[.results[]?|select(.type=="esql_results")]|length>0' "$api_body_file" >/dev/null 2>&1; then
            ok "readable ${pattern}"
        else
            bad "no readable data for ${pattern}"
            note 'The key reached the API but returned no result. Verify index privileges.'
        fi
    done
fi

# ---- Local esdiag state ----------------------------------------------------
head_ 'Local esdiag'
if command -v esdiag >/dev/null 2>&1; then
    ok "esdiag on PATH ($(esdiag --version 2>/dev/null | head -1))"

    keystore_status="$(esdiag keystore status 2>&1 | head -1)"
    case "$keystore_status" in
        *unlocked*) ok "$keystore_status" ;;
        *locked*)   warn "$keystore_status"
                    note 'Unlock with: esdiag keystore unlock' ;;
        *)          warn "keystore status: ${keystore_status}" ;;
    esac

    host_count="$(esdiag host list 2>/dev/null | grep -c . || printf '0')"
    if [ "${host_count:-0}" -gt 0 ]; then
        ok "${host_count} saved host(s)"
    else
        warn 'no saved hosts; collection will need one'
        note 'The first review will offer to help configure a host and job.'
    fi
else
    bad 'esdiag not found on PATH'
    note 'Install the esdiag CLI to collect and process diagnostics.'
fi

# ---- Result ----------------------------------------------------------------
printf '\n'
if [ "$failures" -gt 0 ]; then
    printf 'Binding incomplete: %d problem(s), %d warning(s).\n' "$failures" "$warnings"
    exit 1
fi
if [ "$warnings" -gt 0 ]; then
    printf 'Binding usable with %d warning(s).\n' "$warnings"
    exit 0
fi
printf 'Binding complete.\n'
exit 0
