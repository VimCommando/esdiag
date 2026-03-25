## ADDED Requirements

### Requirement: Mode-Aware Remote Collection Inputs
The staged workflow routes SHALL be mounted only in `user` mode for now. Within that user-mode workflow, `Collect -> Collect` SHALL allow selecting from saved known hosts.

#### Scenario: User mode remote collection uses saved host
- **GIVEN** the web interface is running in `user` mode
- **WHEN** the user selects `Collect -> Collect` in the `Collect` panel
- **THEN** the UI offers saved known hosts as selectable remote collection sources

#### Scenario: Service mode does not mount advanced workflow routes
- **GIVEN** the web interface is running in `service` mode
- **WHEN** the user requests `/workflow` or `/jobs`
- **THEN** the server does not mount those routes
- **AND** advanced workflow configuration is deferred until a future design pass

### Requirement: Mode-Aware Bundle Persistence
The user-mode staged workflow SHALL support browser-managed bundle downloads without requiring a user-configured local filesystem save path.

#### Scenario: User mode exposes browser download save behavior
- **GIVEN** the web interface is running in `user` mode
- **WHEN** the user enables `Save Bundle`
- **THEN** the workflow uses browser-managed download behavior
- **AND** the workflow does not require manual local path entry before execution
