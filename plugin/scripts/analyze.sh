#!/usr/bin/env bash

# Copyright Elasticsearch B.V. and/or licensed to Elasticsearch B.V. under one
# or more contributor license agreements. Licensed under the Elastic License 2.0.

# Stream diagnostic analysis from Agent Builder. Every request deliberately
# creates or continues a persisted Kibana conversation; metadata lookups use a
# separate direct-Elasticsearch script and never enter conversation history.

set -uo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=plugin/scripts/config.sh
. "${script_dir}/config.sh"

command_name="${0##*/}"
diagnostic_id=""
question=""
conversation_id=""
force_new=false
state_file="${ESDIAG_HOME:-${HOME}/.esdiag}/plugin-conversations.tsv"

usage() {
    cat <<EOF
Request diagnostic analysis from the configured Agent Builder agent.

Usage: ${command_name} --diagnostic <id> --question <text> [options]

Options:
  --diagnostic <id>    Diagnostic identifier to analyze (required)
  --question <text>    Question to ask (required)
  --conversation <id>  Continue a specific Kibana conversation
  --new                Start a new conversation for this diagnostic
  --help               Show this help

Exit codes:
  0  analysis completed and persisted in Kibana conversation history
  1  usage or configuration error
  2  request failed
  3  stream interrupted after a conversation was created; do not duplicate it
EOF
}

die() { printf '%s: %s\n' "$command_name" "$1" >&2; exit 1; }

while [ $# -gt 0 ]; do
    case "$1" in
        --diagnostic) diagnostic_id="${2:-}"; shift 2 ;;
        --question) question="${2:-}"; shift 2 ;;
        --conversation) conversation_id="${2:-}"; shift 2 ;;
        --new) force_new=true; shift ;;
        --help|-h) usage; exit 0 ;;
        *) die "unknown option: $1" ;;
    esac
done

[ -n "$diagnostic_id" ] || die '--diagnostic is required'
[ -n "$question" ] || die '--question is required'
esdiag_config_resolve || exit 1

lookup_conversation() {
    [ -f "$state_file" ] || return 0
    awk -F '\t' \
        -v base="$ESDIAG_CFG_KIBANA_BASE" \
        -v space="$ESDIAG_CFG_SPACE" \
        -v agent="$ESDIAG_CFG_AGENT_ID" \
        -v diagnostic="$diagnostic_id" \
        '$1 == base && $2 == space && $3 == agent && $4 == diagnostic { id = $5 }
         END { if (id) print id }' "$state_file"
}

remember_conversation() {
    local id="$1" tmp
    mkdir -p "$(dirname "$state_file")"
    tmp="$(mktemp "${state_file}.tmp.XXXXXX")"
    if [ -f "$state_file" ]; then
        awk -F '\t' \
            -v base="$ESDIAG_CFG_KIBANA_BASE" \
            -v space="$ESDIAG_CFG_SPACE" \
            -v agent="$ESDIAG_CFG_AGENT_ID" \
            -v diagnostic="$diagnostic_id" \
            '!(NF >= 5 && $1 == base && $2 == space && $3 == agent && $4 == diagnostic)' \
            "$state_file" > "$tmp"
    fi
    printf '%s\t%s\t%s\t%s\t%s\n' \
        "$ESDIAG_CFG_KIBANA_BASE" "$ESDIAG_CFG_SPACE" "$ESDIAG_CFG_AGENT_ID" \
        "$diagnostic_id" "$id" >> "$tmp"
    mv "$tmp" "$state_file"
    chmod 600 "$state_file" 2>/dev/null || true
}

if [ -z "$conversation_id" ] && [ "$force_new" = false ]; then
    conversation_id="$(lookup_conversation)"
fi

input="$(printf 'Analyze diagnostic %s.\n\n%s' "$diagnostic_id" "$question")"
body="$(jq -n \
    --arg agent "$ESDIAG_CFG_AGENT_ID" \
    --arg input "$input" \
    --arg conversation "$conversation_id" \
    --arg inference "$ESDIAG_CFG_INFERENCE_ID" \
    '{agent_id:$agent,input:$input}
     + (if $conversation == "" then {} else {conversation_id:$conversation} end)
     + (if $inference == "" then {} else {inference_id:$inference} end)')"
url="$(esdiag_config_url 'api/agent_builder/converse/async')"

if [ -n "$conversation_id" ]; then
    printf 'Continuing Kibana conversation %s for diagnostic %s\n' "$conversation_id" "$diagnostic_id" >&2
else
    printf 'Starting Kibana conversation for diagnostic %s (agent: %s)\n' "$diagnostic_id" "$ESDIAG_CFG_AGENT_ID" >&2
fi

message_file="$(mktemp)"
usage_file="$(mktemp)"
header_file="$(mktemp)"
curl_error_file="$(mktemp)"
raw_body_file="$(mktemp)"
trap 'rm -f "$message_file" "$usage_file" "$header_file" "$curl_error_file" "$raw_body_file"' EXIT

event=""
completed=false
stream_error=""

