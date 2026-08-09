## ADDED Requirements

### Requirement: Interactive First-Run Workflow
The CLI SHALL provide `esdiag init` as an interactive staged workflow covering default user identity, keystore creation or unlock, output deployment configuration, the first collect host, optional additional collect hosts, the first saved job, and an optional agent-skill installation offer. The workflow SHALL use in-process domain APIs and MUST NOT invoke another `esdiag` process or an external helper executable.

#### Scenario: New user completes initialization
- **GIVEN** no ESDiag local state exists
- **WHEN** the user completes `esdiag init`
- **THEN** a keystore, linked output hosts, at least one collect host, a default saved job, and `esdiag.yml` are persisted
- **AND** the final result identifies the configured references without exposing credentials

#### Scenario: User adds multiple collection hosts
- **WHEN** the first collect host has been saved and the user elects to add another
- **THEN** initialization repeats the collect-host stage
- **AND** each accepted host is validated and saved independently
- **AND** the user can finish the loop without adding another host

### Requirement: Initialization Is Resumable And Non-Destructive
Initialization SHALL inspect existing state before each stage, reuse valid completed state by default, and require explicit confirmation before replacing a configured user preference, host, secret, or saved job. It SHALL persist only validated domain values and SHALL write `esdiag.yml` last as the completion record.

#### Scenario: Interrupted initialization resumes
- **GIVEN** a prior initialization created the keystore and output hosts but stopped before creating a job
- **WHEN** the user runs `esdiag init` again
- **THEN** the workflow recognizes and offers the valid existing stages
- **AND** resumes at the first incomplete or explicitly changed stage

#### Scenario: Existing job is protected
- **GIVEN** the requested default job name already exists
- **WHEN** initialization would replace it
- **THEN** the user must explicitly confirm replacement
- **AND** declining leaves the saved job unchanged

### Requirement: Secret Input Remains Outside Agent Conversations
Initialization SHALL obtain required keystore passwords, API keys, and host passwords through hidden controlling-terminal prompts or existing secure credential sources. It MUST NOT echo those values, include them in command outcomes or logs, or accept a workflow that requires an agent to receive the values.

#### Scenario: Interactive API key entry
- **WHEN** initialization prompts for an output API key
- **THEN** entered characters are hidden
- **AND** the value is stored through the encrypted keystore API
- **AND** neither stdout nor stderr contains the key

#### Scenario: Required secret without terminal
- **GIVEN** no secure existing credential source supplies a required secret
- **AND** no controlling terminal is available
- **WHEN** `esdiag init` reaches that secret stage
- **THEN** it exits without reading the secret from ordinary stdin
- **AND** reports how to resume from an interactive terminal

### Requirement: Output Deployment Is Linked And Validated
The output stage SHALL create or select an Elasticsearch send host and a Kibana view host, link the send host to the viewer, and validate both endpoints using existing clients before making the deployment active. It SHALL reuse one secret reference by default when both products share authentication and SHALL permit explicitly selected distinct existing secret references.

#### Scenario: Shared output credential is configured once
- **GIVEN** Elasticsearch and Kibana accept the same API key
- **WHEN** the user configures the output deployment
- **THEN** the wizard stores the credential once in the keystore
- **AND** both saved hosts reference that secret
- **AND** the send host's viewer references the Kibana host

#### Scenario: Output validation fails
- **WHEN** either output endpoint rejects its resolved credentials
- **THEN** initialization does not set the output as active
- **AND** preserves any previously active valid output reference

### Requirement: Asset Setup Requires Explicit Approval
Initialization SHALL inspect whether the configured output deployment has the ESDiag assets needed for processing and Agent Builder use. When setup is needed, it SHALL offer the existing setup operation, describe its privilege and license implications, and MUST NOT provision assets unless the user explicitly approves.

#### Scenario: User approves missing asset setup
- **GIVEN** output endpoints validate but required ESDiag assets are absent
- **WHEN** the user approves setup
- **THEN** initialization runs the existing setup behavior against the configured deployment
- **AND** revalidates readiness before continuing

#### Scenario: User declines setup
- **GIVEN** required assets are absent
- **WHEN** the user declines setup
- **THEN** the configured endpoints remain saved
- **AND** initialization reports that processing or Agent Builder use is not yet ready

### Requirement: First Saved Job Produces A Repeatable Workflow
Initialization SHALL create or select a valid first saved job after at least one collect host and an output deployment are configured. The default processing-job path SHALL use the configured collect host and output send host so a run indexes a diagnostic; an explicitly selected collect-only job SHALL remain supported.

#### Scenario: Default processing job is created
- **WHEN** the user accepts the default job shape
- **THEN** the saved job collects from the selected collect host
- **AND** processes to the configured output host
- **AND** its name becomes `default_job` in `esdiag.yml`

#### Scenario: Collect-only job is explicit
- **WHEN** the user selects a collect-only first job
- **THEN** initialization identifies that the job produces an archive rather than indexed diagnostic data
- **AND** persists it only after explicit confirmation

### Requirement: Embedded Agent Skill Installation Is Optional
After the required diagnostic workflow stages validate, initialization SHALL offer to detect supported coding agents and install the running binary's embedded version-matched ESDiag skill using the same in-process service as `esdiag agent skills`. It MUST NOT install without user approval, download skill content, duplicate agent path logic, or make successful skill installation a prerequisite for valid ESDiag configuration.

#### Scenario: User accepts detected skill targets
- **GIVEN** initialization has configured the default saved job
- **AND** one or more supported agent homes are detected
- **WHEN** the user approves skill installation
- **THEN** the embedded skill installer processes the selected targets
- **AND** the initialization result includes each target action and reload guidance

#### Scenario: User declines skill installation
- **WHEN** the user declines the optional agent-skill stage
- **THEN** initialization completes successfully without modifying any agent home
- **AND** reports `esdiag agent skills` as the later standalone installation command

#### Scenario: Skill installation fails after configuration
- **GIVEN** all required ESDiag configuration stages completed
- **WHEN** one selected agent target conflicts or cannot be written
- **THEN** the configured identity, keystore, output deployment, hosts, and job remain valid
- **AND** initialization reports the per-target failure and standalone recovery command
- **AND** does not claim that the core ESDiag configuration failed

### Requirement: Skill Onboarding Is A Local Handoff
The portable ESDiag skill SHALL include `references/onboarding.md` and SHALL direct first-install or missing-configuration cases to a human-run `esdiag init`. The onboarding reference SHALL also identify `esdiag agent skills` as the standalone offline installer for the skill embedded in the binary. The skill MUST NOT reproduce secret-entry steps, manually write ESDiag state files, or request that credentials be pasted into an agent conversation.

#### Scenario: Agent encounters an uninitialized installation
- **WHEN** normal skill use receives a structured missing-configuration outcome
- **THEN** the skill explains that first-run initialization is required
- **AND** directs the user to run `esdiag init` locally
- **AND** does not ask for a password or API key
