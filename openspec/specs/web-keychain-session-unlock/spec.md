## Purpose

Define the web keystore unlock, relock, and availability behavior for keychain-backed operations. Unlock state is shared with the CLI via a file-based lease.

## ADDED Requirements

### Requirement: Secrets Password Unlock for Web Session
The system SHALL require the user to provide the secrets password before any encrypted keychain read or write operation is performed from the web interface.

#### Scenario: Keychain operation attempted while locked
- **WHEN** the user initiates a keychain-backed action and the keystore unlock file is absent or expired
- **THEN** the system prompts for the secrets password and does not perform the keychain operation until unlock succeeds

### Requirement: File-Based Unlock Shared With CLI
The web runtime SHALL write the `keystore.unlock` file on a successful unlock, using the same format and default TTL as `esdiag keystore unlock`. Web lock actions SHALL delete the `keystore.unlock` file. Lock state is determined solely by whether a valid, unexpired `keystore.unlock` file exists on disk; no separate in-memory session state is maintained.

#### Scenario: Successful web unlock writes unlock file
- **WHEN** the user submits a valid secrets password via the web interface
- **THEN** the system writes `keystore.unlock` alongside the active keystore path with a 24-hour TTL
- **AND** subsequent keychain-backed CLI runs and the Agent Skill may read that lease without re-authenticating

#### Scenario: Web lock deletes unlock file
- **WHEN** the user triggers lock from the web interface
- **THEN** the system deletes the `keystore.unlock` file alongside the active keystore path
- **AND** the CLI and web interface both reflect locked state immediately

#### Scenario: Web lock state derived from unlock file
- **GIVEN** no in-memory session state exists
- **WHEN** the web server checks whether the keystore is unlocked
- **THEN** it reads `keystore.unlock` and treats the keystore as unlocked if and only if a valid unexpired lease is found

#### Scenario: CLI unlock is reflected in web interface
- **GIVEN** the user has run `esdiag keystore unlock` from the terminal
- **WHEN** the web interface checks keystore status
- **THEN** it reads the `keystore.unlock` file and shows the keystore as unlocked

#### Scenario: CLI lock is reflected in web interface
- **GIVEN** the web session shows keystore as unlocked
- **AND** the user runs `esdiag keystore lock` from the terminal
- **WHEN** the web interface checks keystore status next
- **THEN** it shows the keystore as locked because the unlock file is gone

### Requirement: Explicit Relock Support
The system SHALL provide an explicit relock action that deletes the `keystore.unlock` file and requires a new secrets password for future keychain-backed actions.

#### Scenario: Relock requested
- **WHEN** the user triggers relock from the web interface
- **THEN** the system deletes the `keystore.unlock` file and marks keychain access as locked for all clients

### Requirement: Bootstrap Creates Unlock Lease
When the web bootstrap flow creates a new keystore after the user confirms creation and sets a password, the system SHALL immediately write a `keystore.unlock` file so the newly bootstrapped process reflects unlocked state without requiring a separate unlock action.

#### Scenario: Bootstrap writes unlock lease after keystore creation
- **GIVEN** no keystore file exists
- **WHEN** the user completes the web bootstrap modal and a new keystore is created
- **THEN** the system writes `keystore.unlock` alongside the new keystore path with a 24-hour TTL
- **AND** the web interface immediately shows the keystore as unlocked

### Requirement: User Menu Keystore Toggle
The system SHALL provide a `Keystore` menu item in the user pop-up menu that toggles behavior by lock state: selecting it while locked prompts for password, and selecting it while unlocked asks for confirmation before relocking.

#### Scenario: Selecting Keystore while locked
- **WHEN** the user clicks `Keystore` from the user menu and the keystore is locked
- **THEN** the system displays a password prompt for unlock

#### Scenario: Selecting Keystore while unlocked
- **WHEN** the user clicks `Keystore` from the user menu and the keystore is unlocked
- **THEN** the system asks the user to confirm locking and locks only after confirmation

### Requirement: Idempotent Lock Lifecycle Endpoints
The system SHALL expose only `/keystore/unlock` and `/keystore/lock` endpoints for lock lifecycle transitions, and both endpoints SHALL be idempotent.

#### Scenario: Repeated unlock request while unlocked
- **WHEN** the user calls `/keystore/unlock` while already unlocked with a valid password
- **THEN** lock state remains unlocked and the unlock file lease is rewritten

#### Scenario: Repeated lock request while locked
- **WHEN** the user calls `/keystore/lock` while already locked
- **THEN** lock state remains locked and the response is successful

