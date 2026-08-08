#!/usr/bin/env bash

# Copyright Elasticsearch B.V. and/or licensed to Elasticsearch B.V. under one
# or more contributor license agreements. Licensed under the Elastic License 2.0.

# Find the most recent diagnostic within the freshness window.
#
# This is a metadata lookup, not analysis: it runs ES|QL directly through the
# tool execution endpoint and invokes no agent, so it consumes no inference
# tokens. Used to decide whether an ambiguous request can reuse an existing
# diagnostic or needs a new collection.

set -uo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./config.sh
. "${script_dir}/config.sh"

command_name="${0##*/}"
window=""
all_users=false

usage() {
    cat <<EOF
Report the most recent diagnostic within the freshness window as JSON.

Usage: ${command_name} [--window <duration>] [--all-users]

Options:
  --window <duration>  Freshness window such as 24h, 7d, 90m.
                       Defaults to \$ESDIAG_DIAGNOSTIC_MAX_AGE, then 24h
  --all-users          Do not scope to \$ESDIAG_USER
  --help               Show this help

Output: {"found":bool,"window":str,"scoped_to":str|null,
         "diagnostic_id":str,"ingested":str,"age_minutes":num}

An empty result means no diagnostic within the window. It does not mean the
deployment has no diagnostics.

Exit codes:
  0  lookup succeeded (found true or false)
  1  configuration error
  2  query failed; freshness is unknown and must not be treated as stale
EOF
}

die() { printf '%s: %s\n' "$command_name" "$1" >&2; exit 1; }

while [ $# -gt 0 ]; do
    case "$1" in
        --window) window="${2:-}"; shift 2 ;;
        --all-users) all_users=true; shift ;;
        --help|-h) usage; exit 0 ;;
        *) die "unknown option: $1" ;;
    esac
done

esdiag_config_resolve || exit 1
[ -n "$window" ] || window="$ESDIAG_CFG_MAX_AGE"

# Convert a compact duration to an ES|QL interval. The window is always stated
# explicitly in the query rather than inherited from the execution endpoint's
# own default time range, which would silently govern the result.
esql_interval() {
    local value="$1" number unit
    number="${value%[a-zA-Z]}"
    unit="${value##*[0-9]}"
    case "$unit" in
        m|min|minutes) printf '%s minutes' "$number" ;;
        h|hours)       printf '%s hours' "$number" ;;
        d|days)        printf '%s days' "$number" ;;
        w|weeks)       printf '%s weeks' "$number" ;;
        *) return 1 ;;
    esac
}

interval="$(esql_interval "$window")" || die "unrecognized window: ${window} (use forms like 24h, 7d, 90m)"

# Only fields confirmed to exist are referenced. An unknown field yields an
# empty result rather than an error, which would be indistinguishable from a
# genuinely stale diagnostic and would trigger a needless collection.
scoped_to="null"
user_filter=""
if [ "$all_users" = false ] && [ -n "${ESDIAG_USER:-}" ]; then
    user_filter="| WHERE diagnostic.user == \"${ESDIAG_USER}\""
    scoped_to="$(printf '%s' "$ESDIAG_USER" | jq -Rs 'rtrimstr("\n")')"
fi

query="FROM metrics-diagnostic-esdiag*
| WHERE event.ingested >= NOW() - ${interval}
${user_filter}
| EVAL age_minutes = DATE_DIFF(\"minutes\", event.ingested, NOW())
| KEEP diagnostic.id, event.ingested, diagnostic.user, age_minutes
| SORT event.ingested DESC
| LIMIT 1"

body_file="$(mktemp)"
trap 'rm -f "$body_file"' EXIT

url="$(esdiag_config_url 'api/agent_builder/tools/_execute')"
status="$(curl -sS -m 60 -o "$body_file" -w '%{http_code}' -X POST "$url" \
    -H "Authorization: ApiKey ${ESDIAG_CFG_APIKEY}" \
    -H 'kbn-xsrf: true' -H 'Content-Type: application/json' \
    -d "$(jq -n --arg q "$query" '{tool_id:"platform.core.execute_esql",tool_params:{query:$q}}')" 2>/dev/null)" || status="000"

if [ "$status" != "200" ]; then
    printf '%s: freshness query failed (HTTP %s). Freshness is unknown; do not treat this as stale.\n' \
        "$command_name" "$status" >&2
    head -c 400 "$body_file" >&2; printf '\n' >&2
    exit 2
fi

# A query that returns no rows means nothing within the window, which is the
# stale branch. It is reported as found:false, never as an error.
# ES|QL silently drops columns for fields that do not exist, so a column named
# in KEEP is not guaranteed to come back. diagnostic.user in particular is
# absent whenever no user metadata was attached at process time. Look columns up
# by name and yield null when missing rather than indexing by a null position.
jq -c --arg window "$window" --argjson scoped "$scoped_to" '
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
        | if $id == null then
            {found: false, window: $window, scoped_to: $scoped}
          else
            {found: true, window: $window, scoped_to: $scoped,
             diagnostic_id: $id,
             ingested: colval($cols; $row; "event.ingested"),
             user: colval($cols; $row; "diagnostic.user"),
             age_minutes: colval($cols; $row; "age_minutes")}
          end
      end' "$body_file"
