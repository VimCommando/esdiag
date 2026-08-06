## 1. Source Catalog and Shared Version Rules

- [ ] 1.1 Add the `lite` tag to all fifteen Elasticsearch source entries currently collected by `bin/min-diag.sh`, preserving existing tags and source behavior.
- [ ] 1.2 Refactor the existing NPM-style source requirement parsing into reusable Rust logic for both runtime source resolution and generation, with tests for supported inclusive, exclusive, and bounded comparator forms.
- [ ] 1.3 Add tests that assert the exact expected `lite` source membership and validate each tagged source's output path and version rules.

## 2. Handwritten Bash 3.2 Runtime

- [ ] 2.1 Create `bin/esdiag-lite.sh` from the existing helper behavior with generated-region markers, a source-safe main guard, and no use of Bash features newer than 3.2.
- [ ] 2.2 Implement numeric major/minor/patch parsing and inclusive/exclusive version predicate helpers, including prerelease normalization and double-digit minor-version coverage.
- [ ] 2.3 Fetch and save the Elasticsearch root response first, extract and strictly validate `version.number` without `jq`, and abort before versioned requests when bootstrap fails.
- [ ] 2.4 Write `diagnostic_manifest.json` from the validated in-memory version, preserving minimum-mode bundle compatibility and setting the runner to `esdiag-lite`.
- [ ] 2.5 Implement Bash 3.2-compatible parsing for `--archive=zip|none`, default it to `zip`, reject unknown formats, and document the option in script help.
- [ ] 2.6 Check for `zip` before collection only when ZIP mode is selected and emit exactly `No zip executable found, run with --archive=none to skip archive creation` when it is unavailable.
- [ ] 2.7 Create a root-layout `api-diagnostics-<timestamp>.zip` after successful collection, remove the source directory only after archive success, and preserve the directory if `zip` fails.
- [ ] 2.8 Replace embedded URL and API-key configuration with validation for `ELASTIC_ES_URL`, `ELASTIC_ES_API_KEY`, `ELASTIC_ES_USERNAME`, and `ELASTIC_ES_PASSWORD`, requiring either an API key or complete username/password pair.
- [ ] 2.9 Implement shared API-key and HTTP basic authentication request construction with API-key precedence, including the bootstrap request, without logging or persisting credential values.
- [ ] 2.10 Preserve the `collect`, `watch`, help, interval configuration, logging, and directory layout interfaces while documenting the environment variables, labeling the utility as collection-only, and removing the `jq` dependency.

## 3. Generated API Functions

- [ ] 3.1 Add a repository-side Rust generation target that selects `lite` Elasticsearch sources and deterministically renders one `get_api_<source>` function per source plus the complete collection invocation function.
- [ ] 3.2 Generate request branches from each source's version rules and output configuration, delegating HTTP operations to the handwritten `get_api` function and unsupported versions to `skip_api`.
- [ ] 3.3 Treat the tagged `version` source as the pre-resolution bootstrap request and ensure all other generated API functions execute only after successful version detection.
- [ ] 3.4 Add generator validation that rejects unsupported semver expressions, invalid Bash identifiers, ambiguous generated behavior, and missing generated-region markers with actionable errors.
- [ ] 3.5 Add generator write and `--check` modes, regenerate `bin/esdiag-lite.sh`, and add an automated drift check for the checked-in generated region.

## 4. Compatibility and Behavior Tests

- [ ] 4.1 Add shell tests for Bash 3.2-compatible version parsing and predicates at the relevant Elasticsearch boundaries, including `7.7.0`, `7.10.0`, prerelease versions, and malformed input.
- [ ] 4.2 Add generated-function tests that stub the shared request function and compare selected paths and skipped APIs with Rust `sources.yml` resolution across representative boundary versions.
- [ ] 4.3 Add collection tests using a mock Elasticsearch endpoint to verify environment URL handling, API-key authentication, basic authentication, API-key precedence, incomplete-auth rejection, credential redaction, bootstrap ordering, version-specific requests, unsupported API skips, output paths, and jq-free execution.
- [ ] 4.4 Test default and explicit ZIP output, uncompressed output, unknown archive formats, archive failure directory preservation, and the exact missing-`zip` error message.
- [ ] 4.5 Verify completed ESDiag Lite ZIP and directory outputs contain valid manifests and can be processed by ESDiag when version-unsupported files are absent.
- [ ] 4.6 Run `bash -n bin/esdiag-lite.sh` and `shellcheck bin/esdiag-lite.sh`, resolving all syntax and lint findings across handwritten and generated code.
- [ ] 4.7 Run `shfmt -d -i 2 -ci -bn bin/esdiag-lite.sh` and adjust both the handwritten script and generator output until it reports no diff.
- [ ] 4.8 Execute behavior tests with Bash 3.2 where available.

## 5. Rename, Documentation, and Release Notes

- [ ] 5.1 Remove `bin/min-diag.sh` after `bin/esdiag-lite.sh` reaches behavior parity and update `bin/readme.md` and all repository references to the new name.
- [ ] 5.2 Replace `docs/bin/min-diag.md` with ESDiag Lite documentation that prominently labels it as collection-only and covers requirements, `ELASTIC_ES_*` environment configuration, authentication precedence, `--archive=zip|none`, version-aware skips, `collect`, `watch`, generation, and the path migration.
- [ ] 5.3 Document how ZIP and directory outputs are passed to `esdiag process`, clearly assigning collection to `esdiag-lite.sh` and processing, analysis, and export to `esdiag`.
- [ ] 5.4 Add a Keep a Changelog entry describing the `esdiag-lite.sh` replacement, collection-only scope, environment-based authentication, Elasticsearch version awareness, optional ZIP output, and removal of the `jq` dependency.

## 6. Final Verification

- [ ] 6.1 Run the generator in `--check` mode and confirm the checked-in script is current.
- [ ] 6.2 Run `cargo fmt`, `cargo clippy`, and `cargo test` and resolve all failures related to the change.
- [ ] 6.3 Review the final diff to confirm `zip` is required only for ZIP mode and no runtime dependency on `jq`, `yq`, Python, Rust, the ESDiag binary, or a container was introduced into `bin/esdiag-lite.sh`.
- [ ] 6.4 Review `bin/esdiag-lite.sh` and its tests to confirm connection or credential values are never embedded, logged, or written to diagnostic output.
- [ ] 6.5 Re-run Bash syntax validation, ShellCheck, and `shfmt -d -i 2 -ci -bn` against the final generated script.
