#!/usr/bin/env bash

# Copyright Elasticsearch B.V. and/or licensed to Elasticsearch B.V. under one
# or more contributor license agreements. Licensed under the Elastic License 2.0.

# Extract the diagnostic identifier and Kibana link from esdiag command output.
#
# esdiag writes its completion summary to stderr, so callers should combine both
# streams: esdiag process ... 2>&1 | extract-diagnostic.sh
#
# Parsing here rather than in a prompt keeps identifier handling deterministic:
# the wrong identifier silently analyzes the wrong cluster.

set -uo pipefail

command_name="${0##*/}"

if [ "${1:-}" = "--help" ] || [ "${1:-}" = "-h" ]; then
    cat <<EOF
Extract the diagnostic identifier and Kibana link from esdiag output.

Usage: esdiag process ... 2>&1 | ${command_name}

Output: {"diagnostic_id":str|null,"kibana_link":str|null,"included":[str]}

Exit codes:
  0  a primary diagnostic identifier was found
  1  no identifier found in the input
EOF
    exit 0
fi

input="$(cat)"

# "process complete in 4.212 seconds: 18432 documents for <id>"
# Included diagnostics repeat the pattern with a trailing "(<product>)".
primary="$(printf '%s\n' "$input" \
    | grep -E '^(process|collect) complete .*documents for ' \
    | head -1 \
    | sed -E 's/.*documents for ([^ ]+).*/\1/')"

kibana_link="$(printf '%s\n' "$input" \
    | grep -E '^Kibana Link: ' \
    | head -1 \
    | sed -E 's/^Kibana Link: //')"

included="$(printf '%s\n' "$input" \
    | grep -E '^included diagnostic complete .*documents for ' \
    | sed -E 's/.*documents for ([^ ]+).*/\1/' \
    | jq -Rs 'split("\n") | map(select(length > 0))')"

jq -n \
    --arg id "$primary" \
    --arg link "$kibana_link" \
    --argjson included "${included:-[]}" \
    '{diagnostic_id: (if $id == "" then null else $id end),
      kibana_link: (if $link == "" then null else $link end),
      included: $included}'

[ -n "$primary" ] || exit 1
exit 0
