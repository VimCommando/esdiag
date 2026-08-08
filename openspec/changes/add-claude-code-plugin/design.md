## Context

ESDiag's agent-facing assets currently terminate at two disconnected endpoints.

On the client side, `.agents/skills/esdiag/` teaches an agent to drive the CLI: manage hosts, unlock the keystore, run `collect` and `process`, and surface the `Kibana Link` that `process` prints. On the cluster side, `esdiag setup` (`src/setup.rs`) installs a space, dashboards, tools, workflows, and the `agentic-diagnostic-assistant` skill, then attaches that skill to the space's `elastic-ai-agent` via `attach_skills_to_default_agent`.

Verified against the production cluster, the default agent in the `esdiag` space carries:

```json
"skill_ids": ["agentic-diagnostic-assistant","office_hours_skill","autoops_triage"],
"tools": [{"tool_ids": ["platform.core.execute_esql","user_diagnostic_id_fetcher",
                        "platform.core.product_documentation","platform.core.generate_esql"]}]
```

That is the live production configuration and it matches the repository's design intent: the skill supplies the analytical knowledge and its `references/`, while a small set of generic tools performs retrieval. A separate `ada_orchestrator_agent` exists in the same space using pinned per-domain workflow tools instead of the skill; it is a parallel variant and not the target of this change.

The missing piece is a client that can ask that agent a question from the terminal and render the answer.

## Goals / Non-Goals

**Goals:**
- Let a user ask "how is my cluster looking today?" in Claude Code and receive ADA's analysis without opening Kibana.
- Keep the analytical knowledge single-sourced on the cluster, so the answer always reflects the cluster's installed assets.
- Place analysis token spend on the cluster's inference connector rather than the user's local model quota.
- Keep the user informed during a multi-second analysis instead of blocking silently.
- Keep client binding independent of cluster provisioning.

**Non-Goals:**
- Adding an `esdiag` CLI subcommand that speaks to an LLM. Explicitly out of scope; it would put a conversation client inside a diagnostics tool and create a dependency on Kibana's agent runtime rather than just its saved objects.
- Provisioning clusters or licenses. A separate provisioning skill owns this; the operations skill references it.
- Configuring model access. Activating the Elastic Inference Service through Cloud Connect, or standing up a third-party LLM provider and connector, is a deployment prerequisite the user satisfies before binding. It is out of scope for this change and is not owned by the provisioning skill either.
- Adding Kibana workflow or tool assets. The chosen transport requires none.
- Reproducing ADA's analysis locally, in whole or in part.
- Structured or schema-constrained analysis output. Not available on the chosen transport; see below.

## Verified Constraints

Every decision below rests on behavior confirmed against a live Kibana 9.4.2 deployment. These findings are recorded because each one invalidates an approach that appears correct from the documentation alone.

### The Agent Builder MCP server exposes tools, not skills or agents

The MCP endpoint `/api/agent_builder/mcp` surfaces the tool registry. There is no `invoke_skill` or `ask_agent` tool. Skills are injected server-side into an agent's reasoning loop and are not addressable over MCP.

Consequence: an MCP-only integration cannot use the cluster's ADA skill. It would give Claude `platform.core.execute_esql` and require a local copy of the analytical knowledge — the exact drift and local-token outcome this change exists to avoid. MCP remains useful for cheap non-agent lookups but nothing in this change depends on it.

### `ai.agent`'s `agent-id` is not templatable

Kibana Workflows offer an `ai.agent` step that invokes an agent, which would place analysis behind an MCP-visible workflow tool. Its `agent-id` is a top-level step key, and top-level keys are not template-expanded. A dry run with `agent-id: "{{ inputs.agent_id }}"` failed with:

```
Agent "{{ inputs.agent_id }}" not found or not available
```

Consequence: a workflow-tool transport cannot satisfy runtime-configurable agent selection. The agent id would have to be rendered into the workflow YAML at `esdiag setup` time, moving configuration to the cluster and out of plugin settings. `inference-id` is likewise a top-level key and inherits the same limitation.

### `/converse` rejects a `schema` parameter

Structured output is available on the `ai.agent` workflow step (verified: a schema-constrained step validates and completes). It is not available on the chat API:

```
400 [request body.schema]: Additional properties are not allowed ('schema' was unexpected)
```

