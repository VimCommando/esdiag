---
name: esdiag-release
description: Prepare, stage, verify, and publish ESDiag numbered releases. Use when cutting a maintenance branch, setting a stable version, drafting notes, publishing containers or the crates.io crate, creating a numeric tag, attaching native Homebrew assets, updating the elastic/tools Formula, or validating a draft GitHub release.
---

# ESDiag Release

Coordinate a numbered release through a verified GitHub draft. Keep each
distribution target independent and load its reference before acting on it.

## Release Variables

Establish these values before changing state:

```text
VERSION=0.16.0
SERIES=0.16
PREVIOUS=0.15.0
BRANCH=0.16
TAG=0.16.0
CRATE=esdiag
HOMEBREW_TAP=elastic/tools
HOMEBREW_REPO=$HOME/Development/elastic/homebrew-tools
```

ESDiag uses numeric tags without a leading `v`. Keep `VERSION`, the Cargo
package version, `bin/esdiag-local`, the full image tag, and `TAG` aligned.

## Target References

Read only the target references required by the release:

- [GitHub release](references/github.md): branch, curated notes, numeric tag,
  draft workflow, publication gate, and GitHub recovery.
- [Containers](references/containers.md): multi-architecture image build,
  registry aliases, manifest inspection, and runtime verification.
- [crates.io](references/crates-io.md): package validation, dry-run, approved
  publication, and immutable-version recovery.
- [Homebrew](references/homebrew.md): native assets, checksum contract,
  draft-release upload, Formula update, and tap PR.

## Shared Flow

1. Require clean ESDiag and target worktrees. Preserve unrelated user changes.
2. Fetch `upstream` and `origin` with pruning. Fast-forward local `main` to
   `upstream/main`; push `origin/main` when the fork is behind.
3. Set the stable version on `BRANCH`, update all version-sensitive files and
   `NOTICE.txt`, then run the baseline validation:

   ```bash
   cargo fmt --all -- --check
   cargo check
   cargo test
   shellcheck bin/esdiag-local
   bash tests/esdiag-local.sh
   bash tests/bin/esdiag-control.sh
   git diff --check
   ```

4. Commit and push all release-branch fixes before making a tag.
5. Complete the GitHub workflow through the verified draft. Publish container
   manifests before creating `TAG` when the release includes images.
6. Attach every Homebrew release asset to the draft before the GitHub release
   becomes public. Never change those assets after publication.
7. Stop for explicit approval before publishing crates.io or changing a GitHub
   release from draft to public. A human publishes the GitHub release.
8. Update the Homebrew Formula only after the GitHub release is public, stable,
   complete, and immutable.

## Shared Invariants

- Use a new patch version to correct a published crate, tag, release, or
  Homebrew asset. Do not overwrite immutable public artifacts.
- Keep release notes scoped to `PREVIOUS...TAG`, even when release-branch
  topology prevents GitHub from choosing the correct range automatically.
- Treat the release as incomplete until every selected target has passed its
  target-specific verification.

## Command Conventions

Follow repository `AGENTS.md`: use `rtk` for supported commands and pipe GitHub
JSON/API output through `toon -s`. Use authenticated HTTPS if SSH is unavailable.
Keep local `BRANCH`, `upstream/BRANCH`, and the dereferenced `TAG` commit aligned
before handoff.
