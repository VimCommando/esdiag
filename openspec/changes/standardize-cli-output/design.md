## Context

The CLI currently reduces most successful commands to `CommandResult { name, summary }`, formats domain values into prose, and writes that prose to stderr outside tracing. List commands bypass `CommandResult` and print terminal tables to stdout. This protects the special `process ... -` NDJSON stream, but it leaves no uniform machine-readable result.

The useful facts already exist in typed terminal states: `CollectionResult`, `Completed`, `IncludedDiagnosticOutcome`, `JobRunOutcome`, host and keystore models, and uploader responses. The missing abstraction is a compact CLI-facing projection of those terminal values. Serde and `serde_json` are already dependencies, tracing already writes to stderr, and the deprecated `serde_yaml` dependency can be replaced by the maintained `yaml_serde` crate from the `yaml-serde` project without carrying two YAML implementations.

The Agent Skill's `extract-diagnostic.sh` exists only because the current completion prose has no schema. Structured outcomes make that parser obsolete. The skill's other helpers own configuration, HTTP, SSE, and query behavior and are deliberately handled by the separate `add-first-run-onboarding` and `add-agent-cli` changes.

## Goals / Non-Goals

**Goals:**

- Give every existing finite CLI operation one stable, deserializable result on stdout.
- Make pretty block-style YAML the default representation for both people and agents, with compact JSON available explicitly.
- Keep progress and diagnostic logging on stderr, separate from results.
- Keep outcomes small by exposing references, counts, status, and timing rather than serializing internal processor state or reports wholesale.
- Represent success and failure outcomes structurally while retaining conventional process exit codes.
- Remove prose output scraping and `extract-diagnostic.sh` from the portable Agent Skill.
- Preserve the existing NDJSON stdout stream as a valid structured payload.
- Use one maintained in-process YAML implementation and no external formatter binaries.

**Non-Goals:**

- Adding Agent Builder, analysis, binding, freshness, onboarding, or configuration commands.
- Removing the skill's remaining configuration or Agent Builder helpers before their native replacements exist.
- Changing collection, processing, exporting, setup, host, keystore, or saved-job semantics.
- Changing Web UI, HTTP API, Datastar SSE, or desktop application payloads.
- Serializing `Completed`, `DiagnosticReport`, or other internal typestate values directly when doing so would expose excessive implementation detail.
- Turning `--help`, `--version`, Clap usage errors, interactive prompts, or shell completions into YAML.
- Adding a human-table output mode or selecting formats implicitly from terminal detection.

## Decisions

### Return compact CLI outcome projections

Replace prose-bearing `CommandResult` with a `CliOutcome` enum whose variants contain compact command-specific result structs. Use an internally tagged, snake_case Serde representation with a short `result` discriminator. Optional values and empty collections are omitted when their absence is unambiguous.

For example, a processing result is shaped like:

```yaml
result: diagnostic_processed
diagnostic:
  id: prod-es@2026-08-08~a1b2
  documents: 18432
  duration_ms: 4212
  kibana_url: https://kibana.example/s/esdiag/app/dashboards/...
```

A collected archive uses a different typed variant:

```yaml
result: archive_collected
path: /diagnostics/prod-es-20260808.zip
files:
  successful: 19
  total: 20
```

This is preferable to serializing `JobRunOutcome::Processed(Box<Completed>)` directly: `Completed` contains internal state and report data far beyond what a caller needs. Conversion implementations project only durable public facts from terminal domain values.

Alternative considered: a generic `{status, command, message, data}` envelope. It preserves prose as the de facto API and adds redundant keys to every invocation.

### Use one explicit format selector

Add a global `--format <yaml|json>` option, defaulting to `yaml` regardless of terminal detection or agent mode. YAML uses `yaml_serde::to_writer` from the `yaml-serde` project for readable block output. JSON uses compact `serde_json::to_writer`.

Migrate existing project imports from `serde_yaml` to `yaml_serde` in the same change and remove `serde_yaml` from `Cargo.toml` and `Cargo.lock`. Stable structs preserve field order for human readability, enum and field names use snake_case, durations use integer `duration_ms`, and URLs and paths are strings. Secrets, credential material, and raw internal errors are never serialized.

Schema evolution follows CLI semantic versioning: additive optional fields are compatible; removing or renaming fields or discriminator values is breaking.

Alternative considered: JSON by default. JSON has broader parser availability but costs more punctuation tokens and is less comfortable for interactive reading.

### Enforce a single-purpose stdout channel

