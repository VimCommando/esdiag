---
name: esdiag
description: Collect, process, or analyze Elasticsearch, Kibana, and Logstash diagnostics with `esdiag`. Use for binding to an ESDiag deployment, reviewing cluster health through a persisted Kibana Agent Builder conversation, collecting live API diagnostics, processing support bundles or Elastic upload links, sending results to an output cluster, managing saved hosts and encrypted credentials, running saved jobs, or hosting the web UI.
---

# ESDiag

Use this skill to choose and run the right ESDiag workflow safely.

## Portable Resource Paths

Resolve every `scripts/...` and `references/...` path relative to the directory containing this `SKILL.md`, never relative to the user's working directory. Before executing a helper, substitute its absolute skill-directory path. The helpers and references are part of this skill and must travel with it.

Prefer live help output over memory when behavior is unclear:

```sh
esdiag --help
esdiag <command> --help
```

## Command Routing

- Connection management: `esdiag host`
- Credentials and unlock state: `esdiag keystore`
- Asset setup: `esdiag setup`
- Process diagnostics into output docs: `esdiag process`
- Collect fresh API diagnostics: `esdiag collect`
- Saved reusable jobs: `esdiag job`, or `--save-job <NAME>` on compatible `collect`/`process`
- Web/API intake: `esdiag serve`

## Required Checks

Run `esdiag keystore status` before authenticated collection, processing from saved hosts, saved jobs, or host/keystore changes.

If locked, stop and ask the user to unlock with `esdiag keystore unlock` or through the web UI.

```
esdiag keystore status
```

- `result: keystore_status` with `unlock_active: true` — proceed normally.
- `result: keystore_status` with `unlock_active: false` — stop and tell the user to unlock it via the web UI or with `esdiag keystore unlock` before continuing.

## Bind to an Analysis Deployment

Run the read-only, conversation-free binding check:

```sh
scripts/connect.sh
```

Required configuration:

- `ESDIAG_KIBANA_URL`
- `ESDIAG_ELASTICSEARCH_URL` or existing `ESDIAG_OUTPUT_URL`
- `ESDIAG_KIBANA_APIKEY` or `ESDIAG_KIBANA_APIKEY_FILE`

Report failures using these boundaries:

- **Client configuration**: invalid URLs, rejected API key, missing configured agent, or unreadable ESDiag data streams.
- **Cluster provisioning**: Agent Builder or the ESDiag assets are absent.
- **Deployment model prerequisite**: reported by the first real analysis if the agent has no usable model. The binding check deliberately avoids creating a throwaway conversation to probe it.

A missing `esdiag` binary, locked keystore, or absent saved hosts affects collection and processing only. Existing diagnostics can still be analyzed.

## Review Cluster Health

Have the deployment's diagnostic agent perform the analysis. Do not reproduce its metrics or thresholds locally. Every real analysis must use `scripts/analyze.sh` so it is saved in Kibana Agent Builder history; direct Elasticsearch queries are metadata lookups only.

### Select a diagnostic

Classify the request before collecting anything:

- **Reference**: an explicit ID, “my last diagnostic,” “that diagnostic,” or a follow-up. Reuse; never collect.
- **Collection**: an explicit request to collect, get, or run a new diagnostic. Collect without another confirmation.
- **Ambiguous**: a general question such as “How is my cluster looking today?” Check freshness first.

For reference requests without an explicit ID, and for ambiguous requests, run:

```sh
scripts/latest-diagnostic.sh
```

Interpret its JSON:

- `found:true, fresh:true`: reuse it and report its age.
- `found:true, fresh:false`: for an ambiguous request, state its age and the collection host, then ask before collecting. For reference intent, reuse it despite its age.
- `found:false`: no diagnostic was found. Ask before collecting unless collection was explicit.
- Exit `2`: freshness is unknown. Report the query failure and do not infer collection.

If collection is declined, offer the most recent existing diagnostic and do not ask again in this session.

### Collect when required

Run `esdiag keystore status` before using saved hosts or jobs. If locked, stop and ask the user to run `esdiag keystore unlock`.

When `ESDIAG_JOB` is configured:

```sh
esdiag job run "$ESDIAG_JOB"
```

Read `process.diagnostic.id` and `process.diagnostic.kibana_url` from the
returned YAML outcome. A `job_completed` result can also contain `save` and
`send`; do not infer success from prose.

When no job is configured, offer to create one. Establish prerequisites in this order:

1. An unlocked keystore.
2. A saved host with the `collect` role.
3. A saved output host with the `send` role.

Create the reusable process job as part of its first run:

```sh
esdiag process <COLLECT_HOST> <OUTPUT_HOST> --save-job <NAME>
```

