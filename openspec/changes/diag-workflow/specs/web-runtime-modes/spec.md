## ADDED Requirements

### Requirement: Mode-Aware Remote Collection Inputs
The `Collect` panel SHALL adapt its `Collect -> Collect` inputs to the active web runtime mode. In `user` mode, remote collection SHALL allow selecting from saved known hosts. In `service` mode, remote collection SHALL require explicit endpoint and API key inputs instead of local known-host selection.

#### Scenario: User mode remote collection uses saved host
- **GIVEN** the web interface is running in `user` mode
- **WHEN** the user selects `Collect -> Collect` in the `Collect` panel
- **THEN** the UI offers saved known hosts as selectable remote collection sources

#### Scenario: Service mode remote collection uses explicit credentials
- **GIVEN** the web interface is running in `service` mode
- **WHEN** the user selects `Collect -> Collect` in the `Collect` panel
- **THEN** the UI requires explicit endpoint and API key inputs
- **AND** the workflow does not depend on local known-host settings for the remote source

### Requirement: Mode-Aware Bundle Persistence
The workflow SHALL support browser-managed bundle downloads in both `user` mode and `service` mode. The browser workflow SHALL NOT require a user-configured local filesystem save path in either mode.

#### Scenario: Service mode exposes browser download save behavior
- **GIVEN** the web interface is running in `service` mode
- **WHEN** the user configures `Collect -> Collect` and enables `Save Bundle`
- **THEN** the workflow exposes retained bundle download behavior that does not depend on local persisted host settings or a user-entered save path

#### Scenario: User mode uses the same browser download contract
- **GIVEN** the web interface is running in `user` mode
- **WHEN** the user enables `Save Bundle`
- **THEN** the workflow uses the same browser-managed download behavior as service mode
- **AND** the workflow does not require manual local path entry before execution
