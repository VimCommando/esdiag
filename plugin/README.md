# ESDiag Claude Code Plugin

Collect, process, and analyze Elastic Stack diagnostics from Claude Code.

The analysis itself runs on **your** Elastic deployment, through the diagnostic skill that `esdiag setup` installs into Agent Builder. Claude orchestrates and presents; the cluster reasons. Every analysis and follow-up is saved as a normal Kibana Agent Builder conversation, so you can continue the same thread from Kibana later.

- The analysis always reflects the assets actually installed on that cluster, so it cannot drift from what Kibana would tell you.
- The token cost lands on the deployment's inference connector rather than your Claude quota. A measured analysis consumes roughly 110,000 input tokens on the cluster and returns about 1,100 tokens of markdown. Follow-up questions cost almost nothing locally because the conversation lives on the cluster.

That trade is a transfer, not a saving: cluster input tokens grow with each turn as the conversation replays.

## Install

```sh
/plugin marketplace add elastic/esdiag
/plugin install esdiag@esdiag
```

## Two setups, not one

These are separate problems with separate prerequisites and separate failure modes. Most users only need the second.

### Provisioning a cluster (once per deployment, often already done)

An Elastic deployment with the ESDiag assets installed (`esdiag setup`), a suitable subscription, and **model access**.

Model access is a deployment prerequisite that no `esdiag` command provisions. Satisfy it by activating the Elastic Inference Service through Cloud Connect with an Elastic Cloud account, or by configuring your own LLM provider and connector.

### Binding this machine (once per user, per machine)

Needs Kibana and Elasticsearch URLs plus an API key. The `esdiag` CLI and saved hosts are needed only when this machine will collect or process diagnostics. No container runtime.

```sh
export ESDIAG_KIBANA_URL=https://your-deployment.kb.example.com
export ESDIAG_ELASTICSEARCH_URL=https://your-deployment.es.example.com
export ESDIAG_KIBANA_APIKEY_FILE=~/.config/apikey/esdiag
/esdiag:connect
```

`/esdiag:connect` is read-only. It never creates, starts, or reconfigures a deployment.

## Commands

| Command | Purpose |
|---|---|
| `/esdiag:connect` | Verify this machine can reach and use a deployment |
| `/esdiag:check` | Review cluster health by analyzing a diagnostic |

Or just ask: *"How is my cluster looking today?"*

## Settings

The plugin-specific settings are:

| Variable | Default | Purpose |
|---|---|---|
| `ESDIAG_KIBANA_URL` | — | Kibana base URL (required) |
| `ESDIAG_ELASTICSEARCH_URL` | `ESDIAG_OUTPUT_URL` | Elasticsearch URL used for conversation-free metadata queries |
| `ESDIAG_KIBANA_APIKEY` | — | API key value (prefer the file setting below when practical) |
| `ESDIAG_KIBANA_APIKEY_FILE` | — | Path to a file holding the API key |
| `ESDIAG_KIBANA_SPACE` | `esdiag` | Space holding the ESDiag assets |
| `ESDIAG_AGENT_ID` | `elastic-ai-agent` | Agent carrying the diagnostic skill |
| `ESDIAG_INFERENCE_ID` | — | Route analysis to a specific model endpoint |
| `ESDIAG_JOB` | — | Saved job used to collect |
| `ESDIAG_DIAGNOSTIC_MAX_AGE` | `24h` | Freshness window |

The generated `skills/esdiag/references/env-vars.md` documents the `esdiag` CLI's own environment variables separately.

`ESDIAG_AGENT_ID` must name the agent that actually carries the diagnostic skill in your space. The default matches what `esdiag setup` configures, but a deployment can attach it elsewhere — and a wrong agent produces a plausible, worse answer rather than an error, so `/esdiag:connect` validates it.

### API key privileges

| Level | Privilege |
|---|---|
| Kibana, scoped to your space | `feature_agentBuilder.read`, `feature_actions.read` |
| Elasticsearch cluster | `monitor_inference` (only when the connector uses the Elasticsearch Inference API) |
| Elasticsearch indices | `read`, `view_index_metadata` on `metrics-*-esdiag*` and `settings-*-esdiag*` |

The same API key is sent to Kibana for Agent Builder and directly to Elasticsearch for metadata lookups and access validation. The index privileges are not optional: Agent Builder tools query Elasticsearch **as the calling identity**, so a key with chat access but no data access can authenticate successfully yet return a degraded analysis.

Freshness and access checks use Elasticsearch `POST /_query` directly. They never create Agent Builder conversations or consume inference tokens. Actual analysis always uses Agent Builder, and its returned conversation ID is retained for follow-ups and Kibana history.

## When a new diagnostic gets collected

Collection issues API calls against a live production cluster, so it follows the request rather than a default:

- "my last diagnostic", an explicit id, or any follow-up → reuses what exists
- "collect a new diagnostic" → collects, no confirmation needed
- "how is my cluster looking today?" → reuses a diagnostic newer than `ESDIAG_DIAGNOSTIC_MAX_AGE`, and **asks first** when the latest is older or absent

## Development

The bundled skill under `skills/` is generated from `.agents/skills/esdiag/`:

```sh
./bin/sync-plugin-skill.sh          # regenerate
./bin/sync-plugin-skill.sh --check  # fail on drift
./tests/plugin.sh                   # non-networked tests
```

Edit `.agents/skills/esdiag/`, never the generated `plugin/skills/esdiag/`. The plugin-specific `check` and `connect` skills are maintained directly under `plugin/skills/`.
