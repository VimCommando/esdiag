# First-run ESDiag setup

Initialization is local and interactive. Check whether `esdiag` is available:

```sh
command -v esdiag
```

## Native binary

When the binary is available, ask the human to run:

```sh
esdiag init
```

The wizard configures the workflow they select. For local processing it can
start `esdiag local up --stack=core`; for a remote deployment it configures the
selected output.

When `esdiag` is unavailable, check for Homebrew and Cargo:

```sh
command -v brew
command -v cargo
```

If either is available, ask whether the human wants a local binary. If both are
available, ask which installer they prefer. After consent, install with the
chosen method:

```sh
brew install elastic/tools/esdiag
```

```sh
cargo install --locked esdiag
```

Confirm `esdiag --version`, then direct the human to run `esdiag init` in their
terminal. Do not run the initializer for them.

## Script-first container stack

When neither installer is available, ask whether the human wants a
containerized ESDiag stack. On approval, download and verify `esdiag-local`,
then start the full stack:

```sh
mkdir -p "$HOME/.local/bin"
curl -fsSL https://github.com/elastic/esdiag/releases/latest/download/esdiag-local \
  -o "$HOME/.local/bin/esdiag-local"
curl -fsSL https://github.com/elastic/esdiag/releases/latest/download/esdiag-local.sha256 \
  -o "$HOME/.local/bin/esdiag-local.sha256"
chmod 755 "$HOME/.local/bin/esdiag-local"
(
  cd "$HOME/.local/bin"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum --check esdiag-local.sha256
  else
    shasum -a 256 -c esdiag-local.sha256
  fi
)
"$HOME/.local/bin/esdiag-local" up --stack=full
```

This path needs Podman or Docker with Compose support. It provides the web UI
and containerized ESDiag runtime; `esdiag init`, native commands, and core mode
need a native binary.

## Guardrails

Keep credentials in the interactive wizard. Do not request passwords, API keys,
or keystore values in chat, and do not write ESDiag state files manually.

For a shared hosted `esdiag serve` service, use the administrator-provided URL.
Install local tools only when the human also wants a local workflow.
