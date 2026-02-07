## ADDED Requirements

### Requirement: Kibana Metadata Extraction
The system SHALL extract common metadata from the diagnostic bundle to enrich all exported Kibana documents.

#### Scenario: Successful metadata extraction
- **WHEN** a Kibana diagnostic is initialized
- **THEN** the system extracts `diagnostic.*` info (ID, version, timestamp) and `node.*` info (name, version) to be used as a shared context for all processors.
