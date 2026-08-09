## Context

Local ESDiag state is currently split across `hosts.yml`, `secrets.yml`, `jobs.yml`, unlock state, and a two-field `settings.yml` used primarily by desktop/user-mode server startup. There is no first-run command and no general persistent preference model. The Agent Skill consequently tries to detect and assemble missing state itself, which puts credential handling and application onboarding in prompt instructions.

The existing domain model already separates concerns correctly. `KnownHost` stores non-secret endpoint metadata and a secret reference, the keystore owns authentication material, `SavedJobs` references known hosts, a send-role Elasticsearch host can identify its view-role Kibana host through `viewer`, and `Identifiers` already honors an explicit `--user` and `ESDIAG_USER`. Initialization should compose those models, not replace them.

The runtime environment also already represents one output deployment: `ESDIAG_OUTPUT_URL` and `ESDIAG_OUTPUT_*` authentication identify Elasticsearch, while `ESDIAG_KIBANA_URL` identifies the Kibana instance attached to that Elasticsearch cluster and uses the same authentication. This relationship must remain atomic so commands cannot accidentally combine Elasticsearch from one deployment with Kibana or credentials from another.

## Goals / Non-Goals

**Goals:**

- Give a newly installed local user one secure, guided path to a successful repeatable workflow.
- Persist only non-secret preferences and references in one general `esdiag.yml` file.
- Reuse `KnownHost`, viewer links, keystore secrets, saved jobs, and existing clients in process.
- Resolve one canonical output deployment consistently across CLI, desktop, web user mode, and future Agent Builder commands.
- Default diagnostic user identifiers from persisted configuration after explicit CLI and environment values.
- Offer installation of the running binary's version-matched ESDiag skill without making agent presence a prerequisite for initialization.
- Make initialization resumable, idempotent, and safe around existing files.
- Keep agents out of secret collection by making credential entry a terminal-native interaction.

**Non-Goals:**

- Storing URLs, API keys, passwords, or complete host/job definitions in `esdiag.yml`.
- Replacing `hosts.yml`, `secrets.yml`, or `jobs.yml`.
- Persisting user configuration in service mode.
- Automatically provisioning licenses, inference services, connectors, or third-party model credentials.
- Automatically running `esdiag setup` without explicit user approval.
- Providing a non-interactive bootstrap language in the first iteration; existing commands and environment variables remain available for automation.
- Performing Agent Builder conversations or diagnostic analysis.
- Downloading agent skills or requiring any supported coding agent to be installed.

## Decisions

### Use one application configuration file containing references

Introduce a versioned `ApplicationConfig` stored at `~/.esdiag/esdiag.yml`:

```yaml
version: 1
user: reno@example.com
output: diagnostic-output
default_job: production-standard
```

`output` names a saved Elasticsearch host with role `send`; that host's `viewer` names the saved Kibana host with role `view`. `default_job` names an entry in `jobs.yml`. Authentication remains a secret reference in each host and encrypted material remains in `secrets.yml`. The output Elasticsearch and Kibana hosts may share one secret identifier when they use the same credentials.

This makes configuration a small preference layer rather than a second inventory. Alternative considered: store both URLs and credentials in `esdiag.yml`. That duplicates `KnownHost`, creates another secret surface, and allows configuration to drift.

### Replace, rather than supplement, desktop settings

Do not create `esdiag.yml` alongside a permanently supported `settings.yml`. On first user-mode load or `esdiag init`, inspect legacy `settings.yml` and migrate representable preferences into `ApplicationConfig`. Preserve a backup until the new file has been written and validated.

An `active_target` that names a valid send host becomes `output`. A legacy `kibana_url` is representable only when it matches the selected send host's viewer; otherwise initialization asks the user to create or select the corresponding view host before completing migration. Ordinary startup reports an actionable migration error instead of silently pairing unrelated endpoints. Service mode neither reads nor writes either local file.

Alternative considered: extend `settings.yml` indefinitely. Its name and current desktop-only shape obscure that the values affect all local CLI workflows, and retaining both names creates precedence ambiguity.

### Resolve the output deployment as one atomic value

Add a shared `OutputDeployment` resolver that yields an Elasticsearch host, its Kibana viewer when required, one resolved authentication method, and space-normalized Kibana routing. It uses this precedence:

1. An explicit command target.
2. A complete runtime environment deployment beginning with `ESDIAG_OUTPUT_URL`, with `ESDIAG_KIBANA_URL` required by operations that need Kibana.
3. The `ApplicationConfig.output` saved-host reference.
4. A configuration error.

Once a source wins, all endpoints and credentials come from that source. A partial environment deployment fails closed; it is never completed with a saved host or preference from another deployment. `ESDIAG_OUTPUT_APIKEY` or the existing output username/password pair authenticates both environment-backed Elasticsearch and Kibana. No analysis-specific Elasticsearch URL or Kibana credential variables are introduced.

For saved deployments, the send host and its viewer may reference the same keystore secret. The resolver uses existing `KnownHost` and keystore APIs and never copies decrypted material into configuration or outcomes.

### Make initialization a resumable state machine

Model the wizard as explicit stages:

```text
InspectExisting
    -> Identity
    -> Keystore
    -> OutputDeployment
    -> CollectionHosts
    -> DefaultJob
    -> AgentSkills
    -> Complete
```

Each transition validates its resulting domain object before persisting it. Completed stages are detected on the next run and offered for reuse or explicit replacement. Files are written atomically where their existing APIs support it; initialization never shells out to another `esdiag` process.

