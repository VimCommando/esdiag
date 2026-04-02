## 1. Workflow UI Structure

- [x] 1.1 Add dedicated advanced workflow pages with separate `Collect`, `Process`, and `Send` panels while leaving the main home page unchanged for now.
- [x] 1.2 Expand the web signal/state model to track the selected mode for each stage (`Collect`/`Upload`, `Process`/`Forward`, `Remote`/`Local`) plus each mode's configuration independently.
- [x] 1.3 Add mode-aware `Collect -> Collect` inputs for known-host selection, direct URL/API-key entry, Elastic Upload Service input, diagnostic type selection, and `Save` controls that advertise browser-managed retained downloads instead of a user-entered directory target.
- [x] 1.4 Add `Collect -> Upload` inputs for drag-and-drop and file-picker local archive intake.

## 2. Collection and Orchestration

- [x] 2.1 Refactor workflow orchestration so `Collect -> Collect`, Elastic Upload Service intake, and `Collect -> Upload` all normalize into a shared workflow input contract for downstream stages.
- [x] 2.2 Replace browser workflow `Collect` save path handling with server-managed retained archive bundles that can be reused by downstream processing/forwarding without requiring direct writes to a user-selected filesystem path.
- [x] 2.3 Preserve the current one-job on-demand path for unsaved `collect + process + send` and keep the saved two-job handoff path, but have the saved path retain an archive bundle for download and later processing/send.
- [x] 2.4 Add `Process -> Forward` execution branching so the raw collected archive is preserved unchanged.
- [x] 2.5 Add a dedicated download endpoint or equivalent bundle-delivery route so saved workflow bundles are fetched outside the SSE status stream.
- [x] 2.6 Add server-managed bundle retention and cleanup semantics for saved workflow bundles, including lifecycle handling after download or expiry.

## 3. Processing Controls

- [x] 3.1 Add `Process -> Process` selectors for diagnostic product and diagnostic type and bind them into workflow execution.
- [x] 3.2 Implement the advanced-options accordion and populate its checkbox list from only fully implemented product processors, using a per-product enum or registry with dependency metadata if module inference is not clean at runtime.
- [x] 3.3 Apply advanced processing overrides so the selected supported subset controls which APIs are processed without allowing required processors or dependencies to be deselected.
- [x] 3.4 Implement required-processor locking from dependency and metadata/manifest rules, preferably reusing dependency metadata from the same per-product enum or registry that defines implemented processors, including Elasticsearch cases such as `node_settings` for `node_stats` plus always-required `version` and `cluster_settings_defaults`.

## 4. Send Target Integration

- [x] 4.1 Move footer output selection into the `Send` panel and map `Remote`/`Local` choices onto the existing exporter options where compatible.
- [x] 4.2 Add `Send -> Remote` behavior for processed-output delivery to diagnostic cluster targets and forwarded archive delivery through the new Elastic Upload Service uploader capability.
- [x] 4.3 Add `Send -> Local` behavior for processed-output delivery to localhost diagnostic clusters or local directories.
- [x] 4.4 Filter known-host send targets to Elasticsearch hosts with the `send` role, further restrict local known-host targets to `localhost`/`127.0.0.1`, and disable incompatible target types as `Collect` and `Process` selections change.
- [x] 4.5 Implement `Forward + Local` behavior so local send is disabled, the UI explains that the local bundle download is handled in `Collect`, and `Collect` save is automatically enabled if it was off.
- [x] 4.6 Enforce runtime-mode validation rules for known-host usage and retained browser-download bundle behavior in the new workflow, while keeping advanced workflow routes user-mode-only for now.
- [x] 4.7 Auto-initiate the retained bundle download from the same `Go` action through a separate browser request/action while allowing processing and send status to continue over SSE.

## 5. Elastic Uploader

- [x] 5.1 Implement the new CLI command `esdiag upload <file_name> <upload_id>` for unprocessed diagnostic bundle upload.
- [x] 5.2 Add the Elastic Upload Service uploader implementation, using `/Users/reno/Development/elastic/eluploader/cmd/eluploader` as a behavior reference.
- [x] 5.3 Wire `Process -> Forward` plus `Send -> Remote` to invoke the uploader capability instead of processed-document export.

## 6. Verification

- [x] 6.1 Add or update UI/integration tests covering `Collect -> Collect` in user mode with known-host selection, `Collect -> Upload`, and service-mode route gating for advanced workflow pages.
- [x] 6.2 Add or update tests for `Process -> Process` and `Process -> Forward`, including advanced processor filtering and required dependency locking.
- [x] 6.3 Add or update tests that invalid `Send` targets become disabled when `Collect` or `Process` options make them incompatible.
- [x] 6.4 Add or update tests for `Send -> Local` processed delivery to localhost clusters/directories and `Forward + Local` auto-enabling `Collect` save/download behavior.
- [x] 6.5 Add or update tests for browser-managed saved bundle download behavior in the user-mode advanced workflow, including auto-trigger from the same workflow execution.
- [x] 6.6 Add or update tests for retained bundle download endpoints and cleanup semantics.
- [x] 6.6 Add or update tests for the `upload` CLI command and forwarded remote uploader behavior.
- [x] 6.7 Run `cargo clippy` and resolve any new warnings in changed code.
- [x] 6.8 Run `cargo test` and confirm relevant suites pass.
