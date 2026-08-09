## MODIFIED Requirements

### Requirement: Persistent Desktop Settings
The system SHALL support mode-aware application configuration persistence. In `user` mode, it SHALL read and write the active output-host reference and other non-secret preferences through the common `~/.esdiag/esdiag.yml` configuration alongside `hosts.yml`. In `service` mode, it SHALL avoid local configuration, credential, and host persistence and only retain allowed runtime preferences in memory.

#### Scenario: User mode persists local settings
- **GIVEN** the web interface is running in `user` mode
- **WHEN** the user configures a custom target host and restarts the application without CLI arguments
- **THEN** the server initializes using the output reference in `esdiag.yml` and its saved host target data

#### Scenario: Service mode does not persist local credentials
- **GIVEN** the web interface is running in `service` mode
- **WHEN** a user updates available preferences from the UI
- **THEN** the system does not write credentials, application configuration, or host target records to local `esdiag.yml`, `settings.yml`, or `hosts.yml` artifacts

#### Scenario: User mode and CLI share output preference
- **GIVEN** user mode selects a saved output host
- **WHEN** a later CLI command omits its output target and no runtime output environment is present
- **THEN** the CLI resolves the same saved output deployment from `esdiag.yml`
