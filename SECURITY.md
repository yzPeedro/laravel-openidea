# Security policy

## Supported versions

Only the latest release receives fixes.

## Reporting a vulnerability

Please don't open a public issue. Use [GitHub's private vulnerability reporting](https://github.com/yzPeedro/laravel-openidea/security/advisories/new) and include:

- what is affected (plugin, recorder, installer or MCP server) and the version;
- how to reproduce it;
- the impact you expect.

You will get an answer within a week. Once a fix is released, you are credited in the advisory unless you prefer otherwise.

## Design notes

- The plugin is a sandboxed WebAssembly component. It reads project files and runs `php` or `docker` only with the permissions the user grants.
- The Live Audit recorder runs only in local environments, masks secrets and personal data, and writes only to `storage/`.
- The MCP server runs locally over stdio and masks secrets in configuration values. Its `run_tinker` tool executes code in the local application by design.