Read `diagnostic.id` and `diagnostic.kibana_url` from the returned YAML
outcome. Request `--format json` only when the caller already uses JSON
deserialization.

Use the existing `{host}-{action}-{destination}` naming convention and let the user override it. Do not use `collect --save-job` for analysis: that creates a local archive but sends no diagnostic to the analysis cluster.

If the user declines job setup, perform a one-off `esdiag process <COLLECT_HOST> <OUTPUT_HOST>` and do not persist a job or repeat the offer this session.

If an older `esdiag` does not report an identifier, resolve it with `scripts/latest-diagnostic.sh --window 15m` and confirm it is newer than the job start. Never ask the cluster agent to guess the identifier.

### Analyze in Kibana Agent Builder

```sh
scripts/analyze.sh \
  --diagnostic "<diagnostic.id>" \
  --question "<the user's question>"
```

Relay streamed progress and then the returned markdown. Follow-ups automatically reuse the conversation associated with this deployment, space, agent, and diagnostic. Tell the user that the thread is available in Kibana Agent Builder history, and present any `Kibana Link` from processing as a clickable link.

Handle exits as follows:

- `0`: present the analysis, diagnostic ID, age, and whether it was reused or newly collected.
- `2`: report the attributed configuration, authorization, deployment-model, or connectivity failure.
- `3`: report the retained Kibana conversation ID. Do not re-run without direction because the original work may already be billed.

Treat the response as unstructured markdown. You may summarize an explicit not-found statement for the user, but do not parse prose to trigger retries, remediation, or other automated action.

## Detailed Workflow

- Use `esdiag host add <NAME> <APP> <URL>` to create and save a new host definition in `~/.esdiag/hosts.yml`.
- Use `esdiag host update <NAME>` with mutable flags to modify an existing saved host in place.
- Use `esdiag host remove <NAME>` to delete an existing saved host from `hosts.yml`.
- Use `esdiag host list` to return a `hosts_listed` YAML outcome with typed `hosts` entries.
- Use `esdiag host auth <NAME>` to test a saved host's persisted authentication and connection settings without modifying it.
- Use `--apikey` for API key auth or `--user`/`--password` for basic auth.
- `--user` is the primary basic-auth flag (with `--username` available as an alias).
- Use `--secret <secret_id>` to reference credentials stored in the encrypted keystore.
- Use `--secret`, `--apikey`, `--user`/`--password`, and `--roles` with `host add` or `host update` to set authentication and workflow roles.
- Use `--roles collect,send,view` to assign host workflow roles.
- Use `--accept-invalid-certs true` to enable invalid-certificate acceptance for a saved host, and `--accept-invalid-certs false` to remove it. If the flag is omitted during `host update`, the saved certificate setting is preserved.
- `host add` fails if the host already exists; `host update`, `host remove`, and `host auth` fail if the host does not exist.
- `host update` always re-tests the live connection before persistence.
- Use environment variables (optionally by sourcing a `.env` file in the shell) when the user does not want a saved host.

## Manage Encrypted Secrets

- Use `esdiag keystore add <secret_id>` to create encrypted credentials.
- Use `esdiag keystore update <secret_id>` to change an existing encrypted secret.
  - Basic auth: `--user <name> --password <value>` or omit the password value in an interactive shell to get a masked prompt.
  - API key auth: `--apikey <value>` or omit the value in an interactive shell to get a masked prompt.
- Use `esdiag keystore remove <secret_id>` to remove encrypted credentials (optionally scoped by auth type flags).
- Use `esdiag keystore unlock [--ttl 24h|7d|90m]` to cache keystore access for later CLI runs, `status` to inspect it, and `lock` to clear it.
- Use `esdiag keystore password` to rotate the keystore password.
- Use `esdiag keystore migrate` to move legacy plaintext host credentials from `hosts.yml` into keystore entries keyed by host name.
- Set `ESDIAG_KEYSTORE_PASSWORD` for non-interactive keystore operations.
- In interactive shells, `keystore add/update/remove/unlock/password` can prompt for the keystore password when `ESDIAG_KEYSTORE_PASSWORD` is unset.

## Setup Output Cluster

- Run `esdiag setup [HOST]` before first ingestion into a cluster.
- If `[HOST]` is omitted, rely on:
  - `ESDIAG_OUTPUT_URL`
  - `ESDIAG_OUTPUT_APIKEY`
  - `ESDIAG_OUTPUT_USERNAME`
  - `ESDIAG_OUTPUT_PASSWORD`
  - `ESDIAG_KIBANA_URL` (required for Kibana asset setup in host-omitted mode)
