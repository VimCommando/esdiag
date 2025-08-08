#!/bin/bash
set -e

# Builds the multi-architecture Docker image for the Elastic Stack Diagnostics,
# tagging it with the current version defined in Cargo.toml
#
# Requires Docker registry pre-authentication with:
# > gcloud auth configure-docker us-west1-docker.pkg.dev

if [[ ! -f Cargo.toml ]]; then
    echo "Cargo.toml not found, run from repository root"
    exit 1
fi

declare version=$(grep '^version =' Cargo.toml | sed 's/version = "\(.*\)"/\1/')

docker buildx build . \
    --platform linux/amd64,linux/arm64 \
    --tag "us-west1-docker.pkg.dev/elastic-ce-tools/esdiag/esdiag:latest" \
    --tag "us-west1-docker.pkg.dev/elastic-ce-tools/esdiag/esdiag:${version}" \
    --push
