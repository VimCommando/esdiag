## Why

Binary-first ESDiag users currently need to download and install a second
`esdiag-local` executable before they can provision a local output stack. The
standalone launcher then pulls an ESDiag container even when the matching native
binary is already present.

## What Changes

- Add `esdiag local <command>` as a binary-owned interface that dispatches the
  version-matched embedded local-stack launcher without a persistent script
  installation.
- Add `--stack=auto|full|core` lifecycle selection. New auto deployments use a
  compatible native binary when available; `full` forces the existing ESDiag
  container service and `core` provisions only Elasticsearch and Kibana.
- Persist a resolved stack mode per managed state directory so later PATH
  changes do not silently change a deployment's ownership model.
- Define shared stack-state and lifecycle compatibility for `esdiag-local` and
  `esdiag local`, while keeping ESDiag user configuration and keystore state
  owned by the selected runtime in phase one. Mode changes must not migrate
  configuration implicitly.
- Let interactive initialization offer to start an embedded core stack when the
  user selects local processing and no usable local deployment exists.
- Keep the downloaded standalone `esdiag-local` artifact as a script-first
  distribution path, including its existing self-update behavior.

## Capabilities

### New Capabilities

- `embedded-local-stack-launcher`: Binary-owned local-stack command dispatch,
  compatibility detection, core/full modes, and core-stack onboarding handoff.

### Modified Capabilities

- `standalone-local-stack`: Define mode-aware standalone lifecycle behavior,
  including native-binary detection and core deployments that omit ESDiag
  containers.

## Impact

- Affects the Rust CLI command surface, embedded build assets, and native
  process dispatch.
- Affects `bin/esdiag-local`, its standalone tests, release behavior, and
  local-stack documentation.
- Changes `esdiag init` to permit the binary-owned embedded launcher as a
  narrowly scoped exception to its current no-external-helper rule.
