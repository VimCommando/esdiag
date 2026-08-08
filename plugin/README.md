# ESDiag Claude Code Plugin

Collect, process, and analyze Elastic Stack diagnostics from Claude Code.

The analysis itself runs on **your** Elastic deployment, through the diagnostic skill that `esdiag setup` installs into Agent Builder. Claude orchestrates and presents; the cluster reasons. Two consequences follow:

- The analysis always reflects the assets actually installed on that cluster, so it cannot drift from what Kibana would tell you.
- The token cost lands on the deployment's inference connector rather than your Claude quota. A measured analysis consumes roughly 110,000 input tokens on the cluster and returns about 1,100 tokens of markdown. Follow-up questions cost almost nothing locally because the conversation lives on the cluster.

That trade is a transfer, not a saving: cluster input tokens grow with each turn as the conversation replays.

## Install

```sh
/plugin marketplace add elastic/esdiag
/plugin install esdiag
```

## Two setups, not one

These are separate problems with separate prerequisites and separate failure modes. Most users only need the second.

### Provisioning a cluster (once per deployment, often already done)

An Elastic deployment with the ESDiag assets installed (`esdiag setup`), a suitable subscription, and **model access**.

Model access is a deployment prerequisite that no `esdiag` command provisions. Satisfy it by activating the Elastic Inference Service through Cloud Connect with an Elastic Cloud account, or by configuring your own LLM provider and connector.

### Binding this machine (once per user, per machine)

Needs the `esdiag` CLI, a Kibana API key, and saved hosts. No container runtime.

```sh
export ESDIAG_KIBANA_URL=https://your-deployment.kb.example.com
export ESDIAG_KIBANA_APIKEY_FILE=~/.config/apikey/esdiag
/esdiag:connect
```

`/esdiag:connect` is read-only. It never creates, starts, or reconfigures a deployment.

## Commands

| Command | Purpose |
|---|---|
| `/esdiag:connect` | Verify this machine can reach and use a deployment |
| `/esdiag:check` | Review cluster health by analyzing a diagnostic |
| `/esdiag:first-job` | Configure a reusable saved job |

Or just ask: *"How is my cluster looking today?"*

## Settings

See `skills/esdiag/references/env-vars.md` for the full table. The essentials:

| Variable | Default | Purpose |
|---|---|---|
| `ESDIAG_KIBANA_URL` | — | Kibana base URL (required) |
| `ESDIAG_KIBANA_APIKEY_FILE` | — | Path to a file holding the API key |
| `ESDIAG_KIBANA_SPACE` | `esdiag` | Space holding the ESDiag assets |
| `ESDIAG_AGENT_ID` | `elastic-ai-agent` | Agent carrying the diagnostic skill |
| `ESDIAG_INFERENCE_ID` | — | Route analysis to a specific model endpoint |
| `ESDIAG_JOB` | — | Saved job used to collect |
| `ESDIAG_DIAGNOSTIC_MAX_AGE` | `24h` | Freshness window |

`ESDIAG_AGENT_ID` must name the agent that actually carries the diagnostic skill in your space. The default matches what `esdiag setup` configures, but a deployment can attach it elsewhere — and a wrong agent produces a plausible, worse answer rather than an error, so `/esdiag:connect` validates it.

### API key privileges

| Level | Privilege |
|---|---|
| Kibana, scoped to your space | `feature_agentBuilder.read`, `feature_actions.read` |
| Elasticsearch cluster | `monitor_inference` (only when the connector uses the Elasticsearch Inference API) |
| Elasticsearch indices | `read`, `view_index_metadata` on `metrics-*-esdiag*` and `settings-*-esdiag*` |

The index privileges are not optional. Agent Builder tools query Elasticsearch **as the calling identity**, so a key with chat access but no data access authenticates successfully and returns an empty analysis rather than an authorization error.

## When a new diagnostic gets collected

Collection issues API calls against a live production cluster, so it follows the request rather than a default:

- "my last diagnostic", an explicit id, or any follow-up → reuses what exists
- "collect a new diagnostic" → collects, no confirmation needed
- "how is my cluster looking today?" → reuses a diagnostic newer than `ESDIAG_DIAGNOSTIC_MAX_AGE`, and **asks first** when there isn't one

## Development

The bundled skill under `skills/` is generated from `.agents/skills/esdiag/`:

```sh
./bin/sync-plugin-skill.sh          # regenerate
./bin/sync-plugin-skill.sh --check  # fail on drift
./tests/plugin.sh                   # non-networked tests
```

Edit `.agents/skills/esdiag/`, never `plugin/skills/`.