- In host-omitted mode, `setup` attempts both Elasticsearch and Kibana asset setup.

## Process Diagnostics

- Use `esdiag process [OPTIONS] <INPUT> [OUTPUT]`.
- Accept these input patterns:
  - Support diagnostics `.zip` archive
  - Unpacked diagnostic directory
  - Known host name from `~/.esdiag/hosts.yml`
  - Elastic Upload URL (`https://token:...@upload.elastic.co/d/...`)
- Resolve `[OUTPUT]` using these rules:
  - If `[OUTPUT]` is `-`, write to stdout.
  - Otherwise, if it matches a saved host name, use that host.
  - Otherwise, treat it as a filesystem target (file or directory).
  - If `[OUTPUT]` is omitted entirely, fall back to `ESDIAG_OUTPUT_*` environment variables (Elasticsearch output target).
  - Do not treat raw `http(s)` output strings as valid output targets unless they are saved and resolved as known hosts.
- Attach report metadata when provided by user:
  - `--account`
  - `--case`
  - `--opportunity`
  - `--user`
- Use `--sources` to override endpoint source definitions when testing new API mappings or reproducing source-selection behavior.
- After a successful `esdiag process`, read `diagnostic.kibana_url` from the YAML outcome and present it as a clickable markdown link. Do not manually look up Kibana hosts.
- Use `--save-job <NAME>` on compatible `process` invocations to persist the job before execution. The input must be a saved known host, and `[OUTPUT]` must be explicit.

## Collect Diagnostics

- Use `esdiag collect [OPTIONS] <HOST> <OUTPUT>` when the user needs fresh API diagnostics.
- Ensure `<OUTPUT>` already exists; command creates a diagnostic subdirectory within it.
- Use `--type` to control collection level, in ascending breadth: `minimal` (cluster + nodes only) → `light` (light-tagged APIs) → `standard` (fixed ~20 API set) → `support` (every available API in the sources definition).
- Use `--include` and `--exclude` to explicitly control which APIs are collected.
- Use metadata options (`--account`, `--case`, `--opportunity`, `--user`) when collected artifacts should carry report context.
- Use `--sources` when the collection endpoints should come from a non-default `sources.yml`.
- Use `--save-job <NAME>` on compatible `collect` invocations to persist the job before execution. `<HOST>` must be a saved host with the `collect` role, and `<OUTPUT>` must be an existing directory.
- For repeated captures, use `bin/min-diag.sh watch` and process each generated directory with `esdiag process`.

## Saved Jobs

Saved jobs persist named diagnostic configurations to `~/.esdiag/jobs.yml` so they can be re-run without reconfiguration. Jobs are saved from the `/jobs` web UI or managed via the CLI. Requires the `keystore` feature (enabled by default).

- Use `esdiag job list` to return a `jobs_listed` YAML outcome with typed phase-composed `jobs` entries. An empty store is `jobs: []`.
- Use `esdiag job run <NAME>` to execute a saved job end-to-end (collect → process → send).
  - Resolves the collection host from `hosts.yml` and credentials from the keystore.
  - Fails with a clear error if the job name is unknown, the jobs file is missing, or the referenced host no longer exists.
- Use `esdiag job delete <NAME>` to remove a saved job from `jobs.yml`.
  - Fails with a clear error if the job name is not found.
- In the web UI (`/jobs` page, user mode only), the left panel lists saved jobs with Load and Delete actions. The Save form derives a default name from the workflow (`{host}-{action}-{destination}`) and disables saving for upload-file and service-link sources since those are not repeatable.

## Run Upload Service

- Use `esdiag serve [OPTIONS] [OUTPUT]` to host upload and API endpoints.
- Default port is `2501`; override with `--port`.
- Pass `--kibana <URL>` (or set `ESDIAG_KIBANA_URL`) to show direct links in UI flows.
- Use output resolution rules from `process`.

## Troubleshooting Rules
- If command behavior looks inconsistent with docs, trust live help output first.
- If auth fails, re-check saved host/app/url/auth mode and whether cert validation is required.
- If a saved-host update fails, remember that `esdiag host update <NAME>` re-validates the merged host definition live before saving it.
- If a host should be removed entirely, prefer `esdiag host remove <NAME>` instead of hand-editing `hosts.yml`.
- If output is not where expected, verify `[OUTPUT]` parsing and known-host name collisions with filenames.
- If setup or ingest fails after version changes, rerun `esdiag setup` before retrying `process`.

## References

- Use `references/cli.md` for command syntax, option details, and output resolution rules.
- Use `references/env-vars.md` for all `ESDIAG_*` environment variables and their purpose.
