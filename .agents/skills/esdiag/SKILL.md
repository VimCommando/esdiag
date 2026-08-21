---
name: esdiag
description: Collect, process, or analyze Elasticsearch, Kibana, and Logstash diagnostics with `esdiag`. Use for local setup, diagnostic collection and processing, and a finite Kibana Agent Builder question.
---

# ESDiag

Use `esdiag` for diagnostic work. When it is unavailable, read
`references/onboarding.md` before suggesting setup.

## First-time setup

Read `references/onboarding.md` when `esdiag` is missing or a user needs
first-run setup. It covers native installation, the script-first container
fallback, and the boundary around `esdiag init`.

Keep credentials in the interactive wizard. Do not write ESDiag state files in
an agent conversation.

## Check local prerequisites

Before collecting, processing from saved hosts, running saved jobs, or changing host/keystore state, run:

```sh
esdiag keystore status
```

If `unlock_active` is false, stop and ask the user to unlock the keystore locally with `esdiag keystore unlock`.

`esdiag agent ask` resolves Kibana and authentication through the same configured output deployment as processed diagnostics. If it reports missing configuration or a viewer problem, direct the user to run `esdiag init`; do not introduce a second Kibana URL, API key, or inference configuration.

## Collect and process diagnostics

Classify the request:

- An explicit diagnostic ID, “my last diagnostic,” or a follow-up refers to existing information. Ask the configured Agent Builder agent; do not collect by default.
- An explicit request to collect runs a saved job or a configured collection workflow.
- An ambiguous health question should reuse existing diagnostics unless the user explicitly asks for collection.

For a repeatable configured workflow:

```sh
esdiag job run <NAME>
```

Read `process.diagnostic.id` and `process.diagnostic.kibana_url` from the typed YAML outcome. When processing a known source directly, use:

```sh
esdiag process <INPUT> [OUTPUT]
```

When the caller asks about the diagnostic being processed, ask Agent Builder in the same command:

```sh
esdiag process <INPUT> [OUTPUT] --ask "<question>"
```

This starts a new conversation with the exact new diagnostic ID in context and returns an `agent_response` outcome. It requires a Kibana-enabled Elasticsearch output deployment and cannot be combined with output `-`.

Use `--format json` only when the caller already parses JSON. Prefer saved jobs for repeated collection and processing. See `references/cli.md` for command behavior.

## Ask Kibana Agent Builder

Submit one opaque prompt to the diagnostic agent:

```sh
esdiag agent ask "Analyze diagnostic <diagnostic-id>: <question>"
```

The finite `agent_response` outcome contains `message`, `conversation_id`, and `kibana_url`. Relay the answer as returned, and present the Kibana link as the durable conversation handoff.

For a follow-up, retain the returned ID in this conversation and pass it explicitly:

```sh
esdiag agent ask --conversation <conversation-id> "Explain the highest-risk finding"
```

An invocation without `--conversation` starts a new Kibana conversation. Do not create or read a local conversation map. Progress is written to stderr; consume only the finite stdout outcome.

If the outcome reports an interrupted conversation with `retry_safe: false`, do not submit the prompt again automatically. Direct the user to its `kibana_url`, where the existing conversation is the recovery location.

Agent Builder selects diagnostics and performs reasoning with the tools and model configured on the output deployment. Do not reproduce metrics, thresholds, freshness lookups, binding checks, or diagnostic analysis locally.

## References

- `references/onboarding.md`: safe first-run and skill-installation handoff.
- `references/cli.md`: CLI behavior and output-deployment resolution.
- `references/env-vars.md`: supported ESDiag environment variables.
