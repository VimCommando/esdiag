<!-- Generated from .agents/skills/esdiag/ by bin/sync-plugin-skill.sh. Do not edit. -->

# ESDiag Environment Variables

Use these variables when configuring `esdiag` without saved hosts, or to supply credentials and settings non-interactively.

## Path Overrides

| Variable | Default | Purpose |
|---|---|---|
| `ESDIAG_HOME` | `~/.esdiag` | Base directory for all esdiag config and data files |
| `ESDIAG_HOSTS` | `$ESDIAG_HOME/hosts.yml` | Override path to the saved-hosts file |
| `ESDIAG_KEYSTORE` | `$ESDIAG_HOME/secrets.yml` | Override path to the encrypted keystore file |

## Output Target (`process`, `setup`, `serve`)

| Variable | Purpose |
|---|---|
| `ESDIAG_OUTPUT_URL` | Elasticsearch output URL |
| `ESDIAG_OUTPUT_APIKEY` | API key for output cluster |
| `ESDIAG_OUTPUT_USERNAME` | Basic auth username for output cluster |
| `ESDIAG_OUTPUT_PASSWORD` | Basic auth password for output cluster |
| `ESDIAG_KIBANA_URL` | Kibana URL — required for Kibana asset setup when `[HOST]` is omitted from `setup`, and to generate dashboard links in `serve` |
| `ESDIAG_KIBANA_SPACE` | Kibana space ID to use when constructing dashboard links. Defaults to `esdiag` when unset; set it to an empty value to omit the `/s/<space>` suffix |

## Agent Builder Analysis (Claude Code plugin)

Client settings for delegating diagnostic analysis to a deployment's Agent Builder agent. These configure the client only; they do not provision anything.

| Variable | Default | Purpose |
|---|---|---|
| `ESDIAG_KIBANA_URL` | — | Kibana base URL of the deployment holding the diagnostics. A trailing `/s/<space>` is recognized and not duplicated |
| `ESDIAG_KIBANA_SPACE` | `esdiag` | Kibana space holding the ESDiag assets; set to an empty value for the default space |
| `ESDIAG_KIBANA_APIKEY` | — | Kibana API key for Agent Builder requests |
| `ESDIAG_KIBANA_APIKEY_FILE` | — | Path to a file containing the API key, so the key need not be stored in plugin settings. Used when `ESDIAG_KIBANA_APIKEY` is unset |
| `ESDIAG_AGENT_ID` | `elastic-ai-agent` | Agent to send analysis requests to. Must be the agent carrying the diagnostic skill in the configured space |
| `ESDIAG_INFERENCE_ID` | — | Inference endpoint for model routing. When unset, the agent uses its own configured model |
| `ESDIAG_JOB` | — | Saved job name used to collect a diagnostic |
| `ESDIAG_DIAGNOSTIC_MAX_AGE` | `24h` | Freshness window. An ambiguous request reuses a diagnostic newer than this, and asks before collecting when none is |

The API key needs `feature_agentBuilder.read` and `feature_actions.read` on the configured space, cluster `monitor_inference` when the connector uses the Elasticsearch Inference API, and `read` plus `view_index_metadata` on `metrics-*-esdiag*` and `settings-*-esdiag*`. Agent Builder tools query Elasticsearch as the calling identity, so a key without the index privileges authenticates but returns empty analysis.

## Keystore

| Variable | Purpose |
|---|---|
| `ESDIAG_KEYSTORE_PASSWORD` | Keystore password for non-interactive operations; suppresses the password prompt in `keystore add/update/remove/unlock/password` |

## Report / Identity

| Variable | Purpose |
|---|---|
| `ESDIAG_USER` | Default user email attached to report metadata; overridden by `--user` flag |

## Server (`serve`)

| Variable | Default | Purpose |
|---|---|---|
| `ESDIAG_MODE` | `user` | Runtime mode: `user` (single-user) or `service` (multi-user) |
| `ESDIAG_PORT` | `2501` | Port the upload service listens on; overridden by `--port` flag |

## Performance Tuning

| Variable | Default | Purpose |
|---|---|---|
| `ESDIAG_ES_BULK_SIZE` | `10000` | Maximum number of documents per Elasticsearch bulk request |
| `ESDIAG_ES_BULK_BYTES` | `52428800` | Approximate maximum serialized bulk request size in bytes for Elasticsearch output; set to `0` to disable byte-based splitting |
| `ESDIAG_ES_WORKERS` | `4` | Number of parallel worker threads for export |
| `ESDIAG_OUTPUT_TASK_LIMIT` | — | Max concurrent tasks when sending to Elasticsearch |
| `ESDIAG_REQUEST_TIMEOUT_MS` | — | HTTP request timeout in milliseconds |
| `ESDIAG_EXPORT_RETRY_MAX` | — | Maximum number of export retry attempts |
| `ESDIAG_EXPORT_RETRY_INITIAL_MS` | — | Initial retry backoff in milliseconds |
| `ESDIAG_EXPORT_RETRY_MAX_MS` | — | Maximum retry backoff ceiling in milliseconds |

## Logging

| Variable | Default | Purpose |
|---|---|---|
| `LOG_LEVEL` | `info` | Log verbosity: `error`, `warn`, `info`, `debug`, `trace` |
