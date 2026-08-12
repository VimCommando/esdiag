## 1. Agent Builder Client Boundary

- [ ] 1.1 Add focused Agent Builder request, SSE event, completion, usage, and safe failure types that compose an existing configured `KibanaClient`.
- [ ] 1.2 Extend `KibanaClient` only as needed for incremental response-body consumption while retaining its existing URL, authentication, TLS, and Kibana header behavior.
- [ ] 1.3 Add recorded response fixtures for conversation IDs, reasoning, tool events, completion, relative links, HTTP errors, unknown events, and interrupted streams.

## 2. Embedded Skill Installer

- [ ] 2.1 Add a `RustEmbed` asset set for canonical ESDiag `SKILL.md`, `references/`, and supported `agents/` metadata that excludes scripts and exposes a deterministic manifest and digest.
- [ ] 2.2 Add Claude Code, Codex, and OpenCode user-scope target adapters with documented home overrides, positive detection, explicit selection, and fixture-backed path tests.
- [ ] 2.3 Implement read-only preflight that classifies missing, exact, intact installer-owned, modified, and unrecognized skill directories before any mutation.
- [ ] 2.4 Implement sibling staging, validation, recoverable backup, atomic replacement where supported, ownership marker written last, and explicit-force conflict handling.
- [ ] 2.5 Add `esdiag agent skills` with automatic multi-target detection, repeatable `--target` selection, guarded `--force`, structured per-target actions, partial-failure context, and restart/reload guidance.
- [ ] 2.6 Verify the canonical skill is included by `cargo package`, embedded in feature combinations that expose the CLI, installable without network access, and byte-equivalent across supported targets.

## 3. Agent Ask Command

- [ ] 3.1 Add `esdiag agent ask <PROMPT>` with `--agent`, `--conversation`, and mutually exclusive `--new` options and default agent `elastic-ai-agent`.
- [ ] 3.2 Resolve the Kibana viewer, space, and authentication exclusively through the canonical output deployment and reject missing or incomplete viewer configuration.
- [ ] 3.3 Submit the prompt to the space-scoped Agent Builder async conversation endpoint without adding inference routing or diagnostic-specific fields.
- [ ] 3.4 Render reasoning and tool progress on stderr and return one finite structured agent-response outcome containing the completed message, conversation ID, Kibana link, and safe optional usage metadata.
- [ ] 3.5 Resolve relative Kibana markdown links against the configured viewer and add exact YAML/JSON outcome tests.
- [ ] 3.6 Implement explicit conversation continuation without local conversation persistence and prove new asks never consult or create a local mapping file.
- [ ] 3.7 Return categorized non-zero structured failures, including `retry_safe: false` plus conversation recovery data after interrupted conversation creation, without automatic retry.

## 4. Portable Skill Simplification

- [ ] 4.1 Update the canonical skill to call `esdiag agent ask`, pass exact diagnostic IDs from structured outcomes when available, and delegate discovery of existing diagnostics to Agent Builder.
- [ ] 4.2 Remove analysis-specific URL, credential, inference, saved-job, freshness, and local-conversation configuration from skill instructions and references.
- [ ] 4.3 Delete `.agents/skills/esdiag/scripts/` and `plugin/skills/esdiag/scripts/`, remove scripts from the sync contract, and assert both directories remain absent from source, generated, and embedded assets.
- [ ] 4.4 Replace shell-helper fixtures with native command and recorded SSE tests, then regenerate the portable skill for Claude Code, Codex, OpenCode, and embedded installation.

## 5. Documentation And Compatibility

- [ ] 5.1 Document `esdiag agent ask`, explicit follow-up, Kibana history handoff, output-deployment prerequisites, inference cost, and interrupted-request recovery.
- [ ] 5.2 Document `esdiag agent skills`, automatic detection, explicit targets, safe updates/conflicts, user scope, offline/version-correct behavior, and agent reload expectations.
- [ ] 5.3 Document that `agent converse`, public SSE/NDJSON proxying, binding, freshness lookup, and local conversation persistence are unsupported and out of scope.
- [ ] 5.4 Update `CHANGELOG.md` using the repository changelog skill and remove obsolete plugin environment-variable guidance.

## 6. Verification

- [ ] 6.1 Run formatting and targeted Kibana client, Agent Builder SSE, embedded asset, installer safety, CLI outcome, redaction, interruption, and portable-skill tests.
- [ ] 6.2 Run `cargo clippy --all-targets --all-features -- -D warnings`.
- [ ] 6.3 Run `cargo test --all-features`.
- [ ] 6.4 Verify no `converse` command or public agent event stream exists and no installed skill references external executable helpers.
- [ ] 6.5 Run strict OpenSpec validation for `add-agent-cli`.
