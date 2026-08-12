## ADDED Requirements

### Requirement: General Application Configuration
The system SHALL persist local non-secret preferences in a versioned `~/.esdiag/esdiag.yml` file. The configuration SHALL contain a default user identifier, output-host reference, and default saved-job reference when configured, and MUST NOT contain endpoint credentials, decrypted secrets, authorization headers, or embedded host and job definitions.

#### Scenario: Configuration contains references only
- **GIVEN** initialization configured an output deployment and default saved job
- **WHEN** `esdiag.yml` is serialized
- **THEN** it contains the output host name and default job name
- **AND** the referenced host and job bodies remain in `hosts.yml` and `jobs.yml`
- **AND** credential material remains only in `secrets.yml`

#### Scenario: Unknown configuration version is rejected
- **GIVEN** `esdiag.yml` declares a schema version newer than the running binary supports
- **WHEN** the application loads local configuration
- **THEN** loading fails with an actionable compatibility error
- **AND** no configuration file is rewritten

### Requirement: Canonical Output Deployment Resolution
The system SHALL resolve the processed-diagnostic output as one atomic deployment containing an Elasticsearch send target, its Kibana view target when required, and authentication from the same configuration source. Resolution precedence SHALL be explicit command target, complete `ESDIAG_OUTPUT_*` and `ESDIAG_KIBANA_URL` runtime configuration, persisted output-host reference, then configuration failure.

#### Scenario: Runtime deployment overrides persisted output
- **GIVEN** `esdiag.yml` references a saved output host
- **AND** a complete `ESDIAG_OUTPUT_URL`, output authentication, and `ESDIAG_KIBANA_URL` are present
- **WHEN** an omitted-output command resolves its deployment
- **THEN** it uses the environment-backed Elasticsearch and Kibana endpoints
- **AND** it applies the output authentication to both products
- **AND** it does not combine either endpoint with the saved deployment

#### Scenario: Persisted output resolves linked viewer
- **GIVEN** `esdiag.yml` references a saved Elasticsearch host with role `send`
- **AND** that host references a saved Kibana host with role `view`
- **WHEN** an operation requiring Elasticsearch and Kibana resolves the output deployment
- **THEN** it uses the saved send host and linked viewer host
- **AND** resolves their referenced secrets through the existing keystore

#### Scenario: Partial runtime deployment fails closed
- **GIVEN** `ESDIAG_OUTPUT_URL` selects an environment-backed deployment
- **AND** an operation requires Kibana but `ESDIAG_KIBANA_URL` is absent
- **WHEN** output deployment resolution runs
- **THEN** it fails with a missing runtime Kibana configuration error
- **AND** it does not borrow the persisted output host's viewer

### Requirement: Shared Output Authentication Contract
Environment-backed output configuration SHALL use `ESDIAG_OUTPUT_APIKEY` or the existing output username/password pair for both Elasticsearch and Kibana. The system MUST NOT require or recognize analysis-specific Elasticsearch URL, Kibana API-key, or Kibana API-key-file variables as a second output deployment configuration.

#### Scenario: API key authenticates both output products
- **GIVEN** `ESDIAG_OUTPUT_URL`, `ESDIAG_KIBANA_URL`, and `ESDIAG_OUTPUT_APIKEY` are configured
- **WHEN** the output deployment constructs its Elasticsearch and Kibana clients
- **THEN** both clients use the configured output API key
- **AND** no duplicate Kibana credential is required

### Requirement: Legacy Settings Migration
User mode SHALL migrate representable preferences from legacy `settings.yml` into `esdiag.yml` and SHALL stop writing `settings.yml` after successful migration. It SHALL preserve a backup until the new configuration validates and MUST require explicit resolution when a legacy Kibana URL cannot be associated with the selected output host's viewer.

#### Scenario: Representable settings migrate
- **GIVEN** `settings.yml` names a valid send host whose linked viewer matches its Kibana URL
- **AND** `esdiag.yml` does not exist
- **WHEN** local application configuration loads
- **THEN** the active target is migrated to the output reference in `esdiag.yml`
- **AND** the new configuration is validated before legacy writes stop

#### Scenario: Ambiguous Kibana URL requires initialization
- **GIVEN** legacy `settings.yml` contains a Kibana URL unrelated to the active send host's viewer
- **WHEN** migration is attempted
- **THEN** the system preserves the legacy file and reports that viewer selection is required
- **AND** it does not silently pair endpoints from different deployments
