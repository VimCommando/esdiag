## ADDED Requirements

### Requirement: Three-Panel Diagnostic Workflow
The web home page SHALL present the diagnostic workflow as three distinct panels named `Collect`, `Process`, and `Send`. Each panel SHALL provide exactly two workflow options:
- `Collect`: `Collect` or `Upload`
- `Process`: `Process` or `Forward`
- `Send`: `Remote` or `Local`

The workflow SHALL preserve panel state across user interaction so source selection, processing choices, and delivery choices can be configured together before execution.

#### Scenario: User loads the home page
- **WHEN** the web home page is rendered
- **THEN** the interface shows separate `Collect`, `Process`, and `Send` panels in the primary workflow area
- **AND** each panel exposes its two stage options
- **AND** each panel exposes only the controls relevant to the currently selected option

### Requirement: Collect Stage Options
The `Collect` panel SHALL support `Collect` and `Upload` options. `Collect` SHALL support remote diagnostic intake through a known host in `user` mode, explicit remote URL plus API key, or Elastic Upload Service input. `Upload` SHALL support drag-and-drop and file-picker selection of a local archive.

#### Scenario: User chooses remote collect option
- **WHEN** the user selects `Collect -> Collect`
- **THEN** the panel displays remote intake inputs
- **AND** local upload inputs are hidden or inactive

#### Scenario: User chooses upload option
- **WHEN** the user selects `Collect -> Upload`
- **THEN** the panel displays drag-and-drop and file-picker controls for a local archive
- **AND** remote intake inputs are hidden or inactive

### Requirement: Collect Save Behavior
When `Collect -> Collect` is active, the panel SHALL provide an optional `Save` control that retains the collected archive as a downloadable workflow bundle before downstream stages consume it. The browser workflow SHALL NOT require the user to configure a local filesystem path in order to save the bundle.

#### Scenario: User configures remote collection with retained bundle download
- **WHEN** the user selects `Collect -> Collect`, chooses a diagnostic type, and enables `Save`
- **THEN** the workflow records the selected remote diagnostic type
- **AND** the collected remote archive is retained as a downloadable workflow bundle before downstream workflow stages consume it

#### Scenario: Save auto-initiates browser download from the same Go action
- **GIVEN** the user enables `Save`
- **WHEN** remote collection completes successfully
- **THEN** the workflow initiates bundle download through a separate browser request or action
- **AND** the download is triggered automatically from the same workflow execution without requiring a second manual click
- **AND** the SSE workflow response remains dedicated to workflow status updates rather than file transfer

### Requirement: Process Stage Options
The `Process` panel SHALL support `Process` and `Forward` options. `Process` SHALL expose diagnostic type selection and advanced processor configuration. `Forward` SHALL preserve the raw diagnostic archive unchanged from the collected or uploaded workflow input.

#### Scenario: User chooses processing
- **WHEN** the user selects `Process -> Process`
- **THEN** the panel displays diagnostic type selection and advanced processor configuration
- **AND** downstream execution produces processed diagnostic output

#### Scenario: User chooses forwarding
- **WHEN** the user selects `Process -> Forward`
- **THEN** processing-specific selectors are hidden or inactive
- **AND** downstream execution preserves the raw diagnostic archive unchanged

### Requirement: Send Stage Options
The `Send` panel SHALL support `Remote` and `Local` options. The target semantics for each option SHALL depend on whether the workflow is in `Process` or `Forward` mode.

#### Scenario: User chooses remote send
- **WHEN** the user selects `Send -> Remote`
- **THEN** the panel displays remote delivery inputs compatible with the current process mode

#### Scenario: User chooses local send
- **WHEN** the user selects `Send -> Local`
- **THEN** the panel displays local delivery behavior compatible with the current process mode

### Requirement: Send Panel Owns Output Selection
The workflow SHALL move output target selection from the footer into the `Send` panel. `Remote` and `Local` are UI-level send choices layered over existing output/exporter options rather than a separate exporter system.

#### Scenario: User configures send target in panel
- **WHEN** the user configures the `Send` panel
- **THEN** output target selection is performed inside the panel instead of the footer
- **AND** the chosen send mode maps onto an existing compatible exporter option or uploader capability

### Requirement: Send Target Availability Follows Workflow State
The `Send` panel SHALL derive target availability from the active `Collect` and `Process` selections. Targets that are incompatible with the current workflow state SHALL be disabled before execution and SHALL NOT remain selectable until the workflow returns to a compatible state.

#### Scenario: Forward workflow disables processed send target
- **GIVEN** the workflow is configured to forward a collected or uploaded archive without processing
- **WHEN** the `Send` panel renders available delivery targets
- **THEN** targets intended for processed diagnostic output are disabled
- **AND** archive-compatible delivery targets remain enabled

#### Scenario: Processed workflow disables archive send target
- **GIVEN** the workflow is configured to produce processed diagnostic output
- **WHEN** the `Send` panel renders available delivery targets
- **THEN** archive-only delivery targets are disabled
- **AND** processed-output targets remain enabled when otherwise valid

### Requirement: Remote Send Behavior
When `Send -> Remote` is selected, the workflow SHALL send processed diagnostics to a diagnostic cluster target and SHALL send forwarded raw archives to an Elastic Upload Service endpoint.

#### Scenario: Processed remote send targets diagnostic cluster
- **GIVEN** the workflow is configured for `Process -> Process`
- **WHEN** the user selects `Send -> Remote`
- **THEN** the workflow requires a remote diagnostic cluster target for processed output

#### Scenario: Forward remote send targets upload service
- **GIVEN** the workflow is configured for `Process -> Forward`
- **WHEN** the user selects `Send -> Remote`
- **THEN** the workflow requires an Elastic Upload Service endpoint
- **AND** the raw archive is forwarded unchanged

### Requirement: Local Send Behavior
When `Send -> Local` is selected, processed diagnostics SHALL support local delivery to either a localhost diagnostic cluster target or a local directory. Forwarded raw archives SHALL NOT support a second local send target; instead, the workflow SHALL direct the user to the `Collect` save/download behavior and automatically enable `Save` if it is currently disabled.

#### Scenario: Processed local send targets localhost cluster
- **GIVEN** the workflow is configured for `Process -> Process`
- **WHEN** the user selects `Send -> Local` and chooses a localhost diagnostic cluster target
- **THEN** the target is valid only when the host resolves to `localhost` or `127.0.0.1`

#### Scenario: Processed local send targets directory
- **GIVEN** the workflow is configured for `Process -> Process`
- **WHEN** the user selects `Send -> Local` and chooses directory delivery
- **THEN** the workflow writes processed output to the selected local directory

#### Scenario: Forward local send reuses collect save
- **GIVEN** the workflow is configured for `Process -> Forward`
- **WHEN** the user selects `Send -> Local`
- **THEN** the local send target is disabled
- **AND** the UI states that the local bundle download is handled in `Collect`
- **AND** the workflow automatically enables `Collect` save if it is currently disabled
