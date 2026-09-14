# crates.io Release Target

Use this reference when validating or publishing either workspace crate:
`esdiag` or `elasticrc`.

## Independent Package Selection

Set `CRATE` to the package being released.

```bash
CRATE=elasticrc
VERSION=0.1.0
```

The `elasticrc` version is independent from the ESDiag application version.
A standalone `elasticrc` release does not publish `esdiag` or change its
version-sensitive release files.

Always pass the selected package to workspace Cargo commands. Do not publish
the workspace without `--package`.

## Package Validation

Run these from the clean release branch after setting `VERSION`:

```bash
cargo package --locked --package "$CRATE"
cargo publish --dry-run --locked --package "$CRATE"
```

These commands validate the actual registry package and metadata. Resolve any
generated-file changes, including `NOTICE.txt`, before committing the release.

## Publication

Require explicit human approval before the actual publication. Read the
crate-specific token without printing or committing it:

```bash
CARGO_REGISTRY_TOKEN="$(<"$HOME/.config/apikey/esdiag.crates.io")" \
  cargo publish --locked --package "$CRATE"
cargo info "$CRATE@$VERSION"
```

Treat an accepted version as immutable. If publication partially succeeds or a
published package needs correction, cut a new patch release; do not retry with
changed contents or attempt to replace the version.
