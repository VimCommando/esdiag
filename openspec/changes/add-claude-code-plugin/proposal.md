## Why

ESDiag already publishes two halves of an agent-driven diagnostic workflow that never meet. The `.agents/skills/esdiag/` skill teaches an agent to drive the `esdiag` CLI, and `esdiag setup` installs the `agentic-diagnostic-assistant` (ADA) skill into a Kibana space where it is attached to the `elastic-ai-agent` default agent. A user in Claude Code can collect and process a diagnostic, but must then leave the terminal and open Kibana chat to have it analyzed.

Closing that gap in the obvious way — teaching Claude to analyze diagnostics itself — is the wrong trade. It duplicates ADA's knowledge outside the cluster where it will drift, and it spends the user's local model quota on work the cluster is already provisioned and separately billed to do. A measured end-to-end ADA analysis consumes ~109,600 input and ~3,600 output tokens on the cluster's inference connector while returning ~1,100 tokens of markdown, a ratio near 100:1. In organizations where per-user assistant quota is constrained but cluster inference is billed separately, that ratio is the entire point.

This change packages ESDiag as a Claude Code plugin that delegates analysis to the cluster's own agent over the Agent Builder chat API, so Claude orchestrates and presents while the cluster reasons and pays.

## What Changes

- Add a Claude Code plugin package that installs into a user's local Claude Code instance and bundles the existing `.agents/skills/esdiag/` operations skill as its single source of CLI guidance.
- Add a client-side analysis path that sends a verified `diagnostic.id` and the user's question to the cluster's Agent Builder agent via `POST /s/{space}/api/agent_builder/converse/async`, consuming the Server-Sent Events stream so the user sees real progress instead of a silent wait.
- Make the target agent configurable through plugin settings, defaulting to `elastic-ai-agent`, because the agent carrying the ADA skill is a per-cluster deployment decision and no single agent id is safe to hard-code.
- Make the inference endpoint optionally overridable per request so analysis spend can be routed to a designated billed endpoint.
- Relay the agent's markdown response, including its relative dashboard links, without re-analyzing the underlying metrics locally.
- Support follow-up drill-down by reusing the returned `conversation_id`, which keeps local cost near zero for subsequent turns.
- Add a client binding command that validates the Kibana endpoint, API key, configured agent id, and `esdiag` keystore state, and that explicitly does not provision a cluster.
- Add a daily-driver command that gates on keystore state, runs a saved job or an explicit collect/process pair to obtain a `diagnostic.id`, then requests analysis.
- Treat cluster provisioning as a separate concern referenced by the operations skill rather than implemented here.

## Capabilities

### New Capabilities
- `claude-code-plugin`: Distribution, installation, configuration resolution, and client-binding behavior for the ESDiag Claude Code plugin, including its separation from cluster provisioning.
- `agent-builder-analysis`: Delegated diagnostic analysis against a Kibana Agent Builder agent over the chat API, covering agent selection, progress reporting, diagnostic identifier handoff, conversation continuation, and failure handling.

### Modified Capabilities
- None.

## Impact

- **Target Elastic products:** Kibana Agent Builder (chat API, agents, skills) in the ESDiag-configured space, and the Elasticsearch diagnostics cluster holding `metrics-*-esdiag*` data. Elasticsearch, Logstash, and Kibana diagnostics remain the analyzed subject matter, unchanged.
- **Rust CLI:** No behavioral changes. The plugin composes existing `esdiag keystore`, `esdiag job`, `esdiag collect`, and `esdiag process` commands. No new subcommand is added; an `esdiag`-native conversation client is explicitly out of scope.
- **Web UI:** No changes.
- **Core processing logic:** No changes.
- **Kibana assets:** No new workflow or tool assets are required. The change depends only on assets `esdiag setup` already installs.
- **Deployment prerequisites:** Requires the deployment to have a usable model, via the Elastic Inference Service through Cloud Connect or a user-configured LLM provider and connector. Configuring model access is out of scope; the plugin detects its absence and reports it as a prerequisite.
- **New repository surface:** A plugin package directory and its packaging or release step. The bundled operations skill must remain sourced from `.agents/skills/esdiag/` rather than copied, so the two consumers cannot drift.
- **Credentials:** Introduces a client-held Kibana API key distinct from the `esdiag` keystore, used only for Agent Builder requests.
- **Cost attribution:** Moves diagnostic analysis token spend from the user's local model to the cluster's inference connector. Cluster input tokens grow per conversation turn as history replays; local cost stays approximately flat.
- **Documentation:** Adds client-binding guidance and makes deployment prerequisites explicit, including that a usable model must already be available to the deployment.
- **Tests:** Adds plugin manifest and configuration-resolution coverage, plus non-networked coverage of the conversation request construction and event handling.
