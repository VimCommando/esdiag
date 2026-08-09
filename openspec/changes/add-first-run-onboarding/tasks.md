## 1. Application Configuration Foundation

- [ ] 1.1 Add a versioned `ApplicationConfig` model for `user`, output-host reference, and default-job reference with atomic `esdiag.yml` load/save behavior under the ESDiag state directory.
- [ ] 1.2 Add validation that referenced output hosts and jobs exist, output hosts have role `send`, and output viewers resolve to role-`view` Kibana hosts without loading secret values into configuration.
- [ ] 1.3 Add fixtures proving configuration round trips, rejects unknown versions and invalid references, and never serializes credential material.
- [ ] 1.4 Implement legacy `settings.yml` import, backup, representable migration, and explicit ambiguous-viewer failure without retaining a second write path.

## 2. Canonical Output Deployment

- [ ] 2.1 Implement an atomic `OutputDeployment` resolver for explicit targets, complete environment deployments, and persisted output references in the specified precedence order.
- [ ] 2.2 Reuse `ESDIAG_OUTPUT_*` authentication for both environment-backed Elasticsearch and Kibana clients and remove any need for analysis-specific credential resolution.
- [ ] 2.3 Add resolution tests for explicit, environment, and persisted sources, including partial-environment failure and prevention of cross-source endpoint mixing.
- [ ] 2.4 Migrate omitted-output CLI and user-mode server/desktop startup paths to the shared resolver while preserving service-mode isolation.

## 3. Identity And Existing State Integration

- [ ] 3.1 Apply `--user`, `ESDIAG_USER`, then `ApplicationConfig.user` precedence when constructing collection and processing identifiers.
- [ ] 3.2 Update user-mode settings reads and writes to use `esdiag.yml` output references and validate runtime exporter updates through the canonical deployment model.
- [ ] 3.3 Add compatibility tests for desktop restart, CLI omitted-output reuse, service-mode non-persistence, and identifier precedence.

## 4. Initialization State Machine

- [ ] 4.1 Add the `esdiag init` Clap surface and explicit inspect, identity, keystore, output, collection-host, default-job, optional agent-skills, and complete stages.
- [ ] 4.2 Implement resumable existing-state inspection and explicit replacement confirmations, writing `ApplicationConfig` only after required stages validate.
- [ ] 4.3 Implement hidden controlling-terminal prompts for keystore passwords and host credentials using existing keystore APIs, including non-TTY failure behavior and redaction tests.
- [ ] 4.4 Implement output deployment creation/selection as a send-role Elasticsearch host linked to a view-role Kibana host, with shared-secret default and existing distinct-secret selection.
- [ ] 4.5 Validate both output clients, inspect required ESDiag assets, and offer the existing setup operation only after explicit approval.
- [ ] 4.6 Implement the repeatable collect-host loop and first saved-job creation, defaulting to collect-process-send while retaining explicit collect-only selection.
- [ ] 4.7 Emit the standard safe initialization outcome when structured CLI output is available and add interrupted/resumed end-to-end wizard tests.
- [ ] 4.8 Compose the embedded skill installer in process, present detected and explicit target choices, keep decline/failure non-gating, and include per-target results plus standalone recovery guidance.

## 5. Documentation And Agent Handoff

- [ ] 5.1 Add `.agents/skills/esdiag/references/onboarding.md` covering the local `esdiag init` handoff and standalone offline `esdiag agent skills` installation without secret-entry commands or manual state-file edits.
- [ ] 5.2 Update `SKILL.md`, generated plugin assets, CLI documentation, configuration documentation, and repository organization references for `esdiag.yml` and the initialization workflow.
- [ ] 5.3 Update `CHANGELOG.md` using the repository changelog skill and add migration guidance from `settings.yml`.

## 6. Verification

- [ ] 6.1 Run formatting and targeted configuration, host, keystore, settings, identifier, saved-job, optional skill-installation, and initialization tests.
- [ ] 6.2 Run `cargo clippy --all-targets --all-features -- -D warnings`.
- [ ] 6.3 Run `cargo test --all-features`.
- [ ] 6.4 Validate the generated portable skill and run strict OpenSpec validation for `add-first-run-onboarding`.
