## 1. Plugin Package Foundation

- [x] 1.1 Add the plugin package directory with a Claude plugin manifest declaring plugin identity, description, and a semver synchronized with the repository package version.
- [x] 1.2 Bundle the ESDiag operations skill from `.agents/skills/esdiag/` including `references/cli.md` and `references/env-vars.md`, sourced rather than hand-copied.
- [x] 1.3 Add a packaging step that regenerates bundled skill content from `.agents/skills/esdiag/` and fails when the packaged content would diverge.
- [x] 1.4 Add a reference from the operations skill to cluster provisioning, keeping provisioning steps out of the plugin itself.
- [x] 1.5 Verify the plugin installs into a Claude Code instance with no source checkout, no container runtime, and no reachable deployment.

## 2. Client Configuration

- [x] 2.1 Define plugin settings for Kibana base URL, space identifier, API key reference, target agent identifier, optional inference endpoint identifier, and optional saved job name.
- [x] 2.2 Implement configuration resolution with `elastic-ai-agent` as the target agent default and omission of model routing when no inference endpoint is configured.
- [x] 2.3 Scope all Agent Builder request paths to the configured space.
- [x] 2.4 Avoid persisting the Kibana API key in plaintext within the plugin package or its settings output.

## 3. Client Binding Command

- [x] 3.1 Implement the binding command to verify Kibana reachability and API key acceptance without modifying any deployment.
- [x] 3.2 Validate the configured target agent against `GET /api/agent_builder/agents` and list available agents when the configured identifier is absent.
- [x] 3.3 Report `esdiag keystore status` and saved host availability as part of binding output.
- [x] 3.4 Avoid a token-consuming model probe during binding; detect a missing usable model on the first real analysis and report it as a deployment prerequisite without attempting configuration.
- [x] 3.5 Verify the configured API key can read `metrics-*-esdiag*` and `settings-*-esdiag*`, since Agent Builder tools query as the calling identity, and fail binding when data is unreadable.
- [x] 3.6 Ensure the binding command never starts, creates, or reconfigures a deployment and never requires a container runtime.

## 4. Delegated Analysis

- [x] 4.1 Implement analysis requests against `POST /s/{space}/api/agent_builder/converse/async` using the configured agent and optional inference routing.
- [x] 4.2 Consume the SSE event stream and report progress from `reasoning`, `tool_call`, and `tool_result` events while the request runs.
- [x] 4.3 Present the completed message as the analysis, preserving markdown structure and resolving relative dashboard links against the configured Kibana base URL.
- [x] 4.4 Pass the `diagnostic.id` reported by `esdiag` output rather than letting the agent infer the current diagnostic.
- [x] 4.5 Retain the returned `conversation_id` and reuse it for follow-up questions about the same diagnostic; start a new conversation for a different diagnostic.
- [x] 4.6 Treat the response as unstructured markdown and do not derive control flow from its content.
- [x] 4.7 Handle stream interruption without issuing a duplicate analysis request, retaining the conversation identifier for inspection.

## 5. Diagnostic Selection

- [x] 5.1 Classify request intent as reference, collection, or ambiguous, and route to reuse or collection accordingly.
- [x] 5.2 Treat any follow-up within an established analysis conversation as reference intent.
- [x] 5.3 Add a configurable freshness window setting defaulting to 24 hours.
- [x] 5.4 Implement the freshness lookup via Elasticsearch `POST /_query`, with an explicit age parameter and only the required `diagnostic.id` and `event.ingested` fields.
- [x] 5.5 Return the latest diagnostic even when stale, and interpret an empty result as no diagnostic in the queried data stream.
- [x] 5.6 Keep lookup deployment-wide because optional `diagnostic.user` metadata cannot be referenced safely when the field is absent.
- [x] 5.7 Collect without confirmation only on explicit collection intent; when collection is inferred from an ambiguous request, ask first, stating the age of the most recent diagnostic and the host that would be collected from.
- [x] 5.8 Handle a declined collection by offering the most recent existing diagnostic rather than proceeding to collect.
- [x] 5.9 Report the selected diagnostic, its age, and whether it was reused or newly collected.

## 6. Review Command Orchestration

- [x] 6.1 Gate the review command on `esdiag keystore status` and stop with an unlock request when locked.
- [x] 6.2 Run the configured saved job when one exists and extract the resulting `diagnostic.id`.
- [x] 6.3 When no saved job is configured and collection is required, offer to help configure the user's first job instead of failing or silently collecting.
- [x] 6.4 Detect and establish job prerequisites in order — keystore access, a collect-role host, then a send-role output target — reporting each step and why it is required.
- [x] 6.5 Persist the job with `--save-job` during the first run so configuration and the first collection happen together, deriving the default name from the existing `{host}-{action}-{destination}` convention.
- [x] 6.6 Support declining the offer by performing a one-off collect and process without persisting a job, and do not repeat the offer within the session.
- [x] 6.7 Request analysis for the resolved identifier and present the result together with the `Kibana Link` as a clickable markdown link.

