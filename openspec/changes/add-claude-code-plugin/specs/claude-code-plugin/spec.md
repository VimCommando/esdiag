## ADDED Requirements

### Requirement: Plugin Package Installable Into Claude Code
The project SHALL distribute an ESDiag Claude Code plugin conforming to the Claude plugin specification, installable into a user's local Claude Code instance without a source checkout of this repository. The package MUST declare its identity, semver version, and metadata in a plugin manifest; its version MUST match the repository package version with any development suffix removed. Installation MUST NOT require a container runtime, an Elastic deployment, or repository files.

#### Scenario: Installing without a repository checkout
- **GIVEN** a user has Claude Code and no ESDiag source checkout
- **WHEN** the user installs the published ESDiag plugin
- **THEN** the plugin and its bundled skill become available in that Claude Code instance
- **AND** installation does not require a container runtime or a reachable Elastic deployment

#### Scenario: Installation does not imply a usable cluster
- **GIVEN** the plugin is installed
- **AND** no ESDiag-configured Elastic deployment has been provisioned
- **WHEN** the user invokes a plugin capability that requires a cluster
- **THEN** the plugin reports that client binding or cluster provisioning is incomplete
- **AND** identifies which of the two is missing

### Requirement: Bundled Operations Skill Is Single-Sourced
The plugin SHALL bundle the portable parts of the ESDiag operations skill as its source of `esdiag` command guidance, and those files MUST be sourced from `.agents/skills/esdiag/` rather than maintained as an independent copy. Packaging MUST fail or regenerate when the selected bundled content diverges from the repository skill, and MUST exclude provider-specific metadata that Claude Code does not consume.

#### Scenario: Packaging with divergent skill content
- **GIVEN** the bundled skill content in the plugin package differs from `.agents/skills/esdiag/`
- **WHEN** the plugin package is built
- **THEN** the build regenerates the bundled content from `.agents/skills/esdiag/` or fails
- **AND** does not publish a package whose skill content differs from the repository skill

#### Scenario: Operations guidance available after install
- **GIVEN** the plugin is installed
- **WHEN** the user asks for help running an `esdiag` command
- **THEN** the bundled operations skill provides the command routing and required checks defined in `.agents/skills/esdiag/`

### Requirement: Client Configuration Settings
The plugin SHALL resolve client configuration from environment-backed plugin settings, and MUST support Kibana and Elasticsearch base URLs, a Kibana space identifier, an API key or API-key file reference, a target agent identifier, an optional inference endpoint identifier, and an optional saved job name. The Elasticsearch URL MAY fall back to the existing `ESDIAG_OUTPUT_URL`. The target agent identifier SHALL default to `elastic-ai-agent`. The inference endpoint identifier, when unset, SHALL cause the request to omit model routing so the agent uses its own configured model.

#### Scenario: Default agent applied when unset
- **GIVEN** no target agent identifier is configured
- **WHEN** the plugin constructs an analysis request
- **THEN** the request targets `elastic-ai-agent`

#### Scenario: Configured agent overrides the default
- **GIVEN** a target agent identifier is configured to a non-default value
- **WHEN** the plugin constructs an analysis request
- **THEN** the request targets the configured agent identifier

#### Scenario: Model routing omitted when unset
- **GIVEN** no inference endpoint identifier is configured
- **WHEN** the plugin constructs an analysis request
- **THEN** the request omits inference and connector routing parameters

#### Scenario: Space applied to request paths
- **GIVEN** a Kibana space identifier is configured
- **WHEN** the plugin constructs any Agent Builder request
- **THEN** the request path is scoped to that space

### Requirement: Client Binding Command
The plugin SHALL provide a client-binding capability that verifies a workstation can reach and use an already-provisioned ESDiag deployment. It MUST verify Kibana reachability and API key acceptance, MUST verify that the configured target agent exists, MUST query diagnostic data directly through Elasticsearch `POST /_query`, and MUST report local `esdiag` state when the CLI is available. It MUST NOT create an Agent Builder conversation, invoke a model, create, start, or modify any Elastic deployment, or require a container runtime.

#### Scenario: Binding against a reachable configured cluster
- **GIVEN** a provisioned ESDiag deployment is reachable
- **AND** a valid Kibana API key and existing target agent are configured
- **WHEN** the user runs the client-binding command
- **THEN** the command reports the endpoint, space, and resolved target agent as usable
- **AND** reports current `esdiag` keystore state

#### Scenario: Configured agent does not exist
- **GIVEN** the configured target agent identifier is not present in the configured space
- **WHEN** the user runs the client-binding command
- **THEN** the command reports the configured agent as unavailable
- **AND** lists the agents that do exist in that space
- **AND** does not report binding as complete

#### Scenario: Binding does not provision
- **GIVEN** no Elastic deployment exists at the configured endpoint
- **WHEN** the user runs the client-binding command
- **THEN** the command reports the deployment as unreachable
- **AND** directs the user to cluster provisioning
- **AND** does not attempt to create or start a deployment

