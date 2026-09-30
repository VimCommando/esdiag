## Purpose

Define collection metadata arguments and preserve those identifiers in diagnostic manifests.

## Requirements

### Requirement: CLI Arguments for Identifiers during Collection
The system SHALL provide CLI arguments for the `collect` command to capture metadata identifiers: `--account` (`-a`), `--case` (`-c`), `--opportunity` (`-o`), and `--user` (`-u`). These MUST mirror the identifier arguments currently available in the `process` command.

#### Scenario: User provides identifiers during collect
- **GIVEN** a collector orchestrator is invoked
- **WHEN** the user runs `esdiag collect --account "Acme" --case "12345" --user "Jane"`
- **THEN** the system captures the provided identifiers in an `Identifiers` object

### Requirement: Recording Identifiers in Diagnostic Manifest
The system SHALL store the provided metadata identifiers within the `DiagnosticManifest` object (e.g., `manifest.json`) generated at the time of collection. 

#### Scenario: Manifest serialization
- **GIVEN** the user provided identifiers during collection
- **WHEN** the collector successfully completes and serializes the `DiagnosticManifest`
- **THEN** the manifest file contains a new `identifiers` property
- **AND** the `identifiers` object includes the values provided via CLI (e.g., `"account": "Acme", "case": "12345"`)

### Requirement: Persisted Default User Identifier
Collection and processing workflows SHALL resolve the diagnostic user identifier using explicit `--user`, then `ESDIAG_USER`, then `ApplicationConfig.user`. An absent value at every level SHALL remain absent, and persisted configuration MUST NOT override an explicit invocation or environment value.

#### Scenario: Persisted user supplies omitted identifier
- **GIVEN** `esdiag.yml` contains `user: reno@example.com`
- **AND** neither `--user` nor `ESDIAG_USER` is supplied
- **WHEN** a collection or processing workflow constructs `Identifiers`
- **THEN** its user identifier is `reno@example.com`

#### Scenario: Explicit user wins
- **GIVEN** `esdiag.yml` and `ESDIAG_USER` both contain default values
- **WHEN** the user invokes a workflow with `--user explicit@example.com`
- **THEN** the diagnostic user identifier is `explicit@example.com`

#### Scenario: Environment user overrides persistence
- **GIVEN** `esdiag.yml` contains a configured user
- **AND** `ESDIAG_USER` contains a different user
- **WHEN** a workflow omits `--user`
- **THEN** the diagnostic user identifier comes from `ESDIAG_USER`
