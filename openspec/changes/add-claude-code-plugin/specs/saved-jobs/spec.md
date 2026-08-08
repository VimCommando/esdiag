## MODIFIED Requirements

### Requirement: CLI Job Execution
The system SHALL provide `esdiag job run <name>` as a CLI subcommand that loads the named job from `~/.esdiag/jobs.yml` and executes it using the existing CLI collect/process pipeline. CLI job execution SHALL NOT depend on `ServerPolicy`, runtime mode, or `ESDIAG_WEB_FEATURES`.

On success, `esdiag job run` SHALL report what the job produced in its completion summary. A saved job conceals which underlying commands ran, so the summary MUST identify the result the user needs in order to reference it afterwards: the diagnostic identifier for a job that processes and sends, the archive path for a collect-only job, and the upload destination for an upload job. A processing job's summary SHALL carry the same identifier and Kibana link that `esdiag process` reports for an equivalent invocation. A collect-only job's summary MUST NOT imply that a diagnostic was created.

#### Scenario: Run saved job by name
- **WHEN** the user runs `esdiag job run my-job`
- **THEN** the system loads `my-job` from `~/.esdiag/jobs.yml` and executes the full job
- **AND** exits with code 0 on success

#### Scenario: Processing job reports its diagnostic identifier
- **GIVEN** a saved job configured to collect, process, and send to an output target
- **WHEN** the user runs `esdiag job run my-job`
- **THEN** the completion summary reports the diagnostic identifier the run created
- **AND** includes the Kibana link when one is available

#### Scenario: Collect-only job reports its archive path
- **GIVEN** a saved job configured to collect without processing
- **WHEN** the user runs `esdiag job run my-job`
- **THEN** the completion summary reports the path of the collected archive
- **AND** does not report a diagnostic identifier

#### Scenario: Upload job reports its destination
- **GIVEN** a saved job configured to collect and forward to the upload service
- **WHEN** the user runs `esdiag job run my-job`
- **THEN** the completion summary reports the upload destination

#### Scenario: Unknown job name
- **WHEN** the user runs `esdiag job run unknown-name` and that name is not in `jobs.yml`
- **THEN** the system exits with a non-zero code and a clear error message naming the missing job

#### Scenario: Missing jobs file
- **WHEN** `~/.esdiag/jobs.yml` does not exist
- **THEN** `esdiag job run` exits with a non-zero code and an informative error message

#### Scenario: Stale host reference
- **GIVEN** a saved job references a known host that no longer exists in `hosts.yml`
- **WHEN** `esdiag job run` is executed for that job
- **THEN** the system exits with a non-zero code and an error identifying the missing host

#### Scenario: Web feature flags do not affect CLI execution
- **GIVEN** `ESDIAG_WEB_FEATURES` is set to an empty string
- **WHEN** the user runs `esdiag job run my-job`
- **THEN** CLI execution behavior is unchanged
