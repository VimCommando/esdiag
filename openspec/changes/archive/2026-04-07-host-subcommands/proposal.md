## Why

`esdiag host` currently overloads create, update, delete, and validation behavior into a single positional command shape, which makes host lifecycle actions hard to discover and inconsistent with the clearer `esdiag keystore` subcommands. Splitting host management into explicit subcommands will make saved-host operations predictable and allow stricter validation for create, update, remove, list, and authentication test flows.

## What Changes

- Replace the overlapping `esdiag host <name> ...` mutation flow with explicit subcommands: `add <name>`, `update <name>`, `remove <name>`, `list`, and `auth <name>`.
- Make `esdiag host add <name>` create-only and fail when the provided host definition is incomplete or the name already exists.
- Make `esdiag host update <name>` update-only and fail when the host does not exist.
- Make `esdiag host remove <name>` delete-only and fail when the host does not exist.
- Add `esdiag host list` to print a compact `name,app,secret` table and print `No saved hosts` when no hosts are saved.
- Add `esdiag host auth <name>` to validate authentication against a saved host without mutating it.
- **BREAKING** Retire the old overlapping `esdiag host <name> ...` mutation workflow in favor of the explicit subcommand structure.

## Capabilities

### New Capabilities
None.

### Modified Capabilities
- `cli-host-record-management`: Change saved-host CLI management from a single overlapping command into explicit add, update, remove, list, and auth subcommands with create-only and update-only guardrails.

## Impact

- CLI argument parsing and dispatch in `src/main.rs`.
- Saved host validation, persistence, lookup, and display helpers in `src/data/known_host.rs` and related CLI support code.
- Host CLI regression coverage in `tests/host_cli_tests.rs`.
- User-facing command references in `docs/` and operator guidance in `skills/esdiag/SKILL.md`.
