## 1. Embedded Launcher Dispatch

- [ ] 1.1 Render the versioned local launcher during the binary build and embed it as a binary-owned asset.
- [ ] 1.2 Add the `esdiag local <command>` CLI surface, secure temporary launcher dispatch, terminal stream forwarding, exit-status propagation, and embedded-update guidance.
- [ ] 1.3 Add unit and CLI tests for embedded version coherence, argument forwarding, dispatch failures, and `local update` behavior.

## 2. Mode-Aware Local Stack Lifecycle

- [ ] 2.1 Add `--stack=auto|full|core`, compatible host-binary detection, and durable resolved-mode state with safe first-run and explicit-transition behavior.
- [ ] 2.2 Generate and reconcile full and core Compose deployments, including mode-specific image pulls, ports, volumes, readiness, setup, and reset behavior.
- [ ] 2.3 Start, identify, log, restart, and safely stop the managed same-version native `esdiag serve` process for core deployments.
- [ ] 2.4 Define and implement the shared stack-state schema, explicit no-migration mode transitions, and unsupported-mode failure behavior for both entry points.
- [ ] 2.5 Preserve standalone script self-update behavior and add standalone regression coverage for compatible, missing, and mismatched native binaries and both stack modes.

## 3. Initialization and Documentation

- [ ] 3.1 Offer user-approved core-stack startup when local processing initialization finds no usable local deployment, then resume native output setup.
- [ ] 3.2 Update Agent Skill, CLI help, local-stack documentation, release/distribution guidance, and `CHANGELOG.md` for binary-first local-stack use and the managed native web UI.
- [ ] 3.3 Add initialization tests for accepted and declined core-stack startup without secret disclosure or arbitrary helper execution.
- [ ] 3.4 Add cross-entry conformance tests for shared state, endpoints, secure credential handling, asset setup, lifecycle controls, structured native output, and deliberate runtime-user-state separation.

## 4. Verification

- [ ] 4.1 Run `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-features`.
- [ ] 4.2 Run `tests/esdiag-local.sh` and relevant real-Compose validation for full and core modes.
- [ ] 4.3 Run `openspec validate embed-local-stack-launcher --strict`.
