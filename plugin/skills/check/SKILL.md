---
name: check
description: Review Elastic cluster health by selecting or collecting an ESDiag diagnostic and analyzing it in a persisted Kibana Agent Builder conversation. Use for cluster-health questions, diagnostic analysis, and follow-up questions about an earlier ESDiag review.
---

# Check an Elastic cluster

Have the deployment's diagnostic agent perform the analysis. Do not reproduce its metrics or thresholds locally. Every real analysis must use `analyze.sh` so it is saved in Kibana Agent Builder history; direct Elasticsearch queries are metadata lookups only.

## Select a diagnostic

Classify the request before collecting anything:

- **Reference**: an explicit ID, “my last diagnostic,” “that diagnostic,” or a follow-up. Reuse; never collect.
- **Collection**: an explicit request to collect, get, or run a new diagnostic. Collect without another confirmation.
- **Ambiguous**: a general question such as “How is my cluster looking today?” Check freshness first.

For reference requests without an explicit ID, and for ambiguous requests, run:

```sh
"${CLAUDE_PLUGIN_ROOT}/scripts/latest-diagnostic.sh"
```

Interpret its JSON:

- `found:true, fresh:true`: reuse it and report its age.
- `found:true, fresh:false`: for an ambiguous request, state its age and the collection host, then ask before collecting. For reference intent, reuse it despite its age.
- `found:false`: no diagnostic was found. Ask before collecting unless collection was explicit.
- Exit `2`: freshness is unknown. Report the query failure and do not infer collection.

If collection is declined, offer the most recent existing diagnostic and do not ask again in this session.

## Collect when required

Run `esdiag keystore status` before using saved hosts or jobs. If locked, stop and ask the user to run `esdiag keystore unlock`.

When `ESDIAG_JOB` is configured:

```sh
esdiag job run "$ESDIAG_JOB" 2>&1 | "${CLAUDE_PLUGIN_ROOT}/scripts/extract-diagnostic.sh"
```

When no job is configured, offer to create one. Establish prerequisites in this order:

1. An unlocked keystore.
2. A saved host with the `collect` role.
3. A saved output host with the `send` role.

Create the reusable process job as part of its first run:

```sh
esdiag process <COLLECT_HOST> <OUTPUT_HOST> --save-job <NAME> 2>&1 \
  | "${CLAUDE_PLUGIN_ROOT}/scripts/extract-diagnostic.sh"
```

Use the existing `{host}-{action}-{destination}` naming convention and let the user override it. Do not use `collect --save-job` for analysis: that creates a local archive but sends no diagnostic to the analysis cluster.

If the user declines job setup, perform a one-off `esdiag process <COLLECT_HOST> <OUTPUT_HOST>` and do not persist a job or repeat the offer this session.

If an older `esdiag` does not report an identifier, resolve it with `latest-diagnostic.sh --window 15m` and confirm it is newer than the job start. Never ask the cluster agent to guess the identifier.

## Analyze in Kibana Agent Builder

```sh
"${CLAUDE_PLUGIN_ROOT}/scripts/analyze.sh" \
  --diagnostic "<diagnostic.id>" \
  --question "<the user's question>"
```

Relay streamed progress and then the returned markdown. Follow-ups automatically reuse the conversation associated with this deployment, space, agent, and diagnostic. Tell the user that the thread is available in Kibana Agent Builder history, and present any `Kibana Link` from processing as a clickable link.

Exit handling:

- `0`: present the analysis, diagnostic ID, age, and whether it was reused or newly collected.
- `2`: report the attributed configuration, authorization, deployment-model, or connectivity failure.
- `3`: report the retained Kibana conversation ID. Do not re-run without direction because the original work may already be billed.

Treat the response as unstructured markdown. You may summarize an explicit not-found statement for the user, but do not parse prose to trigger retries, remediation, or other automated action.
