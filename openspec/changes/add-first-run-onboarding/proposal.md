## Why

ESDiag persists hosts, secrets, jobs, and a small desktop-only settings record, but a newly installed user must discover and assemble those pieces manually. A first-run workflow and one general application configuration file can establish a secure, repeatable diagnostic workflow without moving credential entry or setup orchestration into an Agent Skill.

## What Changes

- Add an interactive `esdiag init` workflow that can be resumed safely and never overwrites valid existing state without confirmation.
- Capture a default user name or email for diagnostic `Identifiers` when `--user` and `ESDIAG_USER` are absent.
- Create or unlock the encrypted keystore through terminal-native secret prompts that do not expose credentials to an agent conversation or structured output.
- Configure and validate the diagnostic output deployment as one Elasticsearch send host linked to one Kibana view host, sharing one keystore secret when their authentication is the same.
- Create the first collect host, optionally add more collect hosts, and configure the first saved collection or processing job.
- Optionally detect supported local coding agents and install the version-matched ESDiag skill embedded in the running binary through `esdiag agent skills`.
- Introduce `~/.esdiag/esdiag.yml` as the general non-secret application configuration containing preference values and references such as the default user, output host, and saved job.
- **BREAKING** Migrate user-mode settings from the narrow `settings.yml` record to `esdiag.yml`; do not retain two competing application configuration files.
- Keep `ESDIAG_OUTPUT_*` and `ESDIAG_KIBANA_URL` as runtime/deployment overrides for the same output deployment, with shared output credentials, rather than creating analysis-specific URL or credential variables.
- Add `references/onboarding.md` to the portable ESDiag skill so agents direct first-time users to the secure local wizard instead of collecting secrets or reconstructing setup in prompts.

## Capabilities

### New Capabilities

- `cli-first-run-onboarding`: Interactive, resumable initialization of identity, keystore, output deployment, collection hosts, and the first saved job.
- `application-configuration`: General non-secret configuration persistence, migration, precedence, validation, and canonical output-deployment resolution.

### Modified Capabilities

- `desktop-settings`: Store user-mode preferences in the common application configuration and resolve the same active output deployment as the CLI.
- `collection-identifiers`: Use the persisted default user when an invocation does not provide a user through CLI or environment configuration.

## Impact

- **Target Elastic products:** Initialization configures Elasticsearch as the processed-diagnostic destination and its attached Kibana instance; source hosts may be Elasticsearch, Kibana, or Logstash according to existing collection support.
- **CLI:** Adds `esdiag init` and a shared configuration resolver used by normal CLI commands. Initialization may compose the native `esdiag agent skills` library operation in process as an optional stage. Interactive prompts remain human-oriented, while the final initialization result follows the standard structured CLI outcome contract when that change is available.
- **Local state:** Adds `~/.esdiag/esdiag.yml`, migrates `settings.yml`, and composes existing `hosts.yml`, `secrets.yml`, and `jobs.yml` rather than duplicating their contents.
- **Web UI/Desktop:** User mode reads and updates the common output preference. Service mode remains environment-driven and does not gain local credential persistence.
- **Core processing:** Identifier defaulting and omitted-output resolution gain configuration fallbacks; collection and processing behavior otherwise remains unchanged.
- **Agent assets:** Adds onboarding reference documentation and an optional embedded-skill installation offer. Agents never receive keystore passwords, API keys, or other terminal-entered secrets.
