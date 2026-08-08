# GitHub Release Target

Use this reference when preparing the release branch, notes, numeric tag, or
GitHub draft release.

## Preflight And Branch

Require a clean worktree and inspect commits, merged PRs, existing releases,
tags, `CHANGELOG.md`, and the previous release branch. Check ancestry:

```bash
git merge-base --is-ancestor "$PREVIOUS" upstream/main
```

A non-ancestor previous tag is valid, but GitHub cannot infer the note range
from topology. Create `BRANCH` from the exact verified `upstream/main` commit:

```bash
git switch -c "$BRANCH" upstream/main
git push --set-upstream upstream "$BRANCH"
```

Use a PR for branch-protected changes. Do not bypass protections without
explicit authority.

## Release Notes

Build curated notes from verified changes after `PREVIOUS`. Prioritize
user-visible features, compatibility changes, operations, and important fixes.
End with a link to the release branch `CHANGELOG.md`.

In `.github/workflows/release-esdiag-local.yml`:

- Trigger numeric tags with `"[0-9]*.[0-9]*.[0-9]*"`.
- Create the release with `--notes-start-tag "$PREVIOUS"`.
- Keep the release draft after verification.
- Never automate `gh release edit ... --draft=false`.

## Tag And Draft Workflow

Tag the final pushed release-branch commit, then verify the remote annotated tag
dereferences to that tip:

```bash
git tag -a "$TAG" -m "Release $VERSION"
git push upstream "$TAG"
```

Watch `Release esdiag-local`. It must create or reuse a draft, verify the
full-version container manifests, render and validate `esdiag-local`, upload it
with `esdiag-local.sha256`, download both assets, verify the checksum, and leave
the release draft.

Verify remotely:

- `tagName` is `TAG` and `isDraft` is true.
- Curated notes cover only `PREVIOUS...TAG`.
- `esdiag-local` and `esdiag-local.sha256` are uploaded.
- All required Homebrew assets are uploaded before publication.

## Publication And Recovery

Stop for a human review of the tag, notes, images, standalone script, native
archives, checksums, and crates.io result. Only a human publishes the GitHub
release.

- **Wrong note range:** Replace the draft body and fix `--notes-start-tag`.
- **Draft workflow failure:** Fix the release branch and rerun validation.
- **Unpublished tag on the wrong commit:** Move it only with explicit approval.
- **Published tag or release is wrong:** Do not move or delete it; request a
  release-management decision.
- **Workflow update rejected:** Confirm GitHub CLI authentication includes the
  `workflow` scope without exposing a token.
