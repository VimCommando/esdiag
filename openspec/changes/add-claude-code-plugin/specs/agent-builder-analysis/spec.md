## ADDED Requirements

### Requirement: Analysis Delegated To The Cluster Agent
The plugin SHALL obtain diagnostic analysis by sending the user's question and a `diagnostic.id` to a Kibana Agent Builder agent through the space-scoped chat API, and SHALL present the agent's response as the analysis. The plugin MUST NOT reproduce the analysis locally by querying diagnostic indices, recomputing metrics, or substituting its own thresholds for those the agent applied.

#### Scenario: Analysis performed by the cluster
- **GIVEN** a verified `diagnostic.id` and a configured target agent
- **WHEN** the user asks how the cluster is performing
- **THEN** the plugin sends one analysis request to the configured agent
- **AND** presents the agent's response as the analysis
- **AND** does not issue its own diagnostic index queries to derive findings

#### Scenario: Agent findings are not re-derived
- **GIVEN** the agent has returned findings with observed values
- **WHEN** the plugin presents results
- **THEN** the reported values are those the agent returned
- **AND** the plugin does not recompute or override them

#### Scenario: Dashboard links preserved
- **GIVEN** the agent response contains relative dashboard links
- **WHEN** the plugin presents results
- **THEN** the links are preserved and resolved against the configured Kibana base URL

### Requirement: Streaming Progress Reporting
The plugin SHALL use the streaming chat endpoint and report progress from its event stream while analysis runs. The user MUST receive indication of ongoing activity, including which tool the agent is invoking, rather than an unreported wait. The plugin MUST NOT block silently for the duration of the request.

#### Scenario: Progress reported during a long analysis
- **GIVEN** an analysis request that takes tens of seconds
- **WHEN** the agent emits reasoning and tool call events
- **THEN** the plugin reports activity as those events arrive
- **AND** reports the tool being invoked

#### Scenario: Final message presented on completion
- **GIVEN** the event stream reaches message completion
- **WHEN** the plugin presents results
- **THEN** the completed message is presented as the analysis

#### Scenario: Interrupted stream does not silently discard work
- **GIVEN** an analysis request whose stream is interrupted after a conversation identifier was assigned
- **WHEN** the plugin handles the interruption
- **THEN** the plugin reports the interruption and retains the conversation identifier
- **AND** does not issue a duplicate analysis request without user direction

### Requirement: Diagnostic Identifier Supplied By The Client
The plugin SHALL pass an explicit `diagnostic.id` obtained from `esdiag` command output when one is known, rather than relying on the agent to determine which diagnostic is current.

#### Scenario: Identifier taken from process output
- **GIVEN** an `esdiag process` or `esdiag job run` invocation reported a diagnostic identifier
- **WHEN** the plugin requests analysis for that run
- **THEN** the request includes that identifier

#### Scenario: No identifier known
- **GIVEN** no diagnostic identifier is known to the plugin
- **WHEN** the user requests analysis
- **THEN** the plugin obtains an identifier before requesting analysis
- **AND** does not ask the agent to guess which diagnostic is current

### Requirement: Conversation Continuation For Follow-Up Questions
The plugin SHALL retain the conversation identifier returned by an analysis request and SHALL reuse it for follow-up questions about the same diagnostic, so prior context is not resent by the client. Conversation reuse MUST be scoped by Kibana deployment, space, agent, and diagnostic identifier. Every analysis conversation SHALL remain in Kibana Agent Builder history so the user can continue it from Kibana.

#### Scenario: Follow-up reuses the conversation
- **GIVEN** a completed analysis returned a conversation identifier
- **WHEN** the user asks a follow-up question about the same diagnostic
- **THEN** the request includes that conversation identifier
- **AND** the plugin does not restate the prior analysis in the request

#### Scenario: New diagnostic starts a new conversation
- **GIVEN** a conversation identifier exists for a previous diagnostic
- **WHEN** the user requests analysis of a different diagnostic
- **THEN** the plugin does not reuse the previous conversation identifier

#### Scenario: Same diagnostic on another deployment is isolated
- **GIVEN** a conversation identifier exists for a diagnostic on one Kibana deployment
- **WHEN** the user analyzes the same diagnostic identifier on a different deployment, space, or agent
- **THEN** the plugin starts a separate conversation
- **AND** does not send the identifier from the first deployment

#### Scenario: Conversation remains available in Kibana
- **GIVEN** an analysis or follow-up completes through Agent Builder
- **WHEN** the plugin reports the result
- **THEN** that exchange is present in Kibana Agent Builder conversation history
- **AND** the plugin reports the conversation identifier for handoff and troubleshooting

### Requirement: Unstructured Response Handling
The plugin SHALL treat the agent response as unstructured markdown. The plugin MUST NOT depend on a response schema, and MUST NOT parse the response into control-flow decisions such as retrying, escalating, or suppressing findings.

#### Scenario: Response shape varies
- **GIVEN** two analyses return differently structured markdown
- **WHEN** the plugin presents each result
- **THEN** both are presented without requiring a fixed field layout
- **AND** neither triggers a parse failure

#### Scenario: Response content does not drive control flow
- **GIVEN** an analysis response describing a critical finding
- **WHEN** the plugin presents results
- **THEN** the plugin does not take automated remediation or retry action based on parsing that content

### Requirement: Analysis Failure Attribution
The plugin SHALL distinguish client configuration failures from cluster provisioning failures when an analysis request fails, and SHALL report which side requires attention.

#### Scenario: Configured agent missing
- **GIVEN** the configured target agent does not exist in the configured space
- **WHEN** an analysis request fails
- **THEN** the plugin reports the configured agent as missing
- **AND** identifies this as client configuration

#### Scenario: No model available to the agent
- **GIVEN** the target agent exists but the deployment has no usable model
- **WHEN** an analysis request fails
- **THEN** the plugin reports that the deployment lacks a configured model
- **AND** identifies this as a deployment prerequisite the user must satisfy
- **AND** does not attempt to configure model access

#### Scenario: Authorization rejected
- **GIVEN** the configured API key lacks the privileges required for the chat API
- **WHEN** an analysis request is rejected
- **THEN** the plugin reports the missing authorization
- **AND** identifies this as client configuration

#### Scenario: Unknown diagnostic identifier
- **GIVEN** the supplied `diagnostic.id` does not exist in the deployment
- **WHEN** the agent reports it cannot verify the identifier
- **THEN** the plugin reports that the diagnostic was not found
- **AND** does not present an analysis
