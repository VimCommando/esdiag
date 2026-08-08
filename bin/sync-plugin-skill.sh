#!/usr/bin/env bash

# Copyright Elasticsearch B.V. and/or licensed to Elasticsearch B.V. under one
# or more contributor license agreements. Licensed under the Elastic License 2.0.

# Synchronize the portable ESDiag skill content into the Claude Code plugin.
# Provider-specific agents/*.yaml metadata stays with the source skill because
# Claude Code neither consumes nor needs it.

set -euo pipefail

command_name="${0##*/}"
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source_dir="${repo_root}/.agents/skills/esdiag"
target_dir="${repo_root}/plugin/skills/esdiag"
check_only=false
files=(SKILL.md references/cli.md references/env-vars.md)

usage() {
    cat <<EOF
Synchronize the portable ESDiag skill into the Claude Code plugin.

Usage: ${command_name} [--check]

  --check   Fail when the bundled files differ from their source
EOF
}

while [ $# -gt 0 ]; do
    case "$1" in
        --check) check_only=true; shift ;;
        --help|-h) usage; exit 0 ;;
        *) printf '%s: unknown option: %s\n' "$command_name" "$1" >&2; exit 1 ;;
    esac
done

staging="$(mktemp -d)"
trap 'rm -rf "$staging"' EXIT

for relative in "${files[@]}"; do
    [ -f "${source_dir}/${relative}" ] || {
        printf '%s: source file missing: %s\n' "$command_name" "$relative" >&2
        exit 1
    }
    mkdir -p "${staging}/esdiag/$(dirname "$relative")"
    cp "${source_dir}/${relative}" "${staging}/esdiag/${relative}"
done

if [ "$check_only" = true ]; then
    if [ ! -d "$target_dir" ] || ! diff -r -q "${staging}/esdiag" "$target_dir" >/dev/null 2>&1; then
        printf '%s: bundled Claude skill differs from .agents/skills/esdiag\n' "$command_name" >&2
        diff -r -u "$target_dir" "${staging}/esdiag" >&2 || true
        exit 1
    fi
    printf 'Bundled Claude skill matches the portable ESDiag skill files.\n'
    exit 0
fi

rm -rf "$target_dir"
mkdir -p "$(dirname "$target_dir")"
cp -R "${staging}/esdiag" "$target_dir"
printf 'Regenerated plugin/skills/esdiag from portable source files.\n'
