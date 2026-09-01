# Security Policy

## Supported versions

CONTAINER is currently in alpha. Security fixes are applied to the latest release only.

## Reporting a vulnerability

Please do not publish sensitive security details in a public issue. Use GitHub's private vulnerability reporting feature after the repository is published. Include:

- affected CONTAINER version;
- Windows version;
- clear reproduction steps;
- expected and observed behavior;
- whether a crafted media file is required.

Do not include private media files, access tokens or personal paths in the report.

## Scope

Relevant issues include command injection, unsafe path handling, source-file overwrites, untrusted media parsing and dependency vulnerabilities. Normal FFmpeg encoding errors and unsupported codecs should use the bug report template instead.
