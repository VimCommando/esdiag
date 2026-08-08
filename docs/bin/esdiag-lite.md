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
process, analyze, transform, export, or visualize diagnostics. It can
optionally forward a generated ZIP archive to Elastic Upload Service.

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

Optional upload configuration
-----------------------------

Uploads use the Elastic Upload Service multipart API. `UPLOAD_HOST` defaults to
`https://upload.elastic.co`; set `UPLOAD_ID` when a later standalone `upload`
command should not need its optional id argument:

```bash
export UPLOAD_ID='a4f434d4-1fb5-440d-a8ec-bbb4fc522210'
# Only needed for a non-default upload service.
export UPLOAD_HOST='https://upload.elastic.co'
```

Uploads require one SHA-256 command: `shasum`, `sha256sum`, or `openssl`.
The script otherwise uses standard POSIX utilities to calculate digests, create
50 MB parts, skip parts the service already has, and finalize the upload.
Re-running a failed upload is safe because existing parts are skipped.

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

Upload diagnostics
------------------

Upload immediately after collection with an upload id. The generated ZIP path
is used automatically and remains on disk after upload:

```bash
bin/esdiag-lite.sh collect --upload=a4f434d4-1fb5-440d-a8ec-bbb4fc522210
```

Collection uploads are opt-in: setting `UPLOAD_ID` alone does not upload a
collection; pass `--upload=<id>`.

Immediate uploads require the default `zip` archive format; they cannot be
combined with `--archive=none`.

Upload an existing archive separately. The id argument overrides `UPLOAD_ID`;
when it is omitted, `UPLOAD_ID` must be set:

```bash
bin/esdiag-lite.sh upload api-diagnostics-<timestamp>.zip
bin/esdiag-lite.sh upload api-diagnostics-<timestamp>.zip a4f434d4-1fb5-440d-a8ec-bbb4fc522210
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

`esdiag-lite.sh` owns collection and optional archive forwarding. ESDiag owns
processing, analysis, and export.

Migration from min-diag.sh
--------------------------

`bin/min-diag.sh` was removed. Update copied scripts, documentation, and
automation to use `bin/esdiag-lite.sh` and environment configuration instead of
editing `URL` or `APIKEY` in the helper itself.
