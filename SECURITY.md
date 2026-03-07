# Security Policy

## Supported Versions

| Version | Supported |
|---------|-----------|
| 0.1.x   | Yes       |

Only the latest release receives security updates.

## Reporting a Vulnerability

If you discover a security vulnerability in Velos, please report it responsibly.

**Do NOT open a public GitHub issue for security vulnerabilities.**

### How to Report

1. **GitHub Security Advisories** (preferred): Use the [Security tab](https://github.com/Dave93/velos/security/advisories/new) to create a private advisory.
2. **Email**: Send details to the maintainer via GitHub profile contact.

### What to Include

- Description of the vulnerability
- Steps to reproduce
- Affected versions
- Potential impact
- Suggested fix (if any)

### Response Timeline

- **Acknowledgment**: Within 48 hours
- **Initial assessment**: Within 7 days
- **Fix or mitigation**: Within 90 days (depending on severity)

### Scope

The following components are in scope for security reports:

- **Velos daemon** (process isolation, privilege handling)
- **IPC protocol** (Unix socket communication)
- **MCP server** (JSON-RPC over stdio and HTTP)
- **AI agent sandboxing** (path restrictions, command execution)
- **REST API and WebSocket** (authentication, input validation)
- **Installer script** (download integrity, checksum verification)

### Out of Scope

- Vulnerabilities in third-party dependencies (report upstream)
- Issues requiring physical access to the machine
- Social engineering attacks

## Security Design

Velos implements several security measures:

- **AI agent sandboxing**: File operations are restricted to the project directory
- **Checksum verification**: Install script verifies SHA-256 checksums
- **No network by default**: Daemon communicates only via local Unix socket
- **Minimal privileges**: Daemon runs as the current user, no root required