### Requirement: Client Binding Verifies Diagnostic Data Access
Because Agent Builder tools execute queries as the calling identity, the client-binding command SHALL verify that the configured API key can read the diagnostic data the analysis agent queries, in addition to verifying chat API authorization. Binding MUST NOT be reported as complete when the key can reach the agent but cannot read diagnostic data.

#### Scenario: Key can reach the agent but not the data
- **GIVEN** the configured API key is accepted by the chat API
- **AND** the key lacks read access to the ESDiag diagnostic data streams
- **WHEN** the user runs the client-binding command
- **THEN** the command reports that diagnostic data is not readable by the configured key
- **AND** identifies the missing index privileges as client configuration
- **AND** does not report binding as complete

#### Scenario: Key has both chat and data access
- **GIVEN** the configured API key is accepted by the chat API
- **AND** the key can read the ESDiag diagnostic data streams
- **WHEN** the user runs the client-binding command
- **THEN** the command reports both chat authorization and data access as usable

#### Scenario: Binding creates no conversation
- **GIVEN** valid client configuration
- **WHEN** the user runs the client-binding capability
- **THEN** diagnostic access checks use Elasticsearch `POST /_query` directly
- **AND** no Agent Builder conversation or model call is created

### Requirement: Client Binding Separate From Cluster Provisioning
The plugin SHALL treat binding a workstation and provisioning a cluster as distinct operations with distinct prerequisites. Plugin guidance MUST reference cluster provisioning rather than implement it, and MUST NOT present container runtime, license, asset setup, or LLM connector configuration as prerequisites of client binding.

#### Scenario: Shared cluster requires binding only
- **GIVEN** a user has been given a Kibana URL and API key for an already-provisioned shared ESDiag deployment
- **WHEN** the user follows plugin setup guidance
- **THEN** the guidance requires only client-side configuration
- **AND** does not require a container runtime or asset setup on the user's workstation

#### Scenario: Missing deployment prerequisites are attributed correctly
- **GIVEN** the configured deployment is reachable but has no usable model configured
- **WHEN** an analysis request fails for that reason
- **THEN** the plugin attributes the failure to a deployment prerequisite
- **AND** does not present it as a client configuration error
- **AND** does not present it as an ESDiag defect

### Requirement: Keystore State Gates Credentialed Operations
The plugin SHALL check `esdiag keystore status` before any operation requiring saved hosts, saved jobs, or stored credentials. A locked keystore SHALL be treated as an expected terminal outcome that stops the operation and asks the user to unlock, not as a retryable error.

#### Scenario: Locked keystore stops the operation
- **GIVEN** `esdiag keystore status` reports the keystore as locked
- **WHEN** the user requests an operation requiring stored credentials
- **THEN** the plugin stops before collecting or processing
- **AND** asks the user to unlock the keystore
- **AND** does not retry or attempt to bypass the lock

#### Scenario: Unlocked keystore proceeds
- **GIVEN** `esdiag keystore status` reports the keystore as unlocked
- **WHEN** the user requests an operation requiring stored credentials
- **THEN** the plugin proceeds without prompting for unlock

### Requirement: Diagnostic Collection Is Intent-Driven
The plugin SHALL decide between reusing an existing diagnostic and collecting a new one based on the intent expressed in the request. A request referring to an existing diagnostic SHALL reuse it without collecting. A request to collect, get, or run a new diagnostic SHALL collect without silently reusing. Only when intent is ambiguous SHALL the plugin decide by the age of the most recent diagnostic. The plugin SHALL collect without confirmation only when collection was explicitly requested, and SHALL ask before collecting when collection was inferred from an ambiguous request. The plugin SHALL report which diagnostic was selected and whether it was reused or newly collected.

#### Scenario: Reference intent reuses without collecting
- **GIVEN** the user refers to their last or a recent diagnostic, or supplies an explicit `diagnostic.id`
- **WHEN** the plugin resolves which diagnostic to analyze
- **THEN** the plugin reuses the referenced diagnostic
- **AND** does not collect a new diagnostic

#### Scenario: Collection intent collects without confirmation
- **GIVEN** the user asks to collect, get, or run a new diagnostic
- **WHEN** the plugin resolves which diagnostic to analyze
- **THEN** the plugin collects a new diagnostic without asking for confirmation
- **AND** does not substitute an existing one

#### Scenario: Ambiguous request with a recent diagnostic
- **GIVEN** the user asks a general question such as how the cluster is looking today
- **AND** the most recent diagnostic is within the configured freshness window
- **WHEN** the plugin resolves which diagnostic to analyze
- **THEN** the plugin reuses that diagnostic
- **AND** reports its age

#### Scenario: Ambiguous request with a stale diagnostic asks first
- **GIVEN** the user asks a general question such as how the cluster is looking today
- **AND** the most recent diagnostic is older than the configured freshness window, or no diagnostic exists
- **WHEN** the plugin resolves which diagnostic to analyze
- **THEN** the plugin asks the user whether to collect a new diagnostic
- **AND** states the age of the most recent diagnostic or that no diagnostic was found
- **AND** names the host it would collect from
- **AND** does not collect before the user answers

