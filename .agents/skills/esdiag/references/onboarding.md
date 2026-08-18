# First-run ESDiag setup

ESDiag initialization is a local, interactive workflow. Ask the human to run:

```sh
esdiag init
```

It securely configures the diagnostic user, encrypted keystore, linked Elasticsearch and Kibana output deployment, collection host, and default saved job. It may also offer to install the embedded ESDiag Agent Skill.

Do not request passwords, API keys, or keystore values in an agent conversation. Do not write `esdiag.yml`, `hosts.yml`, `secrets.yml`, or `jobs.yml` manually.

If initialization is complete but a coding agent needs the skill, use the standalone offline installer:

```sh
esdiag agent skills
```

Specify a target when automatic detection is insufficient:

```sh
esdiag agent skills --target claude
esdiag agent skills --target codex
esdiag agent skills --target opencode
```

The installer protects locally modified or unrecognized ESDiag skill directories. A human may explicitly decide to replace one with `--force`. Running coding agents can require restart or reload before the skill appears.
