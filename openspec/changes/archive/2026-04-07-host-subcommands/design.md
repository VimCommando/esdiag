## Context

`esdiag host` currently infers create, update, delete, and validation behavior from a single positional command shape. That inference makes saved-host lifecycle operations harder to understand than the parallel `esdiag keystore` lifecycle commands, and it forces the CLI to guess whether a given invocation is trying to create a record, mutate one, or just test connectivity.

This change is limited to CLI-managed saved hosts. It needs to preserve the existing `KnownHost` model, reuse current validation and connection-test paths, and stay cross-platform with no new dependencies.

## Goals / Non-Goals

**Goals:**
- Replace the overlapping host mutation surface with explicit `add`, `update`, `remove`, `list`, and `auth` subcommands.
- Make `add` create-only, `update` update-only, and `remove` delete-only with explicit missing/duplicate host failures.
- Preserve the current saved-host merge behavior for update operations so omitted fields stay unchanged.
- Reuse existing connection validation for `add`, `update`, and `auth`.
- Add a compact list view for persisted hosts that surfaces `name`, `app`, and `secret`.

**Non-Goals:**
- Change the `hosts.yml` storage format or the `KnownHost` enum model.
- Redesign web host management.
- Add bulk host import/export flows.
- Change keystore semantics beyond reusing secret references from saved hosts.

## Decisions

1. **Use a nested `host` subcommand tree**
   - Decision: Change `Commands::Host` from a positional mutation command into a parent command with subcommands: `Add`, `Update`, `Remove`, `List`, and `Auth`.
   - Rationale: Explicit verbs remove mode inference from the UX and align host management with `esdiag keystore`.
   - Alternatives considered:
     - Keep a single `host <name>` command and add more flags: rejected because it preserves the existing ambiguity.
     - Introduce aliases only: rejected because aliases would not remove the overlapping legacy surface.

2. **Share host mutation arguments between `add` and `update`**
   - Decision: Factor the mutable host fields into shared CLI argument structs or helpers so `add` and `update` accept the same override surface for auth, roles, and certificate options, while `add` additionally requires `app` and `url`.
   - Rationale: The two mutation paths differ mainly in existence checks and required fields, not in the host attributes they can set.
   - Alternatives considered:
     - Duplicate clap fields in each subcommand: rejected because drift between `add` and `update` would be easy to introduce.
     - Make `update` accept only a narrow subset of fields: rejected because it would regress current update capabilities.

3. **Keep merge logic in the host model layer**
   - Decision: Reuse or extend `KnownHostCliUpdate` and `KnownHost::merge_cli_update()` so `update <name>` continues to load the saved host, apply only provided overrides, validate the merged result, and save it after a successful connection test.
   - Rationale: Existing merge behavior already encodes auth transitions and omitted-field preservation. The command-line restructure should not duplicate those rules in dispatch code.
   - Alternatives considered:
     - Rebuild update logic in `main.rs`: rejected because it would duplicate persistence rules and increase auth-shape bugs.
     - Treat `update` as full replace: rejected because the user specifically wants incremental modification of an existing host.

4. **Make subcommand semantics strict**
   - Decision: `add <name>` MUST fail if the host already exists or if required fields are incomplete, `update <name>` MUST fail if the host does not exist, `remove <name>` MUST fail if the host does not exist, and `auth <name>` MUST fail if the host does not exist.
   - Rationale: Explicit lifecycle verbs only help if each verb has one unambiguous contract.
   - Alternatives considered:
     - Let `add` upsert: rejected because it hides accidental overwrites.
     - Let `update` create missing hosts when `app` and `url` are present: rejected because it reintroduces the current overlap.

5. **Implement `list` as a persisted-host summary view**
   - Decision: `list` will read saved hosts from `hosts.yml`, print a compact table with headers `name`, `app`, and `secret`, and print `No saved hosts` when the saved-host map is empty. The `secret` column will show the configured secret identifier when present and an empty value otherwise.
   - Rationale: This keeps the output focused on host identity and whether credentials are keystore-backed, which is the most useful quick audit for saved hosts.
   - Alternatives considered:
     - Print full YAML: rejected because it is noisy and not a compact inventory view.
     - Add extra auth columns: rejected because the requested contract is specifically `name,app,secret`.

6. **Treat `auth` as validate-only against a saved host**
   - Decision: `auth <name>` will resolve the saved host exactly as persisted and run the existing host authentication/connection test path without writing changes back to disk.
   - Rationale: This gives users a safe way to verify saved credentials separately from add/update flows.
   - Alternatives considered:
     - Reuse bare `host <name>` as the auth test path: rejected because the goal is to remove overlapping host verbs.
     - Make `auth` update host metadata on success: rejected because auth verification should be side-effect free.

## Risks / Trade-offs

- **[Risk] Subcommand parsing may break existing scripts that still call the old positional host form** -> **Mitigation:** Capture the change explicitly in spec and docs, and keep the migration guidance focused on one-to-one command replacements.
- **[Risk] Shared add/update argument helpers can still drift from `KnownHostCliUpdate` semantics** -> **Mitigation:** Keep normalization and merge behavior in shared host-model helpers and cover both subcommands with CLI tests.
- **[Risk] `list` output may become unstable if columns are overformatted** -> **Mitigation:** Keep the table compact and deterministic with fixed headers and simple values.
- **[Risk] `auth` may be misunderstood as mutating or refreshing a host** -> **Mitigation:** Make the command side-effect free and document that it only tests the saved configuration.

## Migration Plan

1. Add the nested `host` subcommand enum and route each subcommand to dedicated handlers.
2. Reuse existing host-build and host-merge helpers for `add` and `update`.
3. Move delete behavior from `--delete` into `remove <name>`.
4. Add a saved-host table renderer for `list`.
5. Reuse `validate_host_connection()` for `auth <name>` without persistence.
6. Update tests, CLI docs, and `skills/esdiag/SKILL.md` to reflect the new one-command-per-action shape.

Rollback strategy:
- Restore the old positional `host <name> [app] [url]` parsing and remove the explicit subcommands.
- No data migration is required because saved hosts remain stored in the same format.

## Open Questions

None.