Consequence: choosing the chat API for configurability forfeits structured output. Analysis is relayed as prose. This is acceptable because ADA's own response rules already mandate a leading verdict, grouped findings, comparison tables, human-readable units, and one relative dashboard link per reference used.

### `/converse/async` streams progress events

The streaming variant emits a usable event sequence:

```
conversation_id_set → reasoning → tool_call → tool_result → reasoning → …
→ thinking_complete → message_chunk × N → message_complete → round_complete
```

Consequence: the client can report which tool the agent is running as it happens. This resolves the blocking-UX objection without the workflow-based start/poll design that would otherwise have been required. Elastic's A2A server was considered and rejected here: it does not support streaming, and it offers no advantage while Claude Code is the only client.

### Asynchronous workflow execution is already builtin

`platform.core.get_workflow_execution_status` is a builtin tool, and `POST /api/workflows/workflow/{id}/run` returns a `workflowExecutionId` immediately with step-level status available during execution. Recorded because it means a future workflow-based variant would need no custom polling assets — not because this change uses it.

`POST /api/workflows/test` accepts `{workflowYaml, inputs}`, executes with `isTestRun: true`, and persists nothing. Recorded as a viable CI hook should workflow assets be added later.

### Required API key privileges, and the identity tools run as

The chat API route itself requires only `agentBuilder:read`, confirmed both from the route documentation for `/converse` and `/converse/async` and from the deployment's own feature definition:

```json
"agentBuilder": {"privileges": {"read": {"api": ["agentBuilder:read"], "ui": ["show"]}}}
```

Route authorization is not the whole requirement. Agent Builder documents privileges at three levels, and the decisive sentence is about index access:

> "Tools execute queries against Elasticsearch indices **as the current user**."

Consequence: a key holding only `feature_agentBuilder.read` authenticates successfully and the agent runs, but its `platform.core.execute_esql` calls execute as that key and fail or return nothing. The failure surfaces as a degraded or empty analysis rather than an authorization error, which is the worst possible failure shape. The key must carry read access to the diagnostic data itself.

The minimum working privilege set for this plugin, which only uses an existing agent and creates no assets:

| Level | Privilege | Why |
|---|---|---|
| Kibana application, scoped to the ESDiag space | `feature_agentBuilder.read` | Use agents, send chat messages, view skills and tools, access conversations |
| Kibana application, scoped to the ESDiag space | `feature_actions.read` | Required when the agent uses an AI connector |
| Elasticsearch cluster | `monitor_inference` | Required only when the connector calls the Elasticsearch Inference API; not required for other Kibana GenAI connectors |
| Elasticsearch indices | `read`, `view_index_metadata` on `metrics-*-esdiag*` and `settings-*-esdiag*` | Tools query diagnostic data as the caller |

`feature_agentBuilder.all` is not required. Skills, tools, and agents are installed by `esdiag setup`, not by the plugin.

The index patterns follow from the ADA references, which query `metrics-diagnostic-esdiag*`, `metrics-index-esdiag*`, `metrics-node-esdiag*`, `metrics-shard-esdiag*`, `metrics-ingest.pipeline-*`, `metrics-ingest.processor-esdiag*`, `metrics-task-*`, and `settings-node*`. Confirmed against the deployment, every ESDiag data stream carries an `-esdiag` suffix, so the two patterns above cover all eight reference patterns as they resolve while excluding unrelated `metrics-*` data such as Fleet Server metrics.

API keys use the `feature_agentBuilder.*` privilege names shown above. Some published documentation still shows legacy privilege names carried over from the feature's earlier internal naming; those are deprecated and must not be copied into guidance or examples.

Verified with minimally scoped keys against a local stack. A key holding exactly the four privileges above reaches both `agent_builder/agents` and `actions/connectors` and can query the diagnostic data. A key holding only the two Kibana application privileges, with no index privileges, is accepted for chat and fails only on data access — confirming the predicted failure shape, and confirming that binding must check data access separately rather than treating chat authorization as sufficient.

### Model availability cannot be enumerated from a Kibana URL

Checking whether a deployment has a usable model looks like a listing problem and is not one.

`GET /api/actions/connectors` returns Kibana action connectors. On a deployment whose models come from the Elastic Inference Service through Cloud Connect, that list is **empty even for a superuser**, while the agent works perfectly: EIS models are Elasticsearch inference endpoints, visible at `GET _inference` with service `elastic` and task type `chat_completion`, not Kibana connectors. Verified against a local 9.4.2 stack with EIS connected — `_inference` listed the Anthropic model family while the connector list stayed empty.

