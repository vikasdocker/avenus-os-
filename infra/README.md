# Infrastructure

The infrastructure folder records operational configuration for development and release
systems that are outside the boot image.

Phase 0.4 infrastructure includes:

- GitHub Actions workflows under `.github/workflows`
- Docker development environment under `docker/`
- Devcontainer configuration under `.devcontainer/`
- Release package script under `scripts/release/package.sh`

Future production infrastructure must preserve local-first device operation, signed
release provenance, tenant isolation, and auditability.

