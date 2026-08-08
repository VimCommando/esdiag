#!/usr/bin/env bash

# Copyright Elasticsearch B.V. and/or licensed to Elasticsearch B.V. under one
# or more contributor license agreements. Licensed under the Elastic License 2.0.

# Regenerate the Claude Code plugin's bundled operations skill from the
# repository skill at .agents/skills/esdiag/.
#
# The plugin ships a copy because Claude Code installs plugins as plain
# repository content, but that copy is generated, never hand-edited. Run with
# --check in CI and packaging to fail when the two have diverged.

set -euo pipefail

command_name="${0##*/}"
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source_dir="${repo_root}/.agents/skills/esdiag"
target_dir="${repo_root}/plugin/skills/esdiag"
generated_notice="<!-- Generated from .agents/skills/esdiag/ by bin/${command_name}. Do not edit. -->"

check_only=false

usage() {
    cat <<EOF
Regenerate the plugin's bundled ESDiag operations skill.

Usage: ${command_name} [--check]

Options:
  --check   Verify the bundled skill matches the repository skill; exit 1 on drift
  --help    Show this help

Source: .agents/skills/esdiag/
Target: plugin/skills/esdiag/
EOF
}

die() {
    printf '%s: %s\n' "$command_name" "$1" >&2
    exit 1
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --check) check_only=true; shift ;;
        --help|-h) usage; exit 0 ;;
        *) die "unknown option: $1" ;;
    esac
done

[[ -d "$source_dir" ]] || die "source skill not found: ${source_dir}"
[[ -f "${source_dir}/SKILL.md" ]] || die "source skill has no SKILL.md: ${source_dir}"

# Render the generated tree into a staging directory first so a failed run
# never leaves a partially written skill behind.
staging="$(mktemp -d)"
trap 'rm -rf "$staging"' EXIT

render() {
    local destination="$1"
    mkdir -p "$destination"

    # Markdown carries a provenance banner; other files copy verbatim.
    while IFS= read -r relative; do
        local from="${source_dir}/${relative}"
        local to="${destination}/${relative}"
        mkdir -p "$(dirname "$to")"
        if [[ "$relative" == *.md ]]; then
            if head -n 1 "$from" | grep -q '^---$'; then
                # Keep frontmatter first; insert the notice directly after it.
                awk -v notice="$generated_notice" '
                    NR == 1 { print; next }
                    !inserted && /^---$/ { print; print ""; print notice; inserted = 1; next }
                    { print }
                ' "$from" > "$to"
            else
                { printf '%s\n\n' "$generated_notice"; cat "$from"; } > "$to"
            fi
        else
            cp "$from" "$to"
        fi
    done < <(cd "$source_dir" && find . -type f -not -name '.*' | sed 's|^\./||' | sort)
}

render "${staging}/esdiag"

if [[ "$check_only" == true ]]; then
    if [[ ! -d "$target_dir" ]]; then
        die "bundled skill missing: ${target_dir}. Run bin/${command_name} to generate it."
    fi
    if ! diff -r -q "${staging}/esdiag" "$target_dir" >/dev/null 2>&1; then
        printf '%s: bundled plugin skill has drifted from .agents/skills/esdiag/\n' "$command_name" >&2
        diff -r -u "$target_dir" "${staging}/esdiag" >&2 || true
        printf '\nRun bin/%s to regenerate.\n' "$command_name" >&2
        exit 1
    fi
    printf 'Bundled plugin skill matches .agents/skills/esdiag/\n'
    exit 0
fi

rm -rf "$target_dir"
mkdir -p "$(dirname "$target_dir")"
cp -R "${staging}/esdiag" "$target_dir"
printf 'Regenerated %s from %s\n' "plugin/skills/esdiag" ".agents/skills/esdiag"