while IFS= read -r line; do
    case "$line" in
        ':'*) continue ;;
        'event: '*) event="${line#event: }"; continue ;;
        'data: '*) data="${line#data: }" ;;
        *) printf '%s\n' "$line" >> "$raw_body_file"; continue ;;
    esac

    case "$event" in
        conversation_id_set)
            id="$(printf '%s' "$data" | jq -r '.data.conversation_id // empty' 2>/dev/null)"
            [ -n "$id" ] && conversation_id="$id"
            ;;
        reasoning)
            text="$(printf '%s' "$data" | jq -r '.data.reasoning // empty' 2>/dev/null)"
            [ -n "$text" ] && printf '  … %s\n' "$text" >&2
            ;;
        tool_call)
            tool="$(printf '%s' "$data" | jq -r '.data.tool_id // empty' 2>/dev/null)"
            [ -n "$tool" ] && printf '  → %s\n' "$tool" >&2
            ;;
        tool_progress)
            text="$(printf '%s' "$data" | jq -r '.data.message // empty' 2>/dev/null)"
            [ -n "$text" ] && printf '    %s\n' "$text" >&2
            ;;
        tool_result)
            tool="$(printf '%s' "$data" | jq -r '.data.tool_id // empty' 2>/dev/null)"
            [ -n "$tool" ] && printf '  ✓ %s\n' "$tool" >&2
            ;;
        message_complete)
            printf '%s' "$data" | jq -r '.data.message_content // empty' > "$message_file" 2>/dev/null
            completed=true
            ;;
        round_complete)
            printf '%s' "$data" | jq -c '.data.round.model_usage // empty' > "$usage_file" 2>/dev/null
            ;;
        error) stream_error="$data" ;;
    esac
done < <(curl -sSN --connect-timeout 15 --max-time 600 \
    --dump-header "$header_file" --stderr "$curl_error_file" \
    -X POST "$url" \
    -H "Authorization: ApiKey ${ESDIAG_CFG_APIKEY}" \
    -H 'kbn-xsrf: true' \
    -H 'Content-Type: application/json' \
    -d "$body")

http_status="$(awk '/^HTTP\// { status = $2 } END { print status }' "$header_file")"
[ -n "$http_status" ] || http_status="000"

if [ "$http_status" != "200" ]; then
    response_message="$(jq -r '(.message // .error.message // .error // empty) | tostring' "$raw_body_file" 2>/dev/null)"
    [ -n "$response_message" ] || response_message="$(head -c 400 "$curl_error_file")"
    case "$http_status" in
        401|403)
            printf '%s: Agent Builder authorization rejected (HTTP %s). Check feature_agentBuilder.read, feature_actions.read, and API key scope.\n' "$command_name" "$http_status" >&2
            ;;
        404)
            printf '%s: Agent Builder endpoint or configured agent is unavailable (HTTP 404). Run /esdiag:connect.\n' "$command_name" >&2
            ;;
        400)
            case "$response_message" in
                *model*|*Model*|*connector*|*Connector*|*inference*|*Inference*)
                    printf '%s: the deployment has no usable model for this agent. Configure Elastic Inference Service or an LLM connector.\n' "$command_name" >&2
                    ;;
                *agent*not*found*|*Agent*not*found*)
                    printf '%s: configured agent is missing. Run /esdiag:connect and set ESDIAG_AGENT_ID.\n' "$command_name" >&2
                    ;;
                *) printf '%s: analysis request rejected (HTTP 400).\n' "$command_name" >&2 ;;
            esac
            ;;
        000) printf '%s: could not reach Agent Builder at %s.\n' "$command_name" "$ESDIAG_CFG_KIBANA_BASE" >&2 ;;
        *) printf '%s: Agent Builder request failed (HTTP %s).\n' "$command_name" "$http_status" >&2 ;;
    esac
    [ -n "$response_message" ] && printf '  %s\n' "$response_message" >&2
    exit 2
fi

[ -n "$conversation_id" ] && remember_conversation "$conversation_id"

if [ "$completed" != true ]; then
    if [ -n "$stream_error" ]; then
        printf '%s: analysis failed: %s\n' "$command_name" "$stream_error" >&2
    else
        printf '%s: stream ended before analysis completed.\n' "$command_name" >&2
        [ -s "$curl_error_file" ] && head -c 400 "$curl_error_file" >&2
    fi
    if [ -n "$conversation_id" ]; then
        printf 'Conversation %s remains in Kibana Agent Builder history. Do not re-run it or the deployment may be billed twice.\n' "$conversation_id" >&2
        exit 3
    fi
    exit 2
fi

sed -e "s|](<\\/|](<${ESDIAG_CFG_KIBANA_BASE}/|g" \
    -e "s|](\\/s\\/|](${ESDIAG_CFG_KIBANA_BASE}/s/|g" "$message_file"

if [ -s "$usage_file" ]; then
    printf '\n---\nCluster inference usage: %s\n' "$(cat "$usage_file")" >&2
fi
printf 'Kibana Agent Builder conversation: %s\n' "$conversation_id" >&2
