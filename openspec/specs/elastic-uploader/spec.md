# elastic-uploader

## Purpose

Defines the Elastic Upload Service uploader capability for raw diagnostic bundles, including CLI entry points and workflow integration for forward remote send.

## ADDED Requirements

### Requirement: Upload Command For Raw Diagnostic Bundles
The system SHALL provide a CLI command `esdiag upload <file_name> <upload_id>` for sending an unprocessed diagnostic bundle to Elastic Upload Service.

#### Scenario: User uploads a diagnostic bundle from CLI
- **GIVEN** a local diagnostic archive file and an Elastic Upload Service upload identifier
- **WHEN** the user runs `esdiag upload <file_name> <upload_id>`
- **THEN** the system uploads the unprocessed diagnostic bundle to Elastic Upload Service

### Requirement: Workflow Uses Elastic Uploader For Forwarded Remote Send
When the workflow is configured for `Process -> Forward` and `Send -> Remote`, the system SHALL use the Elastic Upload Service uploader capability instead of the processed-diagnostic exporter path.

#### Scenario: Forwarded archive uses uploader capability
- **GIVEN** the workflow is configured to forward a raw archive remotely
- **WHEN** the user executes the send step
- **THEN** the system invokes the Elastic Upload Service uploader path for the archive
- **AND** it does not invoke processed-document export behavior

### Requirement: Upload Command Preserves Raw Archive
The uploader capability SHALL send the raw diagnostic bundle unchanged. It SHALL NOT attempt to process the archive into diagnostic documents before upload.

#### Scenario: Raw archive remains unprocessed during upload
- **GIVEN** a diagnostic archive selected for uploader delivery
- **WHEN** the upload command or workflow uploader path runs
- **THEN** the archive bytes are uploaded as-is
- **AND** no processor pipeline is executed before upload

### Requirement: Collect Command Reuses Elastic Uploader
When `esdiag collect` is invoked with `--upload`, the system SHALL reuse the Elastic Upload Service uploader capability to upload the collected raw diagnostic bundle after collection succeeds.

#### Scenario: Collect hands off a raw bundle to the uploader
- **GIVEN** a collect run has completed successfully and produced a local diagnostic archive
- **AND** the user provided `--upload <upload_id>` on the collect command
- **WHEN** the upload handoff begins
- **THEN** the system invokes the Elastic Upload Service uploader capability for the collected archive
- **AND** the uploader sends the raw archive bytes unchanged

### Requirement: Collect Upload Failure Surfaces After Successful Collection
If the collect step succeeds and the upload handoff fails, the system MUST report the upload failure from the collect command while preserving the already collected local archive.

#### Scenario: Upload fails after archive collection succeeds
- **GIVEN** the collect step has already written a local diagnostic archive successfully
- **AND** the user provided `--upload <upload_id>` on the collect command
- **WHEN** the Elastic Upload Service uploader fails during upload validation, transfer, or finalize
- **THEN** the collect command returns an error for the failed upload step
- **AND** the previously collected local archive remains available for retry or inspection
