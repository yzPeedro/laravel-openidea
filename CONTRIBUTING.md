# Contributing to Laravel OpenIDEA

Thanks for helping. This guide covers the setup, the checks every change must pass, and the conventions the code follows.

## Ground rules

- Open an issue before a large change, so we can agree on the approach first.
- One topic per pull request. Small, focused PRs are reviewed faster.
- Every behaviour change comes with a test.
- Be kind. Everyone here follows the [code of conduct](CODE_OF_CONDUCT.md).

## Development setup

You need Rust (stable, edition 2024), the `wasm32-wasip2` target, and a checkout of [OpenIDEA](https://github.com/l0ng-ai/openidea) next to this repository, because the SDK is a path dependency. Use the tag CI uses (`SDK_REF` in `.github/workflows/ci.yml`).

```sh
git clone https://github.com/l0ng-ai/openidea --branch v0.1.38
git clone https://github.com/yzPeedro/laravel-openidea
cd laravel-openidea
rustup target add wasm32-wasip2
```

```
projects/
├─ openidea/            # SDK at openidea/sdk/rust/openidea-plugin
└─ laravel-openidea/    # this repository
```

Optional: PHP 8.2+ for linting the scripts in `php/`, and Node.js for checking the hub's script.

### Run the plugin in OpenIDEA

```sh
cargo install --path ../openidea/crates/ide-host --bin openidea-plugin
cargo build --release --target wasm32-wasip2
cp target/wasm32-wasip2/release/openidea_plugin_laravel_openidea.wasm dist/plugin.wasm
openidea-plugin link .        # once: loads this folder as a development plugin
```

After each build, use **Reload Plugins** (or *Settings › Plugins › Reload*) in OpenIDEA. The hub page is cached by the workbench, so close and reopen the tool window, or reload the window, to see changes to `web/hub.html`.

## Checks

Run these before pushing. CI runs the same ones.

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release --target wasm32-wasip2
for f in php/*.php; do php -l "$f"; done
./scripts/package.sh          # builds the release zip in build/
```

Tests run against the SDK's `MockHost`, so they need neither OpenIDEA nor PHP.

### Testing against a real application

Two ignored tests exercise a real Laravel project:

```sh
LARAVEL_PROJECT=~/projects/my-app cargo test --release --test real_project -- --ignored --nocapture
LARAVEL_PROJECT=~/projects/my-app HUB_DUMP=/tmp/hub cargo test --release --test hub_dump -- --ignored
```

`real_project` prints index sizes, timings and every hub section; `LARAVEL_MEMBER_PROBE=path::needle` (with `LARAVEL_MEMBER_INSERT` to add text after the needle) prints the member completion at that point of a file. `LARAVEL_MODELS` (for example `User,Post`) and `LARAVEL_MODEL_FILES` choose the models it probes. `hub_dump` copies the hub page with its icons and writes every section's data to `HUB_DUMP`, which is handy for working on `web/hub.html` in a browser.

Never commit output from a private project: test fixtures must be made up.

## Project layout

| Path | What lives there |
| --- | --- |
| `src/lib.rs` | Plugin entry: providers (completion, definitions, gutter, code actions), commands, events, Blade syntax |
| `src/php.rs` | PHP and Blade tokenizer and the call context of each string |
| `src/refs.rs` | Which helper, facade or directive points to which index |
| `src/index/` | Cached indexes: config, env, routes, views, translations, models, migrations, Blade, OpenAPI, packages… |
| `src/hierarchy.rs` | Trait, interface and base class completion |
| `src/modeldoc.rs` | *Generate @property PHPDoc* |
| `src/resources.rs` | Reads classes, attributes and calls of the app (listeners, observers, bindings…) |
| `src/sections.rs` | Hub sections built from code |
| `src/panels.rs` | The hub view and its message protocol |
| `src/devtools.rs` | Tinker, Live Audit, debug tools, ecosystem and AI agents |
| `src/runner.rs` | Running PHP directly or through Docker Compose |
| `php/` | Scripts run inside the app: `tinker.php`, `Recorder.php`, `install.php`, `mcp.php`, `guidelines.md` |
| `web/hub.html` | The hub's UI (one file: HTML, CSS and JavaScript), served as a web view with its icons from `web/icons/` |
| `tests/` | Ignored integration tests against a real application |

### How the hub talks to the plugin

The hub page posts JSON messages (`{"type": "section", "id": "routes"}`) through `openidea.postMessage`. `panels.rs` answers with `host::post_view_message`. Requests are the `Request` enum in `panels.rs`, and the page handles replies in the `openidea.onMessage` switch at the end of `web/hub.html`. A `navigate` message (`{"type": "navigate", "id": "tinker", "code": "…", "run": true}`) opens a section with filters, a search, an Artisan command or a Tinker snippet.

### Adding a hub section

1. Build its table in `sections.rs` (code-based) or in `index_section` in `panels.rs` (index-based), returning columns, rows and facets.
2. Add the id to `CODE_SECTIONS` or `DEV_SECTIONS` when needed, and its actions to `section_actions`.
3. Add it to `SECTIONS` in `web/hub.html` with a group, an icon and a description.
4. Add the id to `tests/real_project.rs` and `tests/hub_dump.rs`.

## Conventions

- **No comments in code.** Names and small functions carry the meaning. Type annotations required by tools are fine.
- Keep the completion path fast: providers must answer within the IDE's 50 ms budget, so index work happens on activation and events, never per keystroke.
- No new capabilities without a discussion: the plugin asks only for `project.read`, `events`, and the optional `process:php` and `process:docker`.
- The recorder and installer never touch tracked files, and they mask secrets and personal data.
- UI text is English, short, and in sentence case.
- Formatting is `rustfmt`'s. PHP follows PSR-12. JavaScript in `hub.html` uses two-space indentation and double quotes.

## Issues and labels

New issues come from the forms in `.github/ISSUE_TEMPLATE`: bug report, crash report (the *Plugin crashed* notification or a hub stuck on *Loading*) and feature request. Questions go to [Discussions](https://github.com/yzPeedro/laravel-openidea/discussions) and vulnerabilities to [private reporting](https://github.com/yzPeedro/laravel-openidea/security/advisories/new).

Every new issue starts with `needs triage`, and the *Area* field adds its `area: …` label. Triage then swaps `needs triage` for `confirmed` or `needs info`.

| Group | Labels |
| --- | --- |
| Type | `bug`, `crash`, `enhancement`, `performance`, `compatibility`, `documentation` |
| Status | `needs triage`, `needs info`, `confirmed` |
| Area | `area: completion`, `area: blade`, `area: eloquent`, `area: routes`, `area: hub`, `area: tinker`, `area: live audit`, `area: artisan`, `area: ai agents`, `area: packages` |
| Contributors | `good first issue`, `help wanted` |
| Closing | `duplicate`, `wontfix` |
| Automation | `dependencies` (Dependabot) |

Pull requests use the type and area labels of the issue they close.

## Commits and pull requests

- Write commit subjects in the imperative mood ("Add observer hooks to the hub"), under 72 characters, and explain the why in the body when it isn't obvious.
- Update `CHANGELOG.md` under *Unreleased* for user-visible changes.
- Fill in the pull request template, including how you verified the change. For UI changes, add a before and after screenshot taken on a demo project.

## Releasing

Maintainers bump `version` in `plugin.toml` and `Cargo.toml`, move *Unreleased* in `CHANGELOG.md` to the new version, update `change-notes`, then tag:

```sh
git tag -a v0.13.0 -m "v0.13.0"
git push origin v0.13.0
```

The release workflow builds and checks the package and publishes `laravel-openidea-<version>.zip` with its SHA-256 on GitHub Releases.