No Kibana route exposes those endpoints. `api/inference/_inference`, `internal/inference/_inference`, `api/ml/inference_endpoints`, `internal/ml/inference_endpoints`, `api/agent_builder/models`, and `internal/agent_builder/models` all return 404 on 9.4.2. Since the client holds a Kibana URL and not an Elasticsearch one, enumeration cannot be made reliable.

Consequence: the binding command probes the capability instead of inferring it, by issuing one minimal `converse` request and reading the outcome. This is authoritative — it exercises the same path analysis uses — and it reports which model actually answered. It costs roughly 8,500 input tokens on the deployment per run, which is why it can be skipped with `--no-model-check`.

A listing-based check would have passed on the deployment where connectors happen to exist and failed on the EIS deployment that works, which is the wrong answer in both directions.

### Measured token attribution

| Request | Cluster input | Cluster output | LLM calls | Wall clock | Returned to client |
|---|---:|---:|---:|---:|---:|
| Trivial prompt, no tools | 8,507 | 103 | 2 | ~7s | ~10 tokens |
| Full ADA analysis of one diagnostic | 109,622 | 3,589 | 6 | 62s | ~1,100 tokens |
| Follow-up on same conversation | 147,483 | 2,916 | 6 | — | ~80 tokens |

The step trace for the full analysis confirms the skill firing as designed: four `filestore.read` calls pulling its own `references/`, then seven `platform.core.execute_esql` calls.

Two implications. First, the ~8.5k input-token floor is the agent's system prompt and tool definitions, re-sent every request; short questions are not cheap on the cluster. Second, conversation history replays on each turn, so cluster input grows with depth while client cost stays approximately flat. Deep sessions are inexpensive locally and progressively more expensive on the cluster.

## Decisions

### Transport: Agent Builder chat API over HTTPS, not MCP and not A2A

The client issues `POST /s/{space}/api/agent_builder/converse/async` and consumes the SSE stream. Given that an `esdiag` conversation subcommand is out of scope and Claude Code speaks MCP but not A2A or arbitrary REST, the remaining options were a bundled MCP shim process or direct HTTPS from the skill. Direct HTTPS wins: it needs no additional runtime, no second place for credentials to live, and no duplication of host resolution.

This is the decision that makes the change small. No workflow assets, no tool assets, no new Rust, no shim process.

### Agent selection is configuration, not a constant

`ESDIAG_AGENT_ID` defaults to `elastic-ai-agent` because that is where `esdiag setup` attaches the ADA skill. It must remain overridable: the production cluster demonstrates both a renamed default agent and a second diagnostic agent in the same space, and nothing prevents an operator from attaching ADA elsewhere. A misconfigured agent id produces a plausible-looking but materially worse answer rather than an error, so the binding command validates the configured id against `GET /api/agent_builder/agents` at bind time rather than deferring the failure to first use.

### Analysis output is relayed, not re-derived

The client presents the agent's markdown as the analysis. It does not re-run the underlying ES|QL, recompute metrics, or substitute its own thresholds. Dashboard links are relative by ADA's own rules and are resolved against the configured Kibana URL for presentation. Relaying rather than re-deriving is what keeps the analysis faithful to the cluster's installed assets and keeps local cost at ~1,100 tokens.

### Diagnostic identity comes from `esdiag`, not from the agent

`format_process_summary` in `src/main.rs` already emits the identifier:

```
process complete in 4.212 seconds: 18432 documents for <diagnostic.id>
Kibana Link: <url>
```

That value is passed to the agent. ADA gates on `diagnostic.id` verification and will otherwise call `user_diagnostic_id_fetcher` to disambiguate, which costs a round trip and risks selecting a different diagnostic than the one just processed. Passing the known identifier avoids both.

### Collecting a new diagnostic is driven by intent, not by default

Collection is the expensive, outward-facing half of the workflow: it issues API calls against a live production cluster. Reuse is nearly free. The request itself carries enough signal to choose between them in most cases, so the plugin classifies intent first and only falls back to a freshness check when the request is genuinely ambiguous.

| Intent | Signal | Behavior |
|---|---|---|
| Reference | "my last", "my recent", "that diagnostic", an explicit `diagnostic.id`, or any follow-up in an existing conversation | Reuse the existing diagnostic. Never collect. |
| Collection | "collect", "get", or "run" a new diagnostic | Collect fresh. Never silently reuse. |
| Ambiguous | "how is my cluster looking today?" | Check the age of the most recent diagnostic. Reuse when within the freshness window. When older or absent, ask before collecting. |

