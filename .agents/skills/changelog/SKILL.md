---
name: changelog
description: Maintain `CHANGELOG.md` entries using Keep a Changelog 1.1.0 conventions. Use when adding or reviewing changelog entries, preparing unreleased notes for a PR, or checking whether changelog bullets are correctly scoped and referenced during PR review workflows.
---

# Changelog Management

Use this skill when working on `CHANGELOG.md`, release notes, or PRs that should
update changelog content.

## Standard

- Follow [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/).
- Prefer `Added`, `Changed`, `Deprecated`, `Removed`, `Fixed`, and `Security`.
- Keep entries user-facing. Describe behavior and outcomes, not internal refactors
  unless users/operators would care.
- Write concise bullets with one feature, fix, or change per bullet.

## Project Rules

- Changelog issue references must point only to the public `elastic/esdiag`
  repository, using `https://github.com/elastic/esdiag/issues/<number>` or a
  verified public issue number such as `#123`.
- `elastic/esdiag-dev` is private. Never include its issue links, qualified
  references, or bare issue numbers in the changelog. Do not transfer a private
  issue number to a public URL; the repositories have separate issue numbering.
- If no corresponding public `elastic/esdiag` issue is verified, keep the
  changelog bullet without a reference. Do not substitute a PR link or number.
- Only include a `Fixed` bullet if the change clearly closed or resolved a GitHub
  issue, or if release notes explicitly frame it as a bug fix.

## Entry Writing Rules

- `Added`: one bullet per feature, not one bullet per PR.
- `Changed`: use for behavior changes, UX changes, packaging/runtime changes, and
  operator-facing refactors.
- `Fixed`: reserve for bug fixes with verified issue/release-note support.
- `Removed`: use for total feature removal, replaced features go into `changed`.
- `Security`: use for security-focused updates and CVE patches.
- Split combined release-note prose into separate bullets when it actually covers
  multiple distinct features.
- Avoid vague bullets like "misc fixes", "multiple features", or "cleanup".
- Keep tense consistent inside a section.

## Reference Workflow

When updating changelog content:

1. Read the current `CHANGELOG.md`.
2. Identify the target section:
   - `Unreleased` for upcoming work on the active branch.
   - the in-scope release section when preparing a release.
3. Gather references in this order:
   - GitHub release notes
   - linked issues
   - linked PRs
   - branch/tag commit history
4. For each candidate bullet:
   - choose the correct section (`Added`, `Changed`, `Fixed`, etc.)
   - reduce it to a single user-facing item
   - attach a reference only if the corresponding public `elastic/esdiag` issue is verified
   - otherwise omit the reference
5. Remove or rewrite bullets that cannot be supported by the sources.

## GitHub Verification Guidance

- Use release notes, PRs, and commit history to find the corresponding public
  `elastic/esdiag` issue. Verify the repository as well as the issue number.
- A PR's `Resolves #123` refers to its own repository unless explicitly qualified;
  it does not establish that public `elastic/esdiag#123` is the same issue.
- Private issues may substantiate a fix internally, but must not appear as
  changelog references. A verified private fix can have an unreferenced `Fixed` bullet.

## PR Review Use

During PR review workflows, check changelog updates for:

- correct Keep a Changelog structure
- one feature per `Added` bullet
- references restricted to verified public `elastic/esdiag` issues, with no private references
- `Fixed` bullets limited to verified fixes
- no invented versions or unsupported claims
- accurate `Unreleased` scope for the branch being reviewed

If the PR is missing a changelog update for user-visible behavior, suggest the
smallest accurate entry rather than a long release-summary paragraph.

## Output Pattern

Use this shape when drafting or revising entries:

```markdown
## [Unreleased]

### Added

- Added feature name or user-visible capability (#123).

### Changed

- Changed operator-visible or runtime behavior (#124).

### Fixed

- Fixed the user-visible bug outcome (#125).
```
