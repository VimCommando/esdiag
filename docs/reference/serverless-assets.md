---
type: Reference
title: Serverless asset compatibility
description: Elasticsearch and Kibana setup asset audit and live verification.
tags: [setup, serverless, elasticsearch, kibana]
---

# Serverless asset compatibility

Audited for issue [#390](https://github.com/elastic/esdiag/issues/390) on
2026-09-04 using the saved `esdiag-less` and `esdiag-less-kb` hosts. Both
products reported version `9.6.0` and build flavor `serverless`.

Setup reads `version.build_flavor` from Elasticsearch's root response or
Kibana's `/api/status`. It does not infer deployment type from the hostname.
An Elasticsearch security usage response of HTTP 410 reports security as
enabled. Deployment metadata selects the Serverless asset adaptations;
security status does not determine deployment compatibility.

## Findings and changes

| Item | Finding | Setup behavior |
| --- | --- | --- |
| Security usage | `/_xpack/usage` returns HTTP 410; Serverless always has security enabled | Skip the probe on a known Serverless deployment and report security as enabled for HTTP 410. |
| Bundled role | The stateful role asset is not provisioned on Serverless | Skip security-dependent assets and tell the administrator to configure project roles separately. Authentication remains enabled. |
| `esdiag@settings` | `index.lifecycle.prefer_ilm` and `index.lifecycle.name` are unsupported ILM settings | Remove these settings from outgoing Serverless templates. Keep stateful assets unchanged. |
| Retention | Data stream lifecycle is supported | Preserve `template.lifecycle.data_retention: 30d`. |
| Kibana space | The project rejects `solution` and a nonempty `disabledFeatures` list | Omit both controls for Serverless. Preserve the space ID, name, description, and appearance. |
| Trial license | Serverless manages feature entitlements | Skip `/_license` and `/_license/start_trial`; Kibana APIs still report missing feature access or privileges. |
| Default agent | A GET response includes fields that the update API rejects, including `access_control.entries` | Send only the existing configuration with the ESDiag skill added. Preserve existing skill IDs and avoid duplicates. |

The remaining template settings are `index.codec`, `index.mapping.source.mode`,
`index.mapping.ignore_malformed`, `index.mapping.total_fields.limit`,
`index.mapping.total_fields.ignore_dynamic_beyond_limit`, and
`index.query.default_field`. All appear in Elastic's
[Serverless settings list](https://www.elastic.co/docs/reference/elasticsearch/index-settings/serverless).
The regression test inventories settings across every component and index
template and requires a compatibility review when that list grows.

The agent update follows the
[partial update API](https://www.elastic.co/docs/api/doc/kibana/operation/operation-put-agent-builder-agents-id).
It does not modify the agent's access controls.

## Retention

The shared `esdiag@settings` component configures
`template.lifecycle.data_retention: 30d`, including on Serverless. Removing
unsupported ILM settings preserves this data stream lifecycle configuration.

Diagnostic reports in `metrics-diagnostic-esdiag` are retained indefinitely.
Their index template disables data stream lifecycle and clears the inherited
retention. Template changes apply to newly created data streams; existing
report streams require their lifecycle to be updated separately.

## Asset inventory

| Asset family | Count | Audit scope |
| --- | ---: | --- |
| Elasticsearch ingest pipelines | 1 | `set` processors and `reroute`; installation and simulation |
| Elasticsearch component templates | 7 | Settings, mappings, metadata, and composition |
| Elasticsearch index templates | 26 | Settings, mappings, data streams, and composition |
| Elasticsearch roles | 1 | Skipped on Serverless |
| Kibana spaces | 1 | Creation and update with project-managed controls omitted |
| Kibana saved objects | 90 | Dashboards, data views, Lens, saved searches, Vega visualizations, links, and tags |
| Kibana workflows | 1 | Installation and its Elasticsearch authentication and ES|QL request definitions |
| Agent Builder tools | 1 | Workflow reference and installation |
| Agent Builder skills | 1 | Installation, referenced diagnostic guides, and attachment to the default agent |
| Custom Agent Builder agents | 0 | The bundle extends the built-in agent |

ILM, SLM, allocation, node, and shard names inside diagnostic mappings and
dashboard queries describe imported source data. They do not configure those
features on the destination. The audit retains them, including the lifecycle
dashboards and data views. No bundled query uses the unsupported
`scripted_metric` aggregation, and the templates do not define join fields.
See Elastic's [Hosted and Serverless comparison](https://www.elastic.co/docs/deploy-manage/deploy/elastic-cloud/differences-from-other-elasticsearch-offerings).

The `sources.yml` files define diagnostic **collection** requests, including
stateful APIs such as ILM and node statistics. Setup does not send these
requests. This audit covers Serverless as the diagnostic destination and
viewer; it does not establish Serverless diagnostic collection support.

## Repeat the verification

Both setup commands completed successfully on the audited project. The live
audit passed after reading all 34 installed Elasticsearch assets, simulating
all 26 composed index templates and the pipeline, finding all 90 Kibana saved
objects, verifying their references, and checking the workflow, tool, skill,
and default-agent attachment. Embedded Vega searches also succeeded.

Kibana may assign new IDs when an imported object already exists in another
space. The audit matches these objects by `originId` and checks their actual
references; it does not assume every imported object retains its original ID.

Use an unlocked keystore and saved Elasticsearch and Kibana hosts for a
Serverless test project. These setup commands install or update ESDiag assets:

```sh
cargo build --bin esdiag
target/debug/esdiag setup esdiag-less
target/debug/esdiag setup esdiag-less-kb
```

The explicit live test reads installed assets, simulates composed templates
and the ingest pipeline, checks Kibana objects and default-agent skill
attachment, and executes embedded Vega searches. It does not create
diagnostic data or run an agent conversation:

```sh
ESDIAG_SERVERLESS_TEST_HOST=esdiag-less \
ESDIAG_SERVERLESS_TEST_KIBANA_HOST=esdiag-less-kb \
cargo test --lib setup::serverless_tests::live_serverless_asset_audit -- --ignored --nocapture
```

Offline checks:

```sh
cargo test --lib setup::
cargo test --test serverless_setup_tests
cargo test --test elasticsearch_asset_templates_tests
```

Live validation covers API acceptance and asset composition. Dashboard
rendering, every ES|QL query against representative diagnostic data, and
workflow execution under an interactive user's identity need separate
functional testing. Agent Builder availability depends on project feature
tiers and privileges, as described in the
[Agent Builder setup guide](https://www.elastic.co/docs/explore-analyze/ai-features/agent-builder/get-started).