The freshness window is configurable and defaults to 24 hours, which matches the daily cadence the ambiguous phrasing implies. Whichever branch is taken, the plugin reports the diagnostic it selected and why, so a reused diagnostic is never mistaken for a fresh one.

Collection is only ever automatic when the user asked for it. Explicit collection intent proceeds without a prompt, because the user already said what they wanted. An ambiguous request that finds no recent diagnostic stops and asks, because inferring collection from a question like "how is my cluster looking today?" would issue unrequested API calls against a live production cluster on the strength of a phrasing guess. The confirmation states the age of the most recent diagnostic, or that none was found in the window, and names the host that would be collected from, so the user can approve, redirect to the older diagnostic, or decline.

The age lookup is a metadata query, not analysis, and must not cost an LLM call. Confirmed against the deployment, `POST /s/{space}/api/agent_builder/tools/_execute` with `platform.core.execute_esql` returns `model_usage: null` — the query executes directly with no agent involvement:

```esql
FROM metrics-diagnostic-esdiag*
| WHERE event.ingested >= NOW() - <window>
| KEEP diagnostic.id, event.ingested, diagnostic.user
| SORT event.ingested DESC
| LIMIT 1
```

Two verified traps constrain this query. First, the `_execute` wrapper applies its own default time range of `now-24h`, so the window must be stated explicitly in the query rather than inherited, or the result silently depends on an undocumented default that happens to equal the intended window. Second, an unknown field name yields an empty result rather than an error — a query naming a field that does not exist returns no rows and reads exactly like "no recent diagnostic," which would trigger a spurious collection. Only `diagnostic.id`, `event.ingested`, and `diagnostic.user` are confirmed present.

An empty result means "no diagnostic within the window," which is the stale branch. It does not mean no diagnostics exist, and must not be reported that way.

`user_diagnostic_id_fetcher` is not used for this. It scopes to `diagnostic.user` matching the authenticated username and its `format_output` step emits only identifiers, discarding the `event.ingested` values the freshness decision needs. Scoping by user is still preferable where `diagnostic.user` is populated, since "my last diagnostic" means the user's own, but that field is set from `esdiag` metadata flags and is not guaranteed present.

### A missing saved job is an onboarding moment, not an error

The first time a user asks for a cluster review with no saved job configured, the plugin offers to help configure one rather than failing or silently falling back to an ad-hoc collect and process pair. The silent fallback is the tempting option and the wrong one: it works, so the user never learns they have no repeatable setup, and every subsequent review re-derives the same host, output, and metadata arguments from scratch.

Saved jobs have real prerequisites, and they cascade. A job needs a saved host carrying the `collect` role, an output target carrying the `send` role, and an unlocked keystore holding their credentials. A first-time user may have none of these. The guided flow therefore establishes what is missing in order — keystore, collect host, output host, then the job itself — and reports at each step what it is doing and why, rather than presenting one opaque configuration prompt.

The first run doubles as the configuration. `--save-job <NAME>` persists a job before executing it, so the initial review both produces a diagnostic and leaves behind the reusable job. There is no configure-then-run round trip and no wasted collection. Job naming follows the existing `{host}-{action}-{destination}` convention already used by the web UI's save form, so CLI-created and UI-created jobs remain consistent.

Declining the offer is a supported path. A user who wants a one-off answer gets an ad-hoc collect and process, and the offer is not repeated within that session. What the plugin must not do is treat the absence of a job as a hard failure, since the user asked a reasonable question and the tooling can satisfy it either way.

### First-pass intent validation

Three prompts were run against a local stack to check the classification behaves as specified. Whether a collection occurred was measured by counting collected archives before and after, rather than taken from the narration.

| Prompt | Classified | Collected? | Outcome |
|---|---|---|---|
| "Collect" | collection | yes, no confirmation | new diagnostic `…~39c2` |
| "Evaluate" | reference | no (2 archives before and after) | reused `…~39c2` |
| "What's going on in my cluster?" | ambiguous | no (age 1 min, inside the 24h window) | reused, age reported |

The stale branch was confirmed separately: the same lookup with a one-minute window returns `found: false`, which is the condition that triggers asking before collecting.

