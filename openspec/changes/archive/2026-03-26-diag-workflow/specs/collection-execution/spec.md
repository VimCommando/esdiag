## ADDED Requirements

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
