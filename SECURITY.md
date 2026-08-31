# Security policy

## Supported version

Security fixes target the latest released version and the active development branch. The latest release is currently `0.2.0`; development work lives on `dev/0.3-search` until a later release passes its gates.

## Reporting a vulnerability

Use GitHub's private vulnerability-reporting channel when it is enabled for the repository. If private reporting is unavailable, contact the repository owner through the GitHub profile without posting exploit details, malicious inputs, credentials, or private artifacts in a public issue.

Include the affected commit/tag, platform, build command, minimal input or UCI transcript, observed impact, and whether the issue reproduces under a clean release build. Hash any attached binary or artifact.

Relevant security reports include memory-safety or concurrency failures, crafted FEN/UCI denial of service, unbounded resource consumption, command execution through build/test tooling, artifact-integrity bypasses, and credential exposure. Playing-strength regressions, ordinary engine crashes without an attacker-controlled input, and incorrect chess decisions belong in the engine-regression issue form unless they cross a security boundary.

Do not run untrusted engine binaries, networks, opening books, PGNs, or match scripts outside an isolated account or worker. Distributed workers should verify every registered source, executable, book, and result identity before accepting or uploading work.
