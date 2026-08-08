## 1. Plugin Package Foundation

- [ ] 1.1 Add the plugin package directory with a Claude plugin manifest declaring plugin identity, version, and description.
- [ ] 1.2 Bundle the ESDiag operations skill from `.agents/skills/esdiag/` including `references/cli.md` and `references/env-vars.md`, sourced rather than hand-copied.
- [ ] 1.3 Add a packaging step that regenerates bundled skill content from `.agents/skills/esdiag/` and fails when the packaged content would diverge.
- [ ] 1.4 Add a reference from the operations skill to cluster provisioning, keeping provisioning steps out of the plugin itself.
- [ ] 1.5 Verify the plugin installs into a Claude Code instance with no source checkout, no container runtime, and no reachable deployment.

## 2. Client Configuration

- [ ] 2.1 Define plugin settings for Kibana base URL, space identifier, API key reference, target agent identifier, optional inference endpoint identifier, and optional saved job name.
- [ ] 2.2 Implement configuration resolution with `elastic-ai-agent` as the target agent default and omission of model routing when no inference endpoint is configured.
- [ ] 2.3 Scope all Agent Builder request paths to the configured space.
- [ ] 2.4 Avoid persisting the Kibana API key in plaintext within the plugin package or its settings output.

## 3. Client Binding Command

- [ ] 3.1 Implement the binding command to verify Kibana reachability and API key acceptance without modifying any deployment.
- [ ] 3.2 Validate the configured target agent against `GET /api/agent_builder/agents` and list available agents when the configured identifier is absent.
- [ ] 3.3 Report `esdiag keystore status` and saved host availability as part of binding output.
- [ ] 3.4 Detect a reachable deployment with no usable model and report it as a deployment prerequisite, pointing to Elastic Inference Service activation or third-party connector setup without attempting configuration.
- [ ] 3.5 Verify the configured API key can read `metrics-*-esdiag*` and `settings-*-esdiag*`, since Agent Builder tools query as the calling identity, and fail binding when data is unreadable.
- [ ] 3.6 Ensure the binding command never starts, creates, or reconfigures a deployment and never requires a container runtime.

## 4. Delegated Analysis

- [ ] 4.1 Implement analysis requests against `POST /s/{space}/api/agent_builder/converse/async` using the configured agent and optional inference routing.
- [ ] 4.2 Consume the SSE event stream and report progress from `reasoning`, `tool_call`, and `tool_result` events while the request runs.
- [ ] 4.3 Present the completed message as the analysis, preserving markdown structure and resolving relative dashboard links against the configured Kibana base URL.
- [ ] 4.4 Pass the `diagnostic.id` reported by `esdiag` output rather than letting the agent infer the current diagnostic.
- [ ] 4.5 Retain the returned `conversation_id` and reuse it for follow-up questions about the same diagnostic; start a new conversation for a different diagnostic.
- [ ] 4.6 Treat the response as unstructured markdown and do not derive control flow from its content.
- [ ] 4.7 Handle stream interruption without issuing a duplicate analysis request, retaining the conversation identifier for inspection.

## 5. Diagnostic Selection

- [ ] 5.1 Classify request intent as reference, collection, or ambiguous, and route to reuse or collection accordingly.
- [ ] 5.2 Treat any follow-up within an established analysis conversation as reference intent.
- [ ] 5.3 Add a configurable freshness window setting defaulting to 24 hours.
- [ ] 5.4 Implement the freshness lookup via `platform.core.execute_esql` through the tool execution endpoint, with an explicit `event.ingested` window and only the confirmed `diagnostic.id`, `event.ingested`, and `diagnostic.user` fields.
- [ ] 5.5 Interpret an empty freshness result as no diagnostic within the window rather than as an absence of diagnostics.
- [ ] 5.6 Prefer scoping the freshness lookup to the current user via `diagnostic.user`, falling back to unscoped when that field is absent.
- [ ] 5.7 Collect without confirmation only on explicit collection intent; when collection is inferred from an ambiguous request, ask first, stating the age of the most recent diagnostic and the host that would be collected from.
- [ ] 5.8 Handle a declined collection by offering the most recent existing diagnostic rather than proceeding to collect.
- [ ] 5.9 Report the selected diagnostic, its age, and whether it was reused or newly collected.

