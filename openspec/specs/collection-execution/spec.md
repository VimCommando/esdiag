## ADDED Requirements

### Requirement: API Deduplication
The system SHALL ensure that no API identifier appears more than once in the final resolved list of APIs to collect.

#### Scenario: Explicit inclusion overlaps with dependency
- **GIVEN** a diagnostic type that already includes `nodes`
- **WHEN** the user runs `esdiag collect --include nodes_stats,nodes`
- **THEN** the system resolves the final list of APIs
- **AND** the `nodes` API is only executed once during the collection phase

### Requirement: Safety-Aware Execution Concurrency
The system SHALL classify registered APIs as either `Heavy` or `Light`. `Heavy` APIs MUST be executed strictly sequentially to protect the target cluster from excessive load. `Light` APIs MAY be executed concurrently (with a bounded concurrency limit) to improve collection speed.

#### Scenario: Executing a mix of APIs
- **GIVEN** `nodes_stats` is classified as `Heavy` and `cluster_health` is classified as `Light`
- **WHEN** the system begins the execution phase of the collection
- **THEN** the `nodes_stats` API is fetched sequentially without other APIs executing concurrently
- **AND** the `cluster_health` API can be fetched concurrently alongside other `Light` APIs (e.g., `licenses`)

### Requirement: Graceful API Retries
The system SHALL implement a graceful retry mechanism for individual API fetch failures during collection. If a fetch fails due to a transient error, the system MUST retry the fetch using an exponential backoff timer for up to 5 minutes and log a warning.

#### Scenario: API fetch encounters a timeout
- **GIVEN** the collection execution loop is attempting to fetch `indices_stats`
- **WHEN** the HTTP request to the cluster times out
- **THEN** the system logs a warning detailing the failure
- **AND** the system retries the `indices_stats` request using exponential backoff
- **AND** if the retries continue to fail for 5 minutes, the system continues to the next API in the queue rather than aborting the entire collection run

### Requirement: Exhaustive API Matching
The system MUST implement exhaustive pattern matching when mapping the generic API enum to the concrete fetch/save execution logic to prevent unhandled APIs at compile time.

#### Scenario: Developer adds a new API enum variant
- **GIVEN** a developer adds a new variant `IndicesRecovery` to the `ElasticsearchApi` enum
- **WHEN** they attempt to compile the `esdiag` CLI
- **THEN** the Rust compiler issues an error because the new variant is not handled in the exhaustive `match` statement within the collection execution loop

### Requirement: Role-Constrained Execution Targets
The collection execution workflow SHALL resolve host targets by role before executing each workflow phase. The collect phase SHALL use only hosts with the `collect` role, the send phase SHALL use only hosts with the `send` role, and the view phase SHALL use only hosts with the `view` role.

#### Scenario: Resolve targets for multi-phase workflow
- **GIVEN** host configuration includes hosts with `collect`, `send`, and `view` roles
- **WHEN** the workflow resolves targets for collection and output handling
- **THEN** collection calls are executed only against `collect` hosts
- **AND** send/output calls are executed only against `send` hosts
- **AND** view target resolution includes only `view` hosts

### Requirement: Remote Collection Bundle Persistence
The workflow SHALL support optionally retaining a remotely collected diagnostic archive as a downloadable bundle before later workflow stages execute. If bundle saving is disabled, the workflow MAY continue with the in-memory or temporary workflow input without retaining a downloadable copy.

#### Scenario: Save retains a remotely collected bundle for download
- **GIVEN** the user starts a remote diagnostic collection and enables `Save Bundle`
- **WHEN** the collection completes successfully
- **THEN** the system retains the collected archive as a downloadable bundle
- **AND** subsequent processing or send steps consume that retained archive or its equivalent normalized workflow input

#### Scenario: Saved workflow bundle downloads outside the SSE stream
- **GIVEN** the workflow enables archive saving during remote collection
- **WHEN** the collected archive is ready for download
- **THEN** the browser fetches the bundle through a separate HTTP request or browser action
- **AND** the workflow status stream continues independently over SSE

### Requirement: One-Job and Two-Job Workflow Modes
The workflow SHALL support both a single-job on-demand path and a two-job saved-bundle path. `Collect -> Collect -> Process -> Send` without save SHALL preserve the current on-demand API retrieval behavior as one job. When save is enabled, collection SHALL complete as one job and processing-plus-send SHALL run as a second job using the retained downloadable archive bundle.

#### Scenario: Unsaved collect-process-send remains on-demand
- **GIVEN** the user selects remote collection followed by processing and send
- **AND** save is disabled
- **WHEN** the workflow executes
- **THEN** collection, processing, and send run as the current on-demand flow without creating an intermediate saved archive job boundary

#### Scenario: Saved collect-process-send becomes two jobs
- **GIVEN** the user selects remote collection followed by processing and send
- **AND** save is enabled
- **WHEN** the workflow executes
- **THEN** collection completes as its own job that retains an archive bundle for download
- **AND** processing and send run as a second job consuming that retained archive

### Requirement: Collect-Without-Process Workflow
The workflow SHALL support sending a collected diagnostic without invoking processing when the `Process` stage is configured for forwarding.

#### Scenario: Collect and send without processing
- **GIVEN** the user has configured a valid collect source
- **AND** the `Process` stage is configured for forwarding
- **WHEN** the workflow runs through collection and send
- **THEN** the system completes the collect stage without creating processed diagnostic documents
- **AND** the send stage receives the collected archive as its workflow input
