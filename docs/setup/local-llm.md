---
type: Guide
title: Connect Agent Builder to a local LLM
description: Register an OpenAI-compatible local model as an Elasticsearch inference endpoint.
tags: [setup, agent-builder, inference, ollama]
---

# Connect Agent Builder to a local LLM

Agent Builder can use a local model through an Elasticsearch
`chat_completion` inference endpoint. The model server must implement the
OpenAI chat completions API, including native tool calling. Ollama is one
option.

This guide uses an Ollama model named `gemma4:26b` and registers it as
`local-ollama-gemma`. Replace both names in every example when using another
model.

## Before you start

You need:

- An Elastic license that includes Agent Builder.
- The `manage_inference` Elasticsearch cluster privilege.
- Access to Agent Builder in the target Kibana space.
- An OpenAI-compatible model with reliable tool calling.
- A URL that the Elasticsearch node can reach.
- An authenticated `elastic` CLI context for the command-line examples.

Elasticsearch calls the model server. A URL that works in your browser or from
the Kibana container may still fail from the Elasticsearch container.

Common endpoint URLs include:

| Deployment | URL |
|---|---|
| Elasticsearch and Ollama on the same host | `http://localhost:11434/v1/chat/completions` |
| Elasticsearch in Podman, Ollama on the host | `http://host.containers.internal:11434/v1/chat/completions` |
| Elasticsearch in Docker Desktop, Ollama on the host | `http://host.docker.internal:11434/v1/chat/completions` |
| Model on another machine | `http://model-host.example:11434/v1/chat/completions` |

When the model runs on another machine, bind it to a reachable interface and
restrict the port with the host firewall. Do not expose an unauthenticated
model endpoint to an untrusted network.

Check the model before configuring Elastic:

```sh
curl http://model-host.example:11434/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gemma4:26b",
    "messages": [
      {
        "role": "user",
        "content": "Reply with exactly READY"
      }
    ]
  }'
```

If Elasticsearch runs in a container, run the same check inside that
container. This catches DNS, port, and host-routing problems before Elastic
stores the endpoint.

## Create the inference endpoint

Open Kibana Dev Tools in the space where you use Agent Builder. Register the
model with the Elasticsearch inference API:

```http
PUT _inference/chat_completion/local-ollama-gemma
{
  "service": "openai",
  "service_settings": {
    "api_key": "ollama-local",
    "model_id": "gemma4:26b",
    "url": "http://host.containers.internal:11434/v1/chat/completions",
    "rate_limit": {
      "requests_per_minute": 500
    }
  }
}
```

Elasticsearch requires an `api_key` for the OpenAI service. Ollama ignores the
placeholder value in this example. If the model server enforces
authentication, replace it with the real key.

The equivalent `elastic` CLI command is:

```sh
elastic es inference put-openai \
  --openai-inference-id local-ollama-gemma \
  --task-type chat_completion \
  --service openai \
  --service-settings '{
    "api_key": "ollama-local",
    "model_id": "gemma4:26b",
    "url": "http://host.containers.internal:11434/v1/chat/completions",
    "rate_limit": {
      "requests_per_minute": 500
    }
  }'
```

Use `--dry-run` first when generating the command in automation. It validates
the CLI arguments but does not contact Elasticsearch or create the endpoint.

## Inspect and test the endpoint

Read the stored configuration in Kibana Dev Tools:

```http
GET _inference/chat_completion/local-ollama-gemma
```

The CLI equivalent is:

```sh
elastic es inference get \
  --inference-id local-ollama-gemma
```

Send a streaming chat completion from Kibana Dev Tools:

```http
POST _inference/chat_completion/local-ollama-gemma/_stream
{
  "messages": [
    {
      "role": "user",
      "content": "Reply with exactly READY"
    }
  ]
}
```

The CLI command uses the unified chat-completion request:

```sh
elastic es inference chat-completion-unified \
  --inference-id local-ollama-gemma \
  --chat-completion-request '{
    "messages": [
      {
        "role": "user",
        "content": "Reply with exactly READY"
      }
    ]
  }' \
  --timeout 2m
```

The response is a stream of events. Confirm that it names the expected model
and ends with the requested text.

## Test Agent Builder

An inference response proves transport and model compatibility with the OpenAI
API. It does not prove that the model can run an agent. Route one Agent Builder
request to the new endpoint.

Kibana Dev Tools accepts `kbn://` requests for Kibana APIs. Run this from the
intended Kibana space:

```http
POST kbn://api/agent_builder/converse
{
  "agent_id": "elastic-ai-agent",
  "inference_id": "local-ollama-gemma",
  "input": "Respond with exactly READY without using tools."
}
```

The equivalent CLI command is:

```sh
elastic kb agent-builder post-agent-builder-converse \
  --agent-id elastic-ai-agent \
  --inference-id local-ollama-gemma \
  --input "Respond with exactly READY without using tools."
```

Then test a question that requires an Agent Builder tool. Local models vary
widely in function-calling accuracy. A successful `READY` response does not
show that the model can select tools, produce valid arguments, or recover from
tool errors.

## Select the model

Open **AI Agent** and choose `local-ollama-gemma` from the model selector to
use it for that conversation.

To make it the default for requests that omit `inference_id`, open **Feature
Settings** from Kibana global search. Set the Agent Builder default model to
`local-ollama-gemma`. This default applies to `esdiag agent ask` and
`esdiag process --ask`, which let Agent Builder select its configured default
model.

API clients can route one request without changing the default by including:

```json
{
  "inference_id": "local-ollama-gemma"
}
```

Do not send both `inference_id` and `connector_id`. Agent Builder rejects that
request.

## Troubleshooting

`401` from Kibana means the CLI context lacks working Kibana authentication.
Check it with:

```sh
elastic status --json
```

A timeout or connection error from `_inference` usually means Elasticsearch
cannot reach the model URL. Test the URL from the Elasticsearch host or
container.

Errors such as `Invalid function call syntax` or `No tool calls found in the
response` mean the endpoint works but the model did not follow Agent Builder's
tool protocol. Use a model with stronger native function calling or adjust the
model server's OpenAI compatibility settings.
