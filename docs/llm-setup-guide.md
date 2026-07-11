# Elastic AI Agent: LLM Configuration Guide

This guide outlines the prerequisite requirements and step-by-step instructions for configuring Large Language Models (LLMs) to power the Agentic Diagnostic Assistant (ADA) and Elastic AI Agent.

It covers both the cloud-hosted **Elastic Inference Service (EIS)** integration and a **Local LLM** deployment designed for air-gapped or restricted enterprise environments.

---

## Prerequisites

Before starting the configuration, ensure your Elasticsearch and Kibana deployment meets the following foundational requirements:

* **Kibana 9.4 Minimum**: ADA relies on the brand-new Skills API, which requires Kibana version 9.4 or higher. Older versions (like 9.2 or 9.3) do not support the skills framework.  
* **Enterprise License**: Advanced orchestration features, workflows, and the Agent Builder require an active Enterprise license or an Enterprise Trial license.  
* **Observability View**: Switch Kibana out of the "Classic" view into either the **Observability** or **Search** solution spaces. Due to interface bugs, the AI connector settings and model management settings pages will not render correctly in Classic view.  
* **LLM Service required**: Elastic Inference Service (EIS): [https://www.elastic.co/docs/explore-analyze/elastic-inference/eis](https://www.elastic.co/docs/explore-analyze/elastic-inference/eis) or a local LLM

---

## Method 1: Cloud Connection via Elastic Inference Service (EIS)

To connect your local deployment to the cloud-hosted Elastic Inference Service (EIS), use the following user-interface guided workflow:

1. **Navigate to Cloud Connect**: Log in to Kibana, go to **Stack Management**, and search for the **Cloud Connect** settings page.  
2. **Authenticate to Elastic Cloud**: Click the connect option. This will redirect your browser to Elastic Cloud to authenticate with your organization or register for a trial.  
3. **Paste the API Key**: After successfully authenticating, copy the generated API key, return to the Kibana interface, and click **Connect**.

## Method 2: Local LLM Configuration (Air-Gapped Environments)

For air-gapped, offline, or highly restricted environments where external internet access is prohibited, you can run a local model.

Kibana treats Ollama as a standard OpenAI service provider because Ollama provides a native, OpenAI-compatible API endpoint. This allows you to perform the entire setup directly within the Kibana user interface.

### Step 1: Prepare your local LLM

Before jumping into Kibana, ensure your local LLM instance is up and running with the model you want to use. Below is an example with Ollama.

1. Open your terminal and download the model you want to run:

```shell
ollama pull llama3.2
```

2. Verify that the Ollama instance responds to the OpenAI-compatible API locally:

```shell
curl http://localhost:11434/v1/models
```

### Step 2: Set Up the OpenAI Connector in Kibana

Configure Kibana to route its generative AI and Playground features to your local computer instead of the cloud.

1. Log into your local Kibana dashboard.  
2. Navigate to **Stack Management \> Alerts and Insights \> Connectors** (or search for *Connectors* in the search bar).  
3. Click **Create connector** and select **OpenAI**.  
4. Fill out the connector configuration form with the following exact settings:  
   * **Connector name**: Name it something recognizable (e.g., `Ollama-Local`).  
   * **Select an OpenAI provider**: Choose **Other (OpenAI Compatible Service)**.  
   * **URL**: Enter the API route depending on your environment setup:  
     * *Standard local binary setup*: `http://localhost:11434/v1/chat/completions`  
     * *Docker environment (ES/Kibana running in container)*: `http://host.docker.internal:11434/v1/chat/completions` (on Linux, configure the `host-gateway` mapping)  
     * *Elastic Cloud (tunneling local port to the web)*: Tunnel your local port using a tool like ngrok and paste that public forwarding URL here (e.g., `https://<your-ngrok-id>.ngrok-free.app/v1/chat/completions`).  
   * **Default model**: Enter the exact model identifier you pulled locally (e.g., `llama3.2` or `gemma4`).  
   * **API key**: Ollama does not require local authentication, but Kibana requires this field to be filled to satisfy UI form validations. Type an arbitrary string (e.g., `local-secret`) to bypass this requirement.  
5. Click **Save**.

### Step 3: Test via Kibana Playground

Once saved, you can instantly run a Retrieval-Augmented Generation (RAG) test using your local Elasticsearch indices.

1. In the Kibana left navigation bar, go to **Search \> Playground**.  
2. Click **Connect to an LLM**.  
3. Choose the OpenAI connector you created in Step 2\.  
4. Click **Add data sources** to select the Elasticsearch data index you want your local LLM to interact with.  
5. Start typing questions into the chat bar; your queries will perform hybrid vector searches on Elasticsearch and stream private responses using your local Ollama instance\!
