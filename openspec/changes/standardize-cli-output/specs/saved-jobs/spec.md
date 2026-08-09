## MODIFIED Requirements

### Requirement: CLI Job Listing
The system SHALL provide `esdiag job list` as a CLI subcommand that emits a structured saved-job list from `~/.esdiag/jobs.yml`. Each job entry SHALL include its name, collection target, processing selection, executable action, send target, and intermediate bundle-retention configuration when present. CLI job listing SHALL NOT depend on `ServerPolicy`, runtime mode, or `ESDIAG_WEB_FEATURES`.

#### Scenario: List saved jobs
- **WHEN** the user runs `esdiag job list` and `jobs.yml` contains entries
- **THEN** the system emits a YAML job-list outcome by default and exits with code 0
- **AND** each saved job appears as one typed entry
- **AND** no terminal table or ANSI color is present

#### Scenario: List with no saved jobs
- **WHEN** the user runs `esdiag job list` and `jobs.yml` does not exist or is empty
- **THEN** the system emits a successful job-list outcome containing `jobs: []`
- **AND** exits with code 0

#### Scenario: Web feature flags do not affect CLI listing
- **GIVEN** `ESDIAG_WEB_FEATURES` is set to an empty string
- **WHEN** the user runs `esdiag job list`
- **THEN** CLI listing behavior and its structured result are unchanged

## ADDED Requirements

### Requirement: CLI Saved Job Operations Return Typed Outcomes
Saved-job run and delete operations SHALL return typed CLI outcomes. A run outcome SHALL preserve whether the job collected, uploaded, or processed; deletion SHALL identify the removed job by name without returning its persisted configuration or any credential material.

#### Scenario: Collect-only job reports archive
- **WHEN** `esdiag job run <name>` completes a collect action
- **THEN** its structured result identifies a collected job outcome
- **AND** contains the resolved collected archive path

#### Scenario: Upload job reports destination
- **WHEN** `esdiag job run <name>` completes an upload action
- **THEN** its structured result identifies an uploaded job outcome
- **AND** contains the collected archive path and resolved upload destination

#### Scenario: Process job reports diagnostic
- **WHEN** `esdiag job run <name>` completes a process action
- **THEN** its structured result identifies a processed job outcome
- **AND** contains the primary diagnostic result and all included diagnostic outcomes

#### Scenario: Delete reports affected job
- **WHEN** `esdiag job delete <name>` succeeds
- **THEN** its structured result identifies `<name>` as deleted
- **AND** does not expose the removed job's credential references beyond safe identifiers
