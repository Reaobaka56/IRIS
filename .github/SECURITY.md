# Security Policy

## Supported Versions

| Version | Supported          |
|---------|--------------------|
| 0.2.x   | :white_check_mark: |
| < 0.2   | :x:                |

## Reporting a Vulnerability

If you discover a security vulnerability in IRIS, please report it responsibly.

**Do NOT open a public issue for security vulnerabilities.**

Instead, please email or contact **[moon9t on GitHub](https://github.com/moon9t)** directly with:

1. A description of the vulnerability
2. Steps to reproduce
3. Potential impact
4. Suggested fix (if any)

You can expect an initial response within 48 hours. We will work with you to understand and address the issue before any public disclosure.

## Scope

The following are in scope for security reports:

- The IRIS compiler and interpreter (`src/`)
- The standard library (`stdlib/`)
- The native compilation pipeline (LLVM codegen)
- The VS Code extension (`vscode-iris/`)
- FFI and networking builtins

The following are out of scope:

- Example programs in `examples/` (these are educational, not production code)
- Third-party dependencies (report those to their respective maintainers)
