# Tasks

Inherited from `unified-job-model` when it was split for archive; the numbering follows
that change's sections so the tracking issue (#368) and the design's phasing still line up.

## 3. Stage-aligned modules
- [ ] 3.1 `receiver/` — resolve Phase-1 input uniformly for `Collect` (remote, client) and `Load` (local/download, no client).
- [ ] 3.2 `processor/` — reduce to transform-only per-API processors; remove collection/sink orchestration.
- [ ] 3.3 `exporter/` — split by role into `BundleExporter` (`Save`, raw) and `DocumentExporter` (`Export`, processed); make processed-to-bundle and raw-to-cluster unrepresentable.
- [ ] 3.4 `uploader.rs` — the `Send` stage over an existing bundle.

## 4. Retire legacy types and paths
- [ ] 4.1 Remove `Collector` and `Processor` as distinct operation types; route both through the executor.
- [ ] 4.3 Remove `into_collect_exporter`.
- [ ] 4.4 Converge the duplicate CLI-streaming and job-staged process paths onto the one executor.

## 5. CLI and Web surfaces
- [ ] 5.1 CLI `collect` / `process` / `read` build a `Job` and hand it to the executor; map `collect --upload` to a `Collect` + `save` + `send` job.
- [ ] 5.2 Bind the web form to the `Job` phases; collapse `JobSignals` to a thin projection.
- [ ] 5.3 `Send` panel: derive target availability from active phases; allow processed-output and raw-bundle targets to be enabled together when a bundle is retained.

## 6. Child jobs
- [ ] 6.1 Spawn each included diagnostic as a child `Job` (`Load` input + `Process`) under the one executor, minting a child `JobID`.
- [ ] 6.2 Set each child job's `Platform` from the parent as it spawns; keep fan-out one level deep.

## 7. Verification
- [ ] 7.4 Regression: streaming concurrency/backpressure preserved after path convergence.
- [ ] 7.5 Test child diagnostics execute as child jobs with inherited `Platform`, one level deep.
- [ ] 7.6 Confirm the delta spec scenarios in `specs/collection-execution`, `specs/diagnostic-workflow`, and `specs/included-diagnostic-jobs` are covered.

---

## Inherited state (2026-08-01 split)

Task 4.2 (remove `JobAction` and the `JobCollect` fusion; construct `Job`s from phases) is
**not** listed here — it completed in `unified-job-model` via `saved-job-migration`, which
made the persisted job the phase model (`data::Job` re-exports `job::model::Job`) so
`JobBuilder` builds through `Job::try_new`. `LegacyJobAction` and `LegacyJobCollect` survive
only to migrate an existing `jobs.yml` (ADR-0009) and are not part of this retirement.

The 6.x semantics already hold: included diagnostics run as child executions with their own
child job IDs and parent `Platform` propagation (`spawn_sub_processors` +
`platform-application-split`). What remains is restructuring them as literal child
`model::Job`s driven by the executor.
