# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

The Aegis-Assets team takes security seriously. If you discover a security vulnerability, please report it responsibly.

### How to Report

**Please do NOT report security vulnerabilities through public GitHub issues.**

Instead, please send an email to: **security@aegis-assets.org**

Include the following information:
- Type of issue (e.g. buffer overflow, SQL injection, cross-site scripting, etc.)
- Full paths of source file(s) related to the manifestation of the issue
- The location of the affected source code (tag/branch/commit or direct URL)
- Any special configuration required to reproduce the issue
- Step-by-step instructions to reproduce the issue
- Proof-of-concept or exploit code (if possible)
- Impact of the issue, including how an attacker might exploit the issue

### Response Timeline

- **Initial response**: Within 48 hours
- **Status update**: Within 7 days  
- **Resolution target**: Within 30 days for critical issues, 90 days for others

### Security Considerations in Aegis-Assets

Given our compliance-first approach, we take special care with:

1. **Data Handling**: No copyrighted content is stored or transmitted
2. **File Processing**: Robust parsing with bounds checking for all game formats
3. **Compliance**: Built-in protections against high-risk extractions
4. **Dependencies**: Regular security audits of all dependencies via `cargo audit`

### Disclosure Policy

Once a security issue is resolved, we will:

1. **Coordinate disclosure** with the reporter
2. **Publish security advisory** on GitHub
3. **Release patched version** with security fixes
4. **Credit the reporter** (if desired)

### Safe Harbor

We support responsible security research and will not pursue legal action against security researchers who:

- Follow this responsible disclosure policy
- Don't access data they don't own or without permission
- Don't disrupt our services
- Don't harm our users

Thank you for helping keep Aegis-Assets and our community safe!
