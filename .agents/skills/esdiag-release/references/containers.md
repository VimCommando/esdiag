# Container Release Target

Use this reference when publishing ESDiag images.

## Required Tags

Build one `linux/amd64,linux/arm64` OCI index and publish the same content under
all six tags:

```text
docker.elastic.co/esdiag/esdiag:SERIES
docker.elastic.co/esdiag/esdiag:VERSION
docker.elastic.co/esdiag/esdiag:latest
us-west1-docker.pkg.dev/elastic-ce-tools/esdiag/esdiag:SERIES
us-west1-docker.pkg.dev/elastic-ce-tools/esdiag/esdiag:VERSION
us-west1-docker.pkg.dev/elastic-ce-tools/esdiag/esdiag:latest
```

Prefer one `docker buildx build --push` with all tags. When `SERIES` and
`latest` already point to the required image, create only the `VERSION` aliases
with `docker buildx imagetools create`; do not rebuild.

## Verification

Inspect every remote tag. Require:

- One identical OCI index digest across registries and aliases.
- Both `linux/amd64` and `linux/arm64` manifests.
- A runtime check on each platform reporting `esdiag VERSION`.

Publish `docker.elastic.co/esdiag/esdiag:VERSION` before creating `TAG`: the
draft workflow and Homebrew Linux asset process depend on it.
