# Design

Full rationale, rejected alternatives, and consequences live in
**`docs/adr/0002-unify-operations-into-one-six-stage-pipeline.md`**,
**`docs/adr/0003-name-the-universal-model-job.md`**, and
**`docs/adr/0004-universal-job-model-and-stage-aligned-modules.md`**. The `Job` model,
its invariants, and the derived execution mode are settled and implemented by
`unified-job-model`; this design covers only the convergence of the remaining surfaces
onto them.

## Context

The executor exists and drives staged, streaming, `Load`-input, and Export-and-Send job
shapes, but only `esdiag job run` reaches it. Three legacy paths remain:

- `main.rs` runs `collect` / `process` / `read` through `Collector` and `Processor`, with
  the CLI-streaming path and the job-staged process path as two code paths for one
  operation.
- `src/server/job_runner.rs` has its own `execute_remote_collection_job`,
  `run_processor_job`, and `run_forward_job`, and `JobSignals` is an independent model
  rather than a projection of `Job`.
- Included diagnostics run as child *executions* — their own child job IDs and inherited
  `Platform` — but not as literal `model::Job` values through the executor.

## Approach

Converge one surface at a time, keeping the executor unchanged, so a regression is
attributable to the surface that moved:

1. **Stage-aligned modules first.** Split `exporter/` by role and reduce `processor/` to
   transform-only, so the CLI and web paths have role-typed sinks to construct against
   before they are rewritten.
2. **CLI next.** `collect` / `process` / `read` build a `Job` and hand it to the executor;
   `collect --upload` becomes `Collect` + `save` + `send`. This is the larger of the two
   surfaces but has the simpler state, so it exercises the executor against real
   invocations before the web form moves.
3. **Web last.** `job_runner` constructs `Job`s and `JobSignals` becomes a projection. The
   `Send` panel then derives target availability from the active phases rather than from
   its own rules, which is what makes Export and Send selectable together.
4. **Child jobs.** With the executor driving the web and CLI paths, spawn included
   diagnostics as literal child `Job`s. The semantics — child job IDs, parent `Platform`
   propagation, one level deep — already hold, so this is a restructuring rather than a
   behavior change.
5. **Retire.** Once nothing constructs them, remove `Collector`, `Processor`, and
   `into_collect_exporter`.

Removing the one-/two-job requirement is the last step, not the first: it stays true of the
web workflow until step 3 lands.

## Risks

- **Wide blast radius.** A 99KB `main.rs` and the web runner. Mitigated by the ordering
  above and by the executor already being covered by model, invariant, mode, and
  `Load`-input tests.
- **Streaming/staged convergence.** The one executor must preserve current streaming
  concurrency and backpressure (`document_channel`); a regression would surface as a memory
  or throughput change rather than a test failure, so the streaming regression test
  (task 7.4 inherited here) gates retirement of the old path.
- **UI projection.** Collapsing `JobSignals` risks web regressions; keep the UI verbs
  stable as a presentation projection over the phases rather than a parallel model.
