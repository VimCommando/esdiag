---
type: Guide
title: esdiag-lite.sh
description: Guide to collecting portable, version-aware Elasticsearch diagnostic bundles with ESDiag Lite.
tags: [bin, collection, diagnostics]
---

esdiag-lite.sh
--------------

`bin/esdiag-lite.sh` is a collection-only Elasticsearch diagnostic utility for
restricted environments where the ESDiag binary or container cannot be
deployed. It collects raw API responses and a diagnostic manifest; it does not
process, analyze, transform, export, send, upload, or visualize diagnostics.

The script requires Bash 3.2 or newer, `curl`, and standard POSIX utilities.
The default ZIP output also requires `zip`. It does not require `jq`, `yq`,
Python, Rust, the ESDiag binary, or a container runtime.

Configuration and authentication
--------------------------------

Set the Elasticsearch endpoint and one supported authentication mode through
the environment:

```bash
export ELASTIC_ES_URL='https://elasticsearch.example:9200'
export ELASTIC_ES_API_KEY='encoded-api-key'
```

Alternatively, configure HTTP basic authentication:

```bash
export ELASTIC_ES_URL='https://elasticsearch.example:9200'
export ELASTIC_ES_USERNAME='diagnostic-reader'
export ELASTIC_ES_PASSWORD='password'
```

`ELASTIC_ES_URL` is required. When `ELASTIC_ES_API_KEY` is non-empty, it takes
precedence over `ELASTIC_ES_USERNAME` and `ELASTIC_ES_PASSWORD`; the basic
authentication values are ignored. Without an API key, both username and
password are required. The script never prints or writes credential values.

Collect diagnostics
-------------------

Collect one bundle with the default ZIP archive:

```bash
bin/esdiag-lite.sh collect
```

This creates `api-diagnostics-<timestamp>.zip`, with the diagnostic files at
the archive root. The source directory is removed only after ZIP creation
succeeds. If `zip` is unavailable, the script exits before collection with:

```text
No zip executable found, run with --archive=none to skip archive creation
```

Use an uncompressed directory when ZIP output is not available:

```bash
bin/esdiag-lite.sh collect --archive=none
```

Only `zip` and `none` are accepted archive formats. `none` skips the `zip`
dependency check and retains `api-diagnostics-<timestamp>`.

Collect periodically with the same interval controls as the previous helper:

```bash
WAIT_SECONDS=60 COLLECTION_COUNT=5 bin/esdiag-lite.sh watch --archive=none
```

Version-aware APIs
------------------

The script fetches `version.json` first, validates `version.number`, then
selects the supported Elasticsearch API path for that version. APIs unavailable
on the target version are logged as skipped and do not stop collection.

The generated API functions are derived from the `lite`-tagged entries in
`assets/elasticsearch/sources.yml`. Maintainers can refresh the checked-in
generated region or verify it has not drifted with:

```bash
cargo run --bin esdiag-lite-generate
cargo run --bin esdiag-lite-generate --check
```

Process collected output with ESDiag
------------------------------------

Pass the ZIP archive or uncompressed directory to `esdiag process` on a system
where ESDiag is available. For example, with `localhost` configured as a saved
output host:

```bash
esdiag process api-diagnostics-<timestamp>.zip localhost
esdiag process api-diagnostics-<timestamp> localhost
```

`esdiag-lite.sh` owns collection. ESDiag owns processing, analysis, and export.

Migration from min-diag.sh
--------------------------

`bin/min-diag.sh` was removed. Update copied scripts, documentation, and
automation to use `bin/esdiag-lite.sh` and environment configuration instead of
editing `URL` or `APIKEY` in the helper itself.
