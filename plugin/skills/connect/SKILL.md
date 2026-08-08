---
name: connect
description: Validate that this machine can use an already-provisioned ESDiag deployment without creating resources, model calls, or Agent Builder conversations. Use when binding the Claude Code plugin, checking its API key and URLs, or diagnosing connection failures.
---

# Connect to an ESDiag deployment

Run the read-only, conversation-free binding check:

```sh
"${CLAUDE_PLUGIN_ROOT}/scripts/connect.sh"
```

Configuration comes from environment variables documented in the plugin README. Required:

- `ESDIAG_KIBANA_URL`
- `ESDIAG_ELASTICSEARCH_URL` or existing `ESDIAG_OUTPUT_URL`
- `ESDIAG_KIBANA_APIKEY` or `ESDIAG_KIBANA_APIKEY_FILE`

Report the result and help with the attributed failure:

- **Client configuration**: invalid URLs, rejected API key, missing configured agent, or unreadable ESDiag data streams.
- **Cluster provisioning**: Agent Builder or the ESDiag assets are absent.
- **Deployment model prerequisite**: reported by the first real analysis if the agent has no usable model. The binding check deliberately avoids creating a throwaway conversation to probe it.

A missing `esdiag` binary, locked keystore, or absent saved hosts affects collection and processing only. Existing diagnostics can still be analyzed.
