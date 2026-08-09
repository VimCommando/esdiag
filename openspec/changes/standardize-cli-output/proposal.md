## Why

`esdiag` currently treats human-readable completion messages and log lines as its command result, forcing humans, agents, and applications to scrape prose for identifiers, paths, links, and status. Because tracing already goes to stderr and stdout is generally clean, the CLI can expose small typed outcomes that reduce ambiguity, dependencies, and token use without expanding into configuration, querying, or analytical responsibilities.

## What Changes

- **BREAKING** Replace finite CLI commands' human-formatted completion summaries and text tables with stable, typed result values serialized as pretty-printed YAML on stdout.
- Add a global output-format option with YAML as the default and JSON as an explicit interoperability choice, using Serde-native libraries rather than external formatting tools.
- Replace the deprecated `serde_yaml` package with the actively maintained `yaml-serde` project (`yaml_serde` crate) across the codebase so ESDiag retains one YAML implementation rather than carrying parallel serializers.
- Model command success results explicitly, starting from `JobRunOutcome` and introducing similarly focused outcome types for collect, process, upload, setup, host, keystore, and job operations.
- Return structured failure details for commands that begin execution while preserving non-zero process exit status; keep operational tracing, warnings, and progress exclusively on stderr.
- Preserve stdout as a single-purpose structured channel: commands that intentionally stream NDJSON to stdout continue emitting only that stream and do not append a differently shaped final-result document.
- Keep CLI help, version output, interactive prompts, and shell-completion behavior human-oriented; the structured contract covers command execution outcomes.
- Remove `extract-diagnostic.sh` once the portable skill consumes native structured collect, process, and job outcomes.
- Document the output schemas, compatibility expectations, stream exception, and migration from prose matching to deserialization.

## Capabilities

### New Capabilities

- `cli-structured-output`: Typed command outcomes, YAML/JSON serialization, stdout/stderr boundaries, structured failures, format stability, and stream-producing command behavior.

### Modified Capabilities

- `cli-agent-mode`: Replace final human-readable summaries on stderr with the same structured stdout result used in every invocation mode while retaining low-noise stderr logging.
- `cli-host-record-management`: Replace the saved-host text table and empty-state sentence with a structured host-list outcome.
- `saved-jobs`: Replace saved-job text tables and prose run summaries with structured list, mutation, and `JobRunOutcome` representations.
- `included-diagnostic-jobs`: Represent completed, skipped, and failed included diagnostics as typed child outcomes rather than prose sections in the CLI summary.

## Impact

- **Target Elastic products:** Elasticsearch, Kibana, and Logstash collection and processing commands receive the same output contract; diagnostic collection and processing behavior is unchanged.
- **CLI:** All existing finite execution paths in `src/main.rs` and `src/job.rs` return serializable outcomes. Existing consumers that parse completion prose or text tables must migrate to YAML or request JSON explicitly.
- **Web UI and APIs:** No response or rendering changes. Shared outcome types may be reused, but web protocols retain their existing JSON/SSE contracts.
- **Core processing:** Processor, receiver, and exporter behavior remains unchanged except for exposing the typed facts needed to build a final outcome.
- **Dependencies:** Replace deprecated `serde_yaml` with the `yaml_serde` crate from the `yaml-serde` project and retain existing `serde` and `serde_json` dependencies.
- **Agent assets:** The portable skill removes only its prose-output parser in this change. General onboarding, Agent Builder transport, and deletion of the remaining shell helpers belong to `add-first-run-onboarding` and `add-agent-cli`.
