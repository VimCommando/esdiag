# crates.io Release Target

Use this reference when validating or publishing the `esdiag` Cargo crate.

## Package Validation

Run these from the clean release branch after setting `VERSION`:

```bash
cargo package --locked
cargo publish --dry-run --locked
```

These commands validate the actual registry package and metadata. Resolve any
generated-file changes, including `NOTICE.txt`, before committing the release.

## Publication

Require explicit human approval before the actual publication. Read the
crate-specific token without printing or committing it:

```bash
CARGO_REGISTRY_TOKEN="$(<"$HOME/.config/apikey/esdiag.crates.io")" \
  cargo publish --locked
cargo info "$CRATE@$VERSION"
```

Treat an accepted version as immutable. If publication partially succeeds or a
published package needs correction, cut a new patch release; do not retry with
changed contents or attempt to replace the version.