This is a first pass. Intent boundaries are a judgment surface and will need real user phrasings to tune; these three only establish that each branch is reachable and that collection happens in exactly one of them.

The first-job flow was exercised the same way, by running it for real against a cluster with no saved job. Prerequisites were established in the specified order — keystore access, then a collect-role host validated live by `host add`, then an output target — and `--save-job` persisted the job before execution in both the collect-only and process-form shapes, so no separate configuration pass was needed. The declined path, where the user refuses the offer and gets a one-off collection with no job persisted and no repeat prompt, is prompt behavior that was not exercised and is likewise deferred to real user feedback.

### Saved jobs come in two shapes, and only one leaves something to analyze

Verified against 0.16.4. `--save-job` records whichever invocation it was attached to:

| Saved from | Recorded | `job list` shows | Lands in the cluster? |
|---|---|---|---|
| `esdiag collect --save-job N <HOST> <DIR>` | `action: collect`, `output_dir` | `Processing: skipped` | no, archive on disk only |
| `esdiag process <HOST> <OUTPUT_HOST> --save-job N` | `action: process`, `output: known-host` | `Processing: standard` | yes |

A known-host input makes `process` collect, process, and send in one step, so the second form is the one the review flow needs. The first produces an archive and nothing to analyze.

Separately, **`esdiag job run` did not print the diagnostic identifier.** It reported only `job run complete`, unlike `esdiag process`, which prints `process complete … documents for <id>` plus the Kibana link. The identifier was therefore unrecoverable from the job path.

This is fixed in the CLI rather than worked around in the plugin, because it is not a plugin problem: a saved job conceals which commands ran, so a CLI user running `job run` had no way to reference what it produced either. `run_job` now returns a `JobRunOutcome` and the CLI reports the diagnostic identifier and Kibana link for a processing job, the archive path for a collect-only job, and the destination for an upload job. A collect-only summary deliberately omits any identifier, so it cannot be mistaken for one that landed data.

The plugin still keeps the freshness-lookup fallback, since it ships to users whose installed `esdiag` may predate this change.

### The keystore gate is a first-class outcome

`esdiag keystore status` returning `Keystore: locked` cannot be resolved non-interactively. The daily-driver command treats this as an expected terminal state that stops and asks the user, not as an error to retry or work around.

### Client binding and cluster provisioning are separate

Binding a workstation needs an `esdiag` binary, a Kibana API key, host and keystore configuration, and plugin settings. Provisioning a cluster needs a container runtime or an existing deployment, a trial or enterprise license, `esdiag setup`, and an LLM connector. These share no failure modes, and the common case — a support engineer pointed at a shared team cluster — involves only the former. Combining them would make the common case appear to require Docker.

## Risks / Trade-offs

- **Model availability is an environment prerequisite, not an ESDiag responsibility.** Delegated analysis requires the deployment to have a usable model, reached either by activating the Elastic Inference Service through Cloud Connect with an Elastic Cloud account, or by configuring a third-party LLM provider and connector. Neither is a reasonable automation target: they span account provisioning, billing, and third-party credentials. This change's obligation is therefore narrow and diagnostic only — detect that no usable model is configured, report it as a deployment prerequisite with a pointer to model setup guidance, and never present it as an ESDiag defect or attempt to configure it.
- **Prose output has no contract.** Without a schema, the response shape is whatever ADA's instructions produce. Presentation must tolerate variation and must not parse prose into decisions. Accepted as the cost of runtime-configurable agent selection.
- **Cluster spend is real and grows with conversation depth.** ~110k input tokens for one analysis, rising per follow-up. Local savings are not free, they are transferred. Documentation should say so plainly.
- **Analysis latency is ~60s.** SSE progress events mitigate the experience but do not shorten it. Requests must not be retried on timeout without first checking whether a conversation was created, or the cluster pays twice.
- **A second credential appears.** The Kibana API key sits outside the `esdiag` keystore, which is a deliberate scope decision but does mean two credential stores. The binding command should avoid persisting the key in plaintext where the keystore pattern already exists.
- **Bundled-skill drift.** If the operations skill is copied into the plugin rather than sourced from `.agents/skills/esdiag/`, the two will diverge. Packaging must make copying impossible or automatic.

## Open Questions

None outstanding. The remaining unknowns are verification items rather than design questions: the documented privilege set has not yet been exercised with a minimally scoped key, and the intent classification boundaries will need tuning against real phrasings once in use.
