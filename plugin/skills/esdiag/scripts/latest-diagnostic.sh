#!/usr/bin/env bash

# Copyright Elasticsearch B.V. and/or licensed to Elasticsearch B.V. under one
# or more contributor license agreements. Licensed under the Elastic License 2.0.

# Return the most recently ingested diagnostic and whether it is inside the
# configured freshness window. This metadata lookup calls Elasticsearch
# directly and never creates an Agent Builder conversation or inference spend.

set -uo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source-path=SCRIPTDIR
. "${script_dir}/config.sh"

command_name="${0##*/}"
window=""

usage() {
    cat <<EOF
Report the most recent diagnostic as JSON.

Usage: ${command_name} [--window <duration>]

Options:
  --window <duration>  Freshness threshold such as 24h, 7d, or 90m.
                       Defaults to \$ESDIAG_DIAGNOSTIC_MAX_AGE, then 24h
  --help               Show this help

Output: {"found":bool,"fresh":bool,"window":str,
         "diagnostic_id":str,"ingested":str,"age_minutes":num}

Exit codes:
  0  lookup succeeded
  1  configuration or duration error
  2  query failed; freshness is unknown and collection must not be inferred
EOF
}

die() { printf '%s: %s\n' "$command_name" "$1" >&2; exit 1; }

while [ $# -gt 0 ]; do
    case "$1" in
        --window) window="${2:-}"; shift 2 ;;
        --help|-h) usage; exit 0 ;;
        *) die "unknown option: $1" ;;
    esac
done

esdiag_config_resolve || exit 1
esdiag_config_require_elasticsearch || exit 1
[ -n "$window" ] || window="$ESDIAG_CFG_MAX_AGE"

duration_minutes() {
    local value="$1" number unit
    [ -n "$value" ] || return 1
    unit="${value#"${value%?}"}"
    number="${value%?}"
    case "$number" in ''|*[!0-9]*) return 1 ;; esac
    case "$unit" in
        m) printf '%s' "$((10#$number))" ;;
        h) printf '%s' "$((10#$number * 60))" ;;
        d) printf '%s' "$((10#$number * 1440))" ;;
        w) printf '%s' "$((10#$number * 10080))" ;;
        *) return 1 ;;
    esac
}

max_age_minutes="$(duration_minutes "$window")" || die "unrecognized window: ${window} (use forms like 24h, 7d, 90m)"

query='FROM metrics-diagnostic-esdiag*
| KEEP diagnostic.id, event.ingested
| SORT event.ingested DESC
| LIMIT 1
| EVAL age_minutes = DATE_DIFF("minutes", event.ingested, NOW())
| EVAL fresh = age_minutes <= ?max_age_minutes'

body_file="$(mktemp)"
trap 'rm -f "$body_file"' EXIT

status="$(curl -sS -m 60 -o "$body_file" -w '%{http_code}' -X POST \
    "${ESDIAG_CFG_ELASTICSEARCH_BASE}/_query" \
    -H "Authorization: ApiKey ${ESDIAG_CFG_APIKEY}" \
    -H 'Content-Type: application/json' \
    -d "$(jq -n --arg q "$query" --argjson age "$max_age_minutes" \
        '{query:$q,params:[{max_age_minutes:$age}]}')" 2>/dev/null)" || status="000"

if [ "$status" != "200" ]; then
    printf '%s: freshness query failed (HTTP %s). Freshness is unknown; do not infer collection.\n' \
        "$command_name" "$status" >&2
    head -c 400 "$body_file" >&2
    printf '\n' >&2
    exit 2
fi

if jq -e '.is_partial == true' "$body_file" >/dev/null 2>&1; then
    printf '%s: freshness query returned partial results. Freshness is unknown.\n' "$command_name" >&2
    exit 2
fi

jq -c --arg window "$window" '
    def colval($cols; $row; $name):
        ($cols | index($name)) as $i
        | if $i == null then null else $row[$i] end;
    if ((.values // []) | length) == 0 then
        {found:false, fresh:false, window:$window}
    else
        (.columns | map(.name)) as $cols
        | .values[0] as $row
        | colval($cols; $row; "diagnostic.id") as $id
        | if $id == null then error("ES|QL response omitted diagnostic.id")
          else {found:true,
                fresh:(colval($cols; $row; "fresh") // false),
                window:$window,
                diagnostic_id:$id,
                ingested:colval($cols; $row; "event.ingested"),
                age_minutes:colval($cols; $row; "age_minutes")}
          end
    end' "$body_file" || {
        printf '%s: freshness response was invalid. Freshness is unknown.\n' "$command_name" >&2
        exit 2
    }