#### Scenario: Declining collection does not fall through to collecting
- **GIVEN** the plugin asked whether to collect a new diagnostic
- **WHEN** the user declines
- **THEN** the plugin does not collect
- **AND** offers to analyze the most recent existing diagnostic when one exists

#### Scenario: Follow-up never triggers collection
- **GIVEN** an analysis conversation is already established for a diagnostic
- **WHEN** the user asks a follow-up question
- **THEN** the plugin reuses that diagnostic
- **AND** does not collect a new diagnostic

### Requirement: Diagnostic Freshness Lookup Costs No Model Call
The plugin SHALL determine the age of the most recent diagnostic through Elasticsearch `POST /_query` without invoking an agent or consuming inference tokens. The query MUST evaluate age against an explicit parameter rather than relying on an endpoint time default, and MUST reference only required fields confirmed to exist. It MUST return the latest diagnostic even when stale. Partial, malformed, or structurally incomplete results SHALL make freshness unknown rather than imply that no diagnostic exists.

#### Scenario: Freshness check does not invoke an agent
- **GIVEN** the plugin needs the age of the most recent diagnostic
- **WHEN** it performs the lookup
- **THEN** no agent conversation is created
- **AND** no inference tokens are consumed

#### Scenario: Window stated explicitly
- **GIVEN** a configured freshness window
- **WHEN** the plugin builds the freshness query
- **THEN** the query compares the diagnostic age with that threshold explicitly
- **AND** does not depend on a default time range applied by an execution endpoint

#### Scenario: Stale result retains useful metadata
- **GIVEN** the latest diagnostic is older than the configured threshold
- **WHEN** the plugin interprets the result
- **THEN** it reports the diagnostic identifier and age with `fresh: false`
- **AND** does not collapse the result into an empty response

#### Scenario: Invalid result treated as unknown
- **GIVEN** the freshness response is partial or omits the diagnostic identifier from a populated row
- **WHEN** the plugin interprets the result
- **THEN** it reports that freshness is unknown
- **AND** does not infer that collection is needed

#### Scenario: Empty result means no diagnostic
- **GIVEN** the freshness query returns no rows
- **WHEN** the plugin interprets the result
- **THEN** it reports that no diagnostic was found in the queried data stream

### Requirement: First Saved Job Is Offered, Not Assumed
When a diagnostic must be collected and no saved job is configured, the plugin SHALL offer to help the user configure their first job rather than failing or silently substituting an ad-hoc collect and process pair. The plugin SHALL establish missing prerequisites in order — keystore access, a collect-role host, then an output target with the `send` role — reporting each step and why it is required. The plugin SHALL persist the job as part of the first run rather than requiring a separate configuration-only pass. Declining the offer SHALL remain supported and SHALL proceed with a one-off collect and process.

#### Scenario: No saved job triggers an offer
- **GIVEN** a diagnostic must be collected
- **AND** no saved job is configured
- **WHEN** the plugin resolves how to collect
- **THEN** the plugin offers to help configure a first saved job
- **AND** does not fail
- **AND** does not silently collect without making the offer

#### Scenario: Accepted offer persists the job during the first run
- **GIVEN** the user accepts the offer to configure a first job
- **AND** the required prerequisites are satisfied or established
- **WHEN** the plugin performs the first collection
- **THEN** the job is persisted as part of that run
- **AND** the run produces a diagnostic
- **AND** no separate configuration-only run is required

#### Scenario: Missing prerequisites established in order
- **GIVEN** the user accepts the offer to configure a first job
- **AND** no saved host carries the `collect` role
- **WHEN** the plugin guides configuration
- **THEN** the plugin establishes keystore access, then a collect-role host, then an output target
- **AND** reports each step and why it is required

#### Scenario: Declined offer proceeds one-off
- **GIVEN** the user declines the offer to configure a first job
- **WHEN** the plugin proceeds
- **THEN** the plugin performs a one-off collect and process
- **AND** does not persist a job
- **AND** does not repeat the offer within the same session

### Requirement: Diagnostic Review Skill Orchestration
The plugin SHALL provide a skill that produces a cluster review end to end: verify keystore state when collection is needed, resolve which diagnostic to analyze, obtain one when collection is required, extract the resulting `diagnostic.id`, and request analysis for it. The skill MUST use the identifier reported by `esdiag` rather than asking the agent to infer which diagnostic is current. First-job onboarding SHALL be part of this workflow rather than a separate command.

#### Scenario: Saved job produces an analyzed review
- **GIVEN** the keystore is unlocked and a saved job name is configured
- **WHEN** the user asks for a cluster review requiring collection
- **THEN** the plugin runs the saved job
- **AND** extracts the `diagnostic.id` from the command output
- **AND** requests analysis for that identifier

#### Scenario: Kibana link preserved
- **GIVEN** processing output contains a `Kibana Link: <url>` line
- **WHEN** the plugin reports results
- **THEN** the link is presented to the user as a clickable markdown link