## 7. Failure Attribution

- [x] 7.1 Map missing configured agent, rejected authorization, and unreachable endpoint to client configuration failures with actionable guidance.
- [x] 7.2 Map missing Agent Builder license and missing ESDiag assets to cluster provisioning failures with a pointer to the provisioning skill, and map an absent usable model to a deployment prerequisite with a pointer to model setup guidance.
- [x] 7.3 Report an unverifiable `diagnostic.id` as a not-found result without presenting an analysis.
- [x] 7.4 Distinguish a stale diagnostic from empty, partial, malformed, or structurally incomplete freshness responses.

## 8. Verification

- [x] 8.1 Add coverage for configuration resolution: agent default, agent override, space scoping, and omitted model routing.
- [x] 8.2 Add coverage for packaging: bundled skill content matches `.agents/skills/esdiag/`, and divergence fails the build.
- [x] 8.3 Add non-networked coverage for analysis request construction and SSE event handling, including interrupted streams.
- [x] 8.4 Add coverage for `diagnostic.id` and `Kibana Link` extraction from `esdiag` command output.
- [x] 8.5 Add first-pass coverage for intent classification across reference, collection, ambiguous-fresh, and ambiguous-stale cases, asserting that collection occurs only where intended. Boundary tuning is deferred to real user feedback.
- [x] 8.6 Add first-pass coverage for the first-job flow: job persisted during the first run when accepted, and prerequisite ordering when hosts or keystore access are missing. The declined path is prompt behavior deferred to real user feedback.
- [x] 8.7 Verify end to end against a provisioned deployment: keystore gate, job run, streamed progress, relayed analysis, and a follow-up reusing the conversation identifier.
- [x] 8.8 Confirm the freshness lookup calls Elasticsearch directly without an Agent Builder endpoint and that analysis token consumption is attributed to the cluster connector.
- [x] 8.9 Exercise the documented privilege set with a minimally scoped API key to confirm chat authorization and diagnostic data access both succeed.
- [x] 8.10 Run `cargo clippy` and `cargo test` to confirm the change introduces no Rust regressions.

## 9. Documentation

- [x] 9.1 Document client binding as distinct from cluster provisioning, with separate prerequisite lists and failure modes, and state model availability as a deployment prerequisite satisfied through Elastic Inference Service activation or a user-configured LLM provider and connector.
- [x] 9.2 Document the required API key privileges: `feature_agentBuilder.read` and `feature_actions.read` scoped to the ESDiag space, cluster `monitor_inference` when the connector uses the Elasticsearch Inference API, and `read` plus `view_index_metadata` on `metrics-*-esdiag*` and `settings-*-esdiag*`.
- [x] 9.3 Use only `feature_agentBuilder.*` privilege names in guidance and examples, and do not copy deprecated legacy privilege names that still appear in older documentation.
- [x] 9.4 Document cost attribution: analysis spend moves to the cluster's inference connector, and cluster input tokens grow with conversation depth.
- [x] 9.5 Document the intent classification and the freshness window setting, including which phrasings reuse an existing diagnostic, which collect a new one, and when the plugin asks first.
- [x] 9.6 Update `CHANGELOG.md` for the user-visible plugin addition using `.agents/skills/changelog/SKILL.md`.

## 10. Pre-Archive Simplification Audit

- [x] 10.1 Run freshness and diagnostic-access ES|QL directly through Elasticsearch `POST /_query`, with an explicit Elasticsearch URL and no Agent Builder tool-execution wrapper.
- [x] 10.2 Preserve every real analysis and follow-up in Kibana Agent Builder conversation history, key local conversation reuse by deployment, space, agent, and diagnostic, and keep metadata lookups conversation-free.
- [x] 10.3 Capture HTTP and transport failures from streamed analysis requests and attribute authorization, missing-agent, missing-model, and connectivity failures explicitly.
- [x] 10.4 Remove the token-consuming model probe, temporary agent-list copy, unsafe hand-built configuration JSON, and unused configuration helpers.
- [x] 10.5 Replace legacy command files with focused Claude skills and fold first-job onboarding into the cluster-review workflow.
- [x] 10.6 Slim the bundled operations skill and synchronize only Claude-usable skill content and references.
- [x] 10.7 Exercise production scripts through recorded HTTP/SSE fixtures rather than duplicated parsers, and describe prompt-contract checks accurately as structural coverage.
- [x] 10.8 Correct installation, configuration, versioning, conversation-history, and changelog guidance.
- [x] 10.9 Run plugin, marketplace, shell, OpenSpec, and relevant Rust validation before archive. Rust tests pass serially; Clippy completes with pre-existing warnings outside the plugin changes.
