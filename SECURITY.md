# Security

Cookie Monster is local-only. Decisions, logs, and keys stay on-device.

## Report a vulnerability

Do **not** open a public issue for anything that could harm users’ machines
or expose their cookie metadata.

Contact [@ojs393](https://github.com/ojs393) privately with:

- What you found
- Steps to reproduce
- Affected version / commit if known
- Whether you plan to disclose and preferred timeline

## Design expectations (not a guarantee)

- UI IPC and the system proxy default to **loopback only** (`127.0.0.1`)
- Desktop ↔ host auth uses a per-run token file with owner-only permissions
- Cookie **values** are redacted before encrypt-at-rest (AES-GCM)
- Store key / CA key / policy files use owner-only modes on Unix
- No accounts, no telemetry, no cloud upload path in this repo

System shield (Local CA + MITM) is powerful: only enable it if you
understand you are trusting a local CA on that machine. Cert-pinned
apps may bypass the proxy; that is expected.

## Supported versions

Security fixes target the latest public `main` release of this repository.
