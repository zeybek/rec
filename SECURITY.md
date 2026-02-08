# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

If you discover a security vulnerability in `rec`, please report it responsibly.

**Do NOT open a public GitHub issue for security vulnerabilities.**

Instead, please report vulnerabilities via one of these methods:

1. **GitHub Security Advisories** (preferred): Use the [Report a vulnerability](https://github.com/zeybek/rec/security/advisories/new) button on GitHub.

2. **Email**: Send details to the maintainers via the email listed in the GitHub profile of [@zeybek](https://github.com/zeybek).

### What to include

- Description of the vulnerability
- Steps to reproduce
- Potential impact
- Suggested fix (if any)

### Response timeline

- **Acknowledgment**: Within 48 hours
- **Assessment**: Within 1 week
- **Fix release**: Within 2 weeks for critical issues

### Scope

The following are in scope for security reports:

- **Command injection** via replay, import, or export
- **Path traversal** in session file operations
- **Arbitrary code execution** through crafted session files
- **Privilege escalation** through shell hooks
- **Supply chain** issues in binary distribution

### Out of scope

- Vulnerabilities in dependencies (report upstream)
- Issues requiring physical access to the machine
- Social engineering attacks
