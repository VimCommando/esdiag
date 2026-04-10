## 1. Host CLI Restructure

- [x] 1.1 Replace the positional `host` command with a nested host subcommand enum for `add`, `update`, `remove`, `list`, and `auth`.
- [x] 1.2 Factor shared host mutation arguments so `add` and `update` reuse the same auth, role, and certificate flag parsing while keeping `add` responsible for required `app` and `url` inputs.
- [x] 1.3 Route `add`, `update`, `remove`, and `auth` through dedicated handlers with strict duplicate-host and missing-host guardrails plus existing connection validation where required.

## 2. Saved Host Operations

- [x] 2.1 Reuse or extend `KnownHost` helpers so `update` preserves omitted fields, `remove` deletes persisted hosts cleanly, and `auth` validates a saved host without mutating it.
- [x] 2.2 Add a compact saved-host table renderer for `esdiag host list` that prints `name`, `app`, and `secret`, plus `No saved hosts` for empty storage.
- [x] 2.3 Remove or replace the legacy overlapping `esdiag host <name> ...` mutation path with explicit guidance toward the new subcommands.

## 3. Docs And Tests

- [x] 3.1 Update host CLI regression tests to cover add success, add duplicate failure, add incomplete failure, update success, update missing-host failure, remove success, remove missing-host failure, list output, auth success, auth missing-host failure, and legacy syntax rejection.
- [x] 3.2 Update user-facing CLI documentation to show the new host subcommands and migration examples from the old positional syntax.
- [x] 3.3 Update `skills/esdiag/SKILL.md` alongside the docs so the project skill reflects the new host subcommands and removes the legacy positional host guidance.

## 4. Verification

- [x] 4.1 Run `cargo clippy`.
- [x] 4.2 Run `cargo test`.