Tracing, warnings, and progress stay on stderr. Finite commands write exactly one YAML document or JSON value to stdout after successful completion. Table/list commands return sequences inside that outcome instead of rendering tables or terminal colors.

Commands fall into three output categories:

1. **Finite result:** collect, process to a non-stream target, upload, setup, host, keystore, and job commands emit one terminal outcome.
2. **Structured stream:** `process ... -` and server workflows explicitly configured to export documents to stdout retain the existing NDJSON schema and do not append a YAML/JSON terminal result.
3. **Long-running readiness:** `serve` emits one structured readiness value after binding successfully when stdout is otherwise unused; if its configured exporter owns stdout, readiness remains an operational stderr event.

The selected category is resolved before execution and cannot change during a run.

Alternative considered: write a YAML completion document after NDJSON. That would make the stream ambiguous and break consumers expecting every line to be a processed document.

### Serialize failures after command execution begins

Replace `main() -> Result<()>`'s terminal error rendering with an explicit exit path. For finite-result commands, a failure writes a compact `CliFailure` value to stdout in the selected format and exits non-zero. The failure contains a stable category and safe message, with optional allowlisted context; detailed cause chains remain available through stderr debug logging.

Clap-controlled help, version, and argument-parse failures remain text because they occur before command execution. Structured-stream failures do not inject a foreign failure value into a partial NDJSON stream; they exit non-zero and report the operational error on stderr.

Alternative considered: serialize failures to stderr. Mixing logs and a structured terminal value makes reliable deserialization impossible unless logging is disabled.

### Map only terminal workflow states

Outcome projection happens only after existing state transitions complete:

```text
Collector configured -> CollectionResult -> CollectOutcome
Processor<Ready> -> Processor<Started> -> Completed -> ProcessOutcome
Job -> JobRunOutcome -> JobOutcome
```

Failed transitions return `CliFailure` and never fabricate a success outcome. Included diagnostics retain completed, skipped, or failed variants inside the parent process result.

No receiver, processor, or exporter trait changes are needed. The formatting boundary is a generic `write_outcome<T: serde::Serialize>` function selected by `OutputFormat`; domain-to-CLI conversions remain beside the relevant outcome definitions.

### Remove only the parser made obsolete by this contract

Once collect, process, and job outcomes expose diagnostic IDs, paths, links, and included results, remove `extract-diagnostic.sh` from the canonical and generated skill directories and replace its tests with direct YAML/JSON deserialization assertions.

Do not translate the other helper programs into Markdown and do not claim structured output replaces their responsibilities. Their removal is sequenced through `add-first-run-onboarding` and `add-agent-cli`, which establish canonical configuration and native Agent Builder transport.

## Risks / Trade-offs

- **Breaking existing prose and table consumers** → Document field mappings, support explicit JSON, and update repository consumers in the same change.
- **YAML parsers differ on implicit scalar typing** → Emit strings for identifiers, URLs, paths, and enums; use integers only for counts and milliseconds; test round trips.
- **Project-wide YAML dependency migration changes persistence code** → Retain serialized fixtures and run host, job, settings, setup, and keystore serialization tests before removing `serde_yaml`.
- **Outcome structs drift from domain facts** → Construct them directly from terminal typed values and add exhaustive conversion tests.
- **Accidental secret disclosure** → Use allowlisted output structs and add negative tests for API keys and passwords.
- **Partial NDJSON failures lack a structured final error** → Preserve the stream schema and rely on non-zero exit plus stderr.
- **Temporary skill scripts remain after this change** → Remove only the unsafe prose parser now and track the other helpers in their owning changes.

## Migration Plan

1. Replace `serde_yaml` with `yaml-serde` across existing persistence, configuration, setup, and test call sites while preserving serialized fixtures.
2. Introduce `OutputFormat`, `CliOutcome`, `CliFailure`, and compact projection types behind unit tests.
3. Convert list and status commands, then mutations, then collect/process/upload/setup, and finally saved-job variants.
4. Replace top-level summary/error emission with the format-aware stdout writer while retaining tracing on stderr.
5. Preserve and test the NDJSON stream category and add structured `serve` readiness output.
6. Update CLI documentation, repository consumers, Agent Skill instructions, and changelog to deserialize structured outcomes.
7. Delete `extract-diagnostic.sh` from canonical and generated skill assets after parser-free tests pass.

Rollback requires reverting the release because the default output contract is intentionally breaking; no persisted data migration is involved.

## Open Questions

None. Agent Builder transport, general configuration, first-run onboarding, and removal of the remaining scripts are explicitly owned by their separate changes.