### Requirement: Invalid Password Field Feedback
When unlock submission fails due to incorrect password, the system SHALL keep keystore state locked and mark the password input as invalid so the user can retry.

#### Scenario: Incorrect password on unlock attempt
- **WHEN** the user submits an incorrect secrets password in an unlock prompt
- **THEN** the password field is marked invalid and the user is prompted to re-enter the password

### Requirement: Invalid Password HTTP Semantics
An incorrect unlock password SHALL return HTTP 401 from `/keystore/unlock`.

#### Scenario: Wrong password returns unauthorized
- **WHEN** the unlock password fails to decrypt the keystore
- **THEN** `/keystore/unlock` responds with HTTP 401

### Requirement: Failed Unlock Rate Limiting
Failed unlock attempts SHALL be rate limited in memory with no persistence across process restarts: no delay for first 3 failures, then add 5 minutes per failure from the 4th onward, capped at 60 minutes.

#### Scenario: Backoff begins at fourth failure
- **WHEN** the fourth consecutive unlock failure occurs in a process lifetime
- **THEN** the user is delayed by 5 minutes before another unlock attempt is accepted

#### Scenario: Backoff cap is enforced
- **WHEN** additional failures would exceed the maximum delay
- **THEN** enforced delay is capped at 60 minutes

### Requirement: Keystore Availability Gating
Keystore unlock UI and actions SHALL be available only when the application is built with the `keystore` feature enabled and runtime mode is not `service`.

#### Scenario: Feature-disabled build hides keystore unlock controls
- **WHEN** the application is built without the `keystore` feature
- **THEN** the `Keystore` user-menu item and unlock prompts are not rendered

#### Scenario: Service mode disables keystore unlock controls
- **WHEN** runtime mode is `service`
- **THEN** the `Keystore` user-menu item and unlock prompts are not interactive and are hidden or disabled in the UI

### Requirement: Keystore Route Availability Semantics
When keystore is unavailable (feature disabled or runtime mode `service`), `/keystore/*` routes SHALL not be mounted and requests to those paths SHALL return HTTP 404.

#### Scenario: Unlock route absent when unavailable
- **WHEN** a request is sent to `/keystore/unlock` in feature-disabled or `service` mode
- **THEN** the server responds with HTTP 404

### Requirement: Keystore Status Signal Ownership
The backend SHALL own Datastar status signals `keystore.locked` and `keystore.lock_time` (epoch seconds). These fields are UI status only and SHALL be mutable only by backend state transitions, including successful `/keystore/unlock` and `/keystore/lock` responses and lease expiry or external deletion detected on status reads.

#### Scenario: Unlock returns PatchSignals update
- **WHEN** `/keystore/unlock` succeeds
- **THEN** the response includes PatchSignals updates for `keystore.locked` and `keystore.lock_time`

#### Scenario: Lease expiry or external lock publishes locked signal
- **WHEN** a status read detects that a previously valid unlock lease has expired or been deleted externally
- **THEN** the backend publishes `keystore.locked` and `keystore.lock_time` signals to reflect the lock transition

#### Scenario: Client cannot set lock status directly
- **WHEN** a client attempts to mutate `keystore.locked` or `keystore.lock_time` in a request body
- **THEN** the server ignores or rejects the mutation and keeps backend state authoritative

### Requirement: Authentication and Expiry Logging
The system SHALL log successful keystore authentications and lease expiry detections as INFO, and failed authentications as WARN.

#### Scenario: Successful unlock logged
- **WHEN** keystore unlock succeeds
- **THEN** an INFO log event is emitted for successful authentication

#### Scenario: Failed unlock logged
- **WHEN** keystore unlock fails due to invalid password
- **THEN** a WARN log event is emitted for failed authentication

#### Scenario: Lease expiry or external lock logged
- **WHEN** a status read detects the unlock lease has expired or been cleared externally
- **THEN** an INFO log event is emitted for the detected lock transition

### Requirement: Missing Keystore Bootstrap Flow
When keystore storage does not exist, the web UI SHALL prompt the user to create a keystore through the explicit bootstrap modal instead of auto-creating one at process startup.

#### Scenario: Missing keystore opens bootstrap flow
- **WHEN** the application starts in user mode and no keystore file exists
- **THEN** the UI initializes the bootstrap modal flow for explicit keystore creation

#### Scenario: Unlock request falls back to bootstrap flow
- **WHEN** the user requests keystore unlock while no keystore file exists
- **THEN** the system responds with the bootstrap modal rather than auto-creating a keystore
