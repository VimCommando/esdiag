#!/usr/bin/env bash

# Copyright Elasticsearch B.V. and/or licensed to Elasticsearch B.V. under one
# or more contributor license agreements. Licensed under the Elastic License 2.0.

# Shared configuration for the ESDiag Claude Code plugin. Source this file from
# another plugin script, or run it with --json to inspect the resolved settings.

ESDIAG_AGENT_ID_DEFAULT="elastic-ai-agent"
ESDIAG_KIBANA_SPACE_DEFAULT="esdiag"
ESDIAG_DIAGNOSTIC_MAX_AGE_DEFAULT="24h"

ESDIAG_CFG_KIBANA_BASE=""
ESDIAG_CFG_ELASTICSEARCH_BASE=""
ESDIAG_CFG_SPACE=""
ESDIAG_CFG_AGENT_ID=""
ESDIAG_CFG_INFERENCE_ID=""
ESDIAG_CFG_JOB=""
ESDIAG_CFG_MAX_AGE=""
ESDIAG_CFG_APIKEY=""

esdiag_config_error() {
    printf 'esdiag-plugin: %s\n' "$1" >&2
    return 1
}

esdiag_config_split_space() {
    local url="$1"
    url="${url%/}"
    case "$url" in
        */s/*)
            ESDIAG_CFG_SPACE_FROM_URL="${url##*/s/}"
            case "$ESDIAG_CFG_SPACE_FROM_URL" in
                */*) ESDIAG_CFG_SPACE_FROM_URL="" ;;
                *) url="${url%/s/*}" ;;
            esac
            ;;
        *) ESDIAG_CFG_SPACE_FROM_URL="" ;;
    esac
    ESDIAG_CFG_KIBANA_BASE="$url"
}

esdiag_config_read_apikey() {
    if [ -n "${ESDIAG_KIBANA_APIKEY:-}" ]; then
        ESDIAG_CFG_APIKEY="$ESDIAG_KIBANA_APIKEY"
    elif [ -n "${ESDIAG_KIBANA_APIKEY_FILE:-}" ]; then
        if [ ! -r "$ESDIAG_KIBANA_APIKEY_FILE" ]; then
            esdiag_config_error "API key file not readable: ${ESDIAG_KIBANA_APIKEY_FILE}"
            return 1
        fi
        ESDIAG_CFG_APIKEY="$(tr -d '\r\n' < "$ESDIAG_KIBANA_APIKEY_FILE")"
    else
        ESDIAG_CFG_APIKEY=""
    fi
}

esdiag_config_resolve() {
    local require_key="${1:-require-key}"

    if [ -z "${ESDIAG_KIBANA_URL:-}" ]; then
        esdiag_config_error "ESDIAG_KIBANA_URL is not set. Run /esdiag:connect to bind this machine."
        return 1
    fi

    esdiag_config_split_space "$ESDIAG_KIBANA_URL"
    if [ -n "${ESDIAG_KIBANA_SPACE+x}" ]; then
        ESDIAG_CFG_SPACE="$ESDIAG_KIBANA_SPACE"
    elif [ -n "$ESDIAG_CFG_SPACE_FROM_URL" ]; then
        ESDIAG_CFG_SPACE="$ESDIAG_CFG_SPACE_FROM_URL"
    else
        ESDIAG_CFG_SPACE="$ESDIAG_KIBANA_SPACE_DEFAULT"
    fi

    ESDIAG_CFG_ELASTICSEARCH_BASE="${ESDIAG_ELASTICSEARCH_URL:-${ESDIAG_OUTPUT_URL:-}}"
    ESDIAG_CFG_ELASTICSEARCH_BASE="${ESDIAG_CFG_ELASTICSEARCH_BASE%/}"
    ESDIAG_CFG_AGENT_ID="${ESDIAG_AGENT_ID:-$ESDIAG_AGENT_ID_DEFAULT}"
    ESDIAG_CFG_INFERENCE_ID="${ESDIAG_INFERENCE_ID:-}"
    ESDIAG_CFG_JOB="${ESDIAG_JOB:-}"
    ESDIAG_CFG_MAX_AGE="${ESDIAG_DIAGNOSTIC_MAX_AGE:-$ESDIAG_DIAGNOSTIC_MAX_AGE_DEFAULT}"

    esdiag_config_read_apikey || return 1
    if [ "$require_key" = "require-key" ] && [ -z "$ESDIAG_CFG_APIKEY" ]; then
        esdiag_config_error "No API key configured. Set ESDIAG_KIBANA_APIKEY or ESDIAG_KIBANA_APIKEY_FILE."
        return 1
    fi
}

esdiag_config_require_elasticsearch() {
    if [ -z "$ESDIAG_CFG_ELASTICSEARCH_BASE" ]; then
        esdiag_config_error "No Elasticsearch URL configured. Set ESDIAG_ELASTICSEARCH_URL or ESDIAG_OUTPUT_URL."
        return 1
    fi
}

esdiag_config_url() {
    local suffix="${1#/}"
    if [ -n "$ESDIAG_CFG_SPACE" ]; then
        printf '%s/s/%s/%s' "$ESDIAG_CFG_KIBANA_BASE" "$ESDIAG_CFG_SPACE" "$suffix"
    else
        printf '%s/%s' "$ESDIAG_CFG_KIBANA_BASE" "$suffix"
    fi
}

esdiag_config_json() {
    local key_source="none"
    if [ -n "${ESDIAG_KIBANA_APIKEY:-}" ]; then
        key_source="env"
    elif [ -n "${ESDIAG_KIBANA_APIKEY_FILE:-}" ]; then
        key_source="file"
    fi

    jq -n \
        --arg kibana "$ESDIAG_CFG_KIBANA_BASE" \
        --arg elasticsearch "$ESDIAG_CFG_ELASTICSEARCH_BASE" \
        --arg space "$ESDIAG_CFG_SPACE" \
        --arg agent "$ESDIAG_CFG_AGENT_ID" \
        --arg inference "$ESDIAG_CFG_INFERENCE_ID" \
        --arg job "$ESDIAG_CFG_JOB" \
        --arg max_age "$ESDIAG_CFG_MAX_AGE" \
        --arg key_source "$key_source" \
        --arg converse "$(esdiag_config_url 'api/agent_builder/converse/async')" \
        '{kibana_base:$kibana,
          elasticsearch_base:(if $elasticsearch == "" then null else $elasticsearch end),
          space:$space, agent_id:$agent,
          inference_id:(if $inference == "" then null else $inference end),
          job:(if $job == "" then null else $job end),
          max_age:$max_age, api_key_source:$key_source, converse_url:$converse}'
}

if [ "${BASH_SOURCE[0]}" = "$0" ]; then
    set -euo pipefail
    case "${1:---json}" in
        --json) esdiag_config_resolve "no-require-key"; esdiag_config_json ;;
        --check) esdiag_config_resolve; esdiag_config_json ;;
        --help|-h) printf 'Usage: config.sh [--json|--check]\n' ;;
        *) esdiag_config_error "unknown option: $1"; exit 1 ;;
    esac
fi