The wizard persists independently valid stages rather than attempting a transaction across four files. An interruption can therefore leave a valid keystore or host without falsely marking initialization complete. `ApplicationConfig` is written last and acts as the completion record.

### Keep credential entry on the controlling terminal

Keystore passwords, API keys, and host passwords use hidden TTY prompts and existing keystore functions. They never appear in command arguments generated by the wizard, structured outcomes, logs, `esdiag.yml`, or onboarding documentation examples. When no controlling terminal is available and a required secret is not already supplied through an existing secure mechanism, `init` exits with actionable guidance rather than reading secrets from ordinary stdin.

This boundary lets an Agent Skill recommend `esdiag init` without becoming a credential broker.

### Configure an output deployment as linked existing host types

The output step creates or selects:

- One Elasticsearch host with role `send`.
- One Kibana host with role `view`.
- A `viewer` reference from the send host to the view host.
- One shared secret reference by default, with distinct existing secrets allowed when explicitly selected.

Both clients are tested before the configuration becomes active. Initialization also checks whether ESDiag assets required for processing and Agent Builder use are available. If setup is needed, the wizard offers to run the existing setup operation and explains the privilege/license implications; declining preserves the configured endpoints and reports setup as incomplete.

Alternative considered: add a new deployment inventory type. The existing host-role and viewer relationship already models the pair and is consumed by exporter link generation, so a parallel type would add migration and synchronization costs.

### Make collection hosts and the first job part of success

After the output deployment is usable, initialization creates or selects at least one collect-role host, offers a loop for additional hosts, and builds the first saved job through the existing typed job model. The default successful path creates a processing job whose output is the configured send host, so running it produces indexed data rather than only a local archive. The user may explicitly select a collect-only job instead.

The selected job name is stored as `default_job`; the job body remains solely in `jobs.yml`. Initialization may finish without additional hosts beyond the first, but it does not claim a repeatable diagnostic workflow is ready without a valid default job.

### Apply identifier precedence explicitly

Default user resolution becomes:

```text
--user > ESDIAG_USER > ApplicationConfig.user > absent
```

The configured user is copied into the normal `Identifiers` value at workflow construction time. No global mutable environment value is synthesized.

### Keep onboarding guidance separate from normal skill use

Add `references/onboarding.md` to the canonical skill and generated packages. The main skill routes explicit first-install/configuration requests and missing-configuration failures to that reference. The reference explains the stages and asks the human to run `esdiag init` locally; it does not reproduce commands for writing secrets or files manually.

### Offer embedded agent skill installation without gating success

After the diagnostic workflow is configured, `init` asks whether to detect and install the embedded ESDiag skill for supported local coding agents. The stage calls the same in-process target detection and installation service exposed by `esdiag agent skills`; it does not spawn another ESDiag process, download a plugin, or duplicate path logic.

The wizard shows detected targets and lets the user accept, select explicit additional targets, or decline. Declining completes initialization normally. A conflict or installation failure is recorded in the final outcome and gives the standalone recovery command, but it does not invalidate the already configured keystore, output deployment, hosts, or job. No skill-installation preference or target path is persisted in `esdiag.yml`; installed files and their ownership markers are the authoritative state.

Alternative considered: install automatically whenever an agent is detected. Writing into multiple agent homes is a separate user-visible mutation and should remain opt-in even inside an onboarding wizard.

## Risks / Trade-offs

- **Cross-file interruption leaves partial state** → Persist only independently valid stages, detect them on rerun, and write the completion configuration last.
- **Legacy `kibana_url` cannot always be mapped automatically** → Require an explicit viewer selection during migration and preserve a backup until completion.
- **Environment and saved configuration could identify different deployments** → Select one complete source atomically and fail on partial environment configuration.
- **Initialization becomes a large interactive workflow** → Keep each stage backed by existing domain APIs and make the stage boundaries independently testable.
- **Setup may require elevated cluster privileges or a license** → Validate first, ask explicitly before setup, and distinguish configured endpoints from provisioned assets.
- **A shared API key might not be valid for both products** → Test both clients and allow explicitly selected distinct existing secret references while keeping shared credentials the default.
- **Changing the settings filename affects desktop startup** → Provide one-time migration with backup and compatibility tests; do not silently discard unrepresentable values.
- **Optional skill installation can fail after core initialization succeeds** → Keep it as a non-gating final stage, preserve per-target results, and provide `esdiag agent skills` as a resumable standalone action.

## Migration Plan

1. Add `ApplicationConfig`, path resolution, atomic serialization, and validation without changing consumers.
2. Add legacy `Settings` import and backup behavior with fixtures for representable and ambiguous configurations.
3. Add atomic `OutputDeployment` resolution and migrate omitted-output CLI and user-mode consumers.
4. Add persisted default-user resolution to identifier construction.
5. Implement and test the staged `esdiag init` workflow using existing keystore, host, client, setup, job, and embedded-skill installer APIs.
6. Update desktop/web user-mode settings to write `esdiag.yml`; retain service-mode isolation.
7. Add and package `references/onboarding.md`, then update documentation and changelog.
8. Remove legacy `settings.yml` writes after migration coverage passes.

Rollback restores the backed-up `settings.yml` and leaves `hosts.yml`, `secrets.yml`, and `jobs.yml` unchanged. `esdiag.yml` contains no secrets and can be ignored safely by older releases.

## Open Questions

None. Asset setup and agent skill installation are explicit offers rather than automatic initialization side effects, and the first version is intentionally interactive.
