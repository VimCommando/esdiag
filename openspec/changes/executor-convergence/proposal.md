## Why

`unified-job-model` (ADR-0002, ADR-0003, ADR-0004) landed the phase-structured `Job` and
the one executor that derives staged versus streaming and drives both. It landed them
*behind* the existing surfaces, per its own phased strategy: only `esdiag job run` builds a
`Job` and hands it to `job::executor::execute`. `esdiag collect` / `process` / `read` still
run through `Collector`/`Processor`, and the web `job_runner` still has its own
`execute_remote_collection_job` / `run_processor_job` / `run_forward_job` paths with
`JobSignals` as an independent model rather than a projection of `Job`.

This change is the second half: retire the legacy operation types and route every surface
through the one executor. It is the work tracked by #368, split out of `unified-job-model`
so that change can archive describing only the model and executor it actually shipped.

## What Changes

- Realign the modules to the stages: `receiver/` resolves Phase-1 input uniformly for
  `Collect` (remote, client) and `Load` (local/download, no client); `processor/` reduces to
  transform-only per-API processors; `exporter/` splits by role into `BundleExporter`
  (`Save`, raw) and `DocumentExporter` (`Export`, processed); `uploader.rs` is `Send`.
- **BREAKING (internal):** retire `Collector` and `Processor` as distinct operation types
  and `into_collect_exporter`; converge the duplicate CLI-streaming and job-staged process
  paths onto the one executor.
- CLI `collect` / `process` / `read` build a `Job` and hand it to the executor; the
  `collect --upload` handoff becomes a `Collect` + `save` + `send` job.
- The web form binds the `Job` phases directly and `JobSignals` collapses to a thin
  presentation projection. The `Send` panel derives target availability from the active
  phases, so a processed-output target and a raw-bundle target may be enabled together when
  a bundle is retained.
- Included diagnostics become literal child `Job`s (`Load` input plus `Process`) under the
  same executor, each minting a child `JobID`, with the parent setting each child's
  `Platform` as it spawns it. Fan-out stays one level deep.
- With the web runner converged, the one-/two-job workflow boundary disappears and its
  requirement is removed.

## Capabilities

### New Capabilities

- _(none — this modifies existing capabilities)_

### Modified Capabilities

- `collection-execution`: remove the one-/two-job boundary, now an artifact of the legacy
  always-staged web path.
- `diagnostic-workflow`: bind the web workflow to the unified `Job` phases (`JobSignals`
  becomes a projection); make Phase 3 *and/or* so a processed job MAY also forward its raw
  bundle in the same run.
- `included-diagnostic-jobs`: each included diagnostic executes as a child `Job` (a
  `Load`-input, processing job) under the one executor, minting a child `JobID`.

## Impact

- **Core:** retires `Collector`, `Processor` as operation types, and
  `into_collect_exporter`; restructures `receiver/`, `processor/`, `exporter/`, and
  `uploader.rs` around the stages. The `job/` module and its executor are already in place
  from `unified-job-model` and are not redesigned here.
- **CLI:** `main.rs` routes `collect` / `process` / `read` through the executor.
- **Web UI:** `job_runner` constructs `Job`s; `JobSignals` reduced to a projection; the
  `Send` panel can enable Export **and** Send together.
- **Risk:** wide and regression-prone — a 99KB `main.rs` and the web runner. Streaming
  concurrency and backpressure (`document_channel`) must be preserved across the
  convergence, which is why the model landed and was reviewed first.
- **Depends on** `unified-job-model` (archived) for the `Job` model and executor.
