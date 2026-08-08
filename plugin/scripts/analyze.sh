#!/usr/bin/env bash

# Copyright Elasticsearch B.V. and/or licensed to Elasticsearch B.V. under one
# or more contributor license agreements. Licensed under the Elastic License 2.0.

# Request diagnostic analysis from the deployment's Agent Builder agent.
#
# The analysis runs on the deployment's model, so its token cost lands on the
# cluster's inference connector rather than locally. Progress is reported from
# the streaming event feed; the final markdown is written to stdout unmodified
# apart from resolving relative dashboard links.

set -uo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./config.sh
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
  --conversation <id>  Continue a specific conversation
  --new                Start a new conversation even if one exists for this diagnostic
  --help               Show this help

Exit codes:
  0  analysis completed
  1  usage or configuration error
  2  request failed
  3  stream interrupted before completion (conversation id reported)
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

[ -n "$diagnostic_id" ] || die "--diagnostic is required"
[ -n "$question" ] || die "--question is required"

esdiag_config_resolve || exit 1

# Conversation continuity is keyed by diagnostic, so a different diagnostic
# naturally starts a new conversation rather than inheriting stale context.
lookup_conversation() {
    [ -f "$state_file" ] || return 0
    awk -F'\t' -v d="$diagnostic_id" '$1 == d { id = $2 } END { if (id) print id }' "$state_file"
}

remember_conversation() {
    local id="$1" tmp
    mkdir -p "$(dirname "$state_file")"
    tmp="$(mktemp)"
    if [ -f "$state_file" ]; then
        awk -F'\t' -v d="$diagnostic_id" '$1 != d' "$state_file" > "$tmp"
    fi
    printf '%s\t%s\n' "$diagnostic_id" "$id" >> "$tmp"
    mv "$tmp" "$state_file"
    chmod 600 "$state_file" 2>/dev/null || true
}

if [ -z "$conversation_id" ] && [ "$force_new" = false ]; then
    conversation_id="$(lookup_conversation)"
fi

# Name the diagnostic explicitly so the agent verifies the intended one rather
# than inferring which diagnostic is current.
input="$(printf 'Analyze diagnostic %s.\n\n%s' "$diagnostic_id" "$question")"

body="$(jq -n \
    --arg agent "$ESDIAG_CFG_AGENT_ID" \
    --arg input "$input" \
    --arg conversation "$conversation_id" \
    --arg inference "$ESDIAG_CFG_INFERENCE_ID" \
    '{agent_id: $agent, input: $input}
     + (if $conversation == "" then {} else {conversation_id: $conversation} end)
     + (if $inference == "" then {} else {inference_id: $inference} end)')"

url="$(esdiag_config_url 'api/agent_builder/converse/async')"

if [ -n "$conversation_id" ]; then
    printf 'Continuing conversation %s for diagnostic %s\n' "$conversation_id" "$diagnostic_id" >&2
else
    printf 'Analyzing diagnostic %s on %s (agent: %s)\n' "$diagnostic_id" "$ESDIAG_CFG_KIBANA_BASE" "$ESDIAG_CFG_AGENT_ID" >&2
fi

message_file="$(mktemp)"
usage_file="$(mktemp)"
trap 'rm -f "$message_file" "$usage_file"' EXIT

event=""
completed=false
http_error=""

# Process substitution keeps the loop in this shell, so state set here survives.
while IFS= read -r line; do
    case "$line" in
        ':'*) continue ;;                       # keep-alive padding
        'event: '*) event="${line#event: }"; continue ;;
        'data: '*) : ;;
        *) continue ;;
    esac
    data="${line#data: }"

    case "$event" in
        conversation_id_set)
            id="$(printf '%s' "$data" | jq -r '.data.conversation_id // empty' 2>/dev/null)"
            if [ -n "$id" ] && [ "$id" != "$conversation_id" ]; then
                conversation_id="$id"
            fi
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
            # Report completion only. Results are the agent's to interpret, and
            # parsing them here would re-derive analysis locally.
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
        error)
            http_error="$data"
            ;;
    esac
done < <(curl -sSN -X POST "$url" \
    -H "Authorization: ApiKey ${ESDIAG_CFG_APIKEY}" \
    -H 'kbn-xsrf: true' \
    -H 'Content-Type: application/json' \
    -d "$body" 2>/dev/null)

[ -n "$conversation_id" ] && remember_conversation "$conversation_id"

if [ "$completed" != true ]; then
    if [ -n "$http_error" ]; then
        printf '%s: analysis failed: %s\n' "$command_name" "$http_error" >&2
    else
        printf '%s: stream ended before the analysis completed.\n' "$command_name" >&2
    fi
    if [ -n "$conversation_id" ]; then
        printf 'Conversation %s was started. Ask to check it rather than re-running, or the request is paid for twice.\n' "$conversation_id" >&2
        exit 3
    fi
    exit 2
fi

# Resolve ADA's relative dashboard links against the configured Kibana URL.
# Content is otherwise passed through untouched.
sed -e "s|](<\\/|](<${ESDIAG_CFG_KIBANA_BASE}/|g" -e "s|](\\/s\\/|](${ESDIAG_CFG_KIBANA_BASE}/s/|g" "$message_file"

if [ -s "$usage_file" ]; then
    printf '\n---\nCluster inference usage: %s\n' "$(cat "$usage_file")" >&2
fi
printf 'Conversation: %s\n' "$conversation_id" >&2
exit 0