## 6. Review Command Orchestration

- [ ] 6.1 Gate the review command on `esdiag keystore status` and stop with an unlock request when locked.
- [ ] 6.2 Run the configured saved job when one exists and extract the resulting `diagnostic.id`.
- [ ] 6.3 When no saved job is configured and collection is required, offer to help configure the user's first job instead of failing or silently collecting.
- [ ] 6.4 Detect and establish job prerequisites in order — keystore access, a collect-role host, then a send-role output target — reporting each step and why it is required.
- [ ] 6.5 Persist the job with `--save-job` during the first run so configuration and the first collection happen together, deriving the default name from the existing `{host}-{action}-{destination}` convention.
- [ ] 6.6 Support declining the offer by performing a one-off collect and process without persisting a job, and do not repeat the offer within the session.
- [ ] 6.7 Request analysis for the resolved identifier and present the result together with the `Kibana Link` as a clickable markdown link.

## 7. Failure Attribution

- [ ] 7.1 Map missing configured agent, rejected authorization, and unreachable endpoint to client configuration failures with actionable guidance.
- [ ] 7.2 Map missing Agent Builder license and missing ESDiag assets to cluster provisioning failures with a pointer to the provisioning skill, and map an absent usable model to a deployment prerequisite with a pointer to model setup guidance.
- [ ] 7.3 Report an unverifiable `diagnostic.id` as a not-found result without presenting an analysis.
- [ ] 7.4 Distinguish a stale-diagnostic condition from an empty freshness result caused by an unknown field or a missing time window in the query.

## 8. Verification

- [ ] 8.1 Add coverage for configuration resolution: agent default, agent override, space scoping, and omitted model routing.
- [ ] 8.2 Add coverage for packaging: bundled skill content matches `.agents/skills/esdiag/`, and divergence fails the build.
- [ ] 8.3 Add non-networked coverage for analysis request construction and SSE event handling, including interrupted streams.
- [ ] 8.4 Add coverage for `diagnostic.id` and `Kibana Link` extraction from `esdiag` command output.
- [ ] 8.5 Add coverage for intent classification across reference, collection, ambiguous-fresh, ambiguous-stale, and follow-up cases, asserting that collection occurs only where intended and that ambiguous-stale asks before collecting.
- [ ] 8.6 Add coverage for the first-job flow: offer made when no job exists, job persisted during the first run when accepted, one-off collection when declined, and prerequisite ordering when hosts or keystore access are missing.
- [ ] 8.7 Verify end to end against a provisioned deployment: keystore gate, job run, streamed progress, relayed analysis, and a follow-up reusing the conversation identifier.
- [ ] 8.8 Confirm the freshness lookup returns `model_usage: null` and that analysis token consumption is attributed to the cluster connector.
- [ ] 8.9 Exercise the documented privilege set with a minimally scoped API key to confirm chat authorization and diagnostic data access both succeed.
- [ ] 8.10 Run `cargo clippy` and `cargo test` to confirm the change introduces no Rust regressions.

## 9. Documentation

- [ ] 9.1 Document client binding as distinct from cluster provisioning, with separate prerequisite lists and failure modes, and state model availability as a deployment prerequisite satisfied through Elastic Inference Service activation or a user-configured LLM provider and connector.
- [ ] 9.2 Document the required API key privileges: `feature_agentBuilder.read` and `feature_actions.read` scoped to the ESDiag space, cluster `monitor_inference` when the connector uses the Elasticsearch Inference API, and `read` plus `view_index_metadata` on `metrics-*-esdiag*` and `settings-*-esdiag*`.
- [ ] 9.3 Use only `feature_agentBuilder.*` privilege names in guidance and examples, and do not copy deprecated legacy privilege names that still appear in older documentation.
- [ ] 9.4 Document cost attribution: analysis spend moves to the cluster's inference connector, and cluster input tokens grow with conversation depth.
- [ ] 9.5 Document the intent classification and the freshness window setting, including which phrasings reuse an existing diagnostic, which collect a new one, and when the plugin asks first.
- [ ] 9.6 Update `CHANGELOG.md` for the user-visible plugin addition using `.agents/skills/changelog/SKILL.md`.
