# Development

How to build Laravel OpenIDEA from source, run it in OpenIDEA from your checkout, test it (unit tests, the MCP server test and the end-to-end checks against a real Laravel application), package it and release it. The same steps run in CI, so a change that passes them locally passes there.

## Requirements

| Tool | Why |
| --- | --- |
| Rust, stable, edition 2024 | The plugin is a Rust library (`crate-type = ["cdylib", "rlib"]`) |
| The `wasm32-wasip2` target | It is compiled to a WebAssembly component: `rustup target add wasm32-wasip2` |
| A checkout of [OpenIDEA](https://github.com/l0ng-ai/openidea) next to this repository | The SDK is a path dependency: `openidea-plugin = { path = "../openidea/sdk/rust/openidea-plugin" }`. Use the tag CI uses (`SDK_REF` in `.github/workflows/ci.yml`, currently `v0.1.40`) |
| PHP 8.2 or newer (optional) | Linting the scripts in `php/` and the MCP server test |
| Node.js (optional) | Checking the hub's script |

```sh
git clone https://github.com/l0ng-ai/openidea --branch v0.1.40
git clone https://github.com/yzPeedro/laravel-openidea
cd laravel-openidea
rustup target add wasm32-wasip2
```

```
projects/
├─ openidea/            # SDK at openidea/sdk/rust/openidea-plugin
└─ laravel-openidea/    # this repository
```

## Run the plugin in OpenIDEA

```sh
cargo install --path ../openidea/crates/ide-host --bin openidea-plugin
cargo build --release --target wasm32-wasip2
cp target/wasm32-wasip2/release/openidea_plugin_laravel_openidea.wasm dist/plugin.wasm
openidea-plugin link .        # once: loads this folder as a development plugin
```

`openidea-plugin link .` registers the folder; it then shows in **Settings › Plugins** under **Linked for development** with a *linked* badge, and its *Location* is your checkout. After each build use **Reload Plugins** (or *Settings › Plugins › Reload*).

The hub page is cached by the workbench: after changing `web/hub.html`, close and reopen the tool window, or reload the window.

The release build is small and fast: `opt-level = "s"`, `lto = true`, `strip = true`, `codegen-units = 1` and `panic = "abort"`.

## Project layout

| Path | What lives there |
| --- | --- |
| `plugin.toml` | The manifest: id `pedro.laravel-openidea`, version, `engines.openidea = "^1.4"`, capabilities and every contribution |
| `settings.schema.json` | The 22 [settings](Settings.md) |
| `src/lib.rs` | Plugin entry: providers (completion, definitions, gutter, code actions), commands, events, Blade syntax |
| `src/php.rs` | PHP and Blade tokenizer and the call context of each string |
| `src/refs.rs` | Which helper, facade or directive points to which index |
| `src/index/` | Cached indexes: config, env, routes, views, translations, models, migrations, Blade, OpenAPI, packages… |
| `src/framework.rs` | What the installed `laravel/framework` has |
| `src/hierarchy.rs` | Trait, interface and base class completion |
| `src/indexer.rs`, `src/usages.rs`, `src/projectfiles.rs` | The document indexer and the project file store behind gutter usages |
| `src/checks.rs`, `src/inspections.rs`, `src/inspect.rs` | Inspections and quick fixes |
| `src/renames.rs`, `src/formrequest.rs`, `src/modeldoc.rs`, `src/querysql.rs`, `src/queries.rs` | Refactorings, Extract Form Request, model PHPDoc, Laravel to SQL, the editor commands |
| `src/resources.rs` | Reads classes, attributes and calls of the app (listeners, observers, bindings…) |
| `src/sections.rs` | Hub sections built from code |
| `src/security.rs`, `src/securityscan.rs`, `src/unused.rs`, `src/upgrade.rs` | Security inspections and the Security section, the Unused section, and the upgrade rules taken from the official guides |
| `src/routetools.rs`, `src/runtime.rs` | Route fields, bindings and sample bodies for the request panel, the feature test generator, and naming the PHP runtime |
| `src/schemadiff.rs`, `src/indexadvice.rs` | Comparing the migrations with the live database, the schema diagram, and missing index advice for Live Audit |
| `src/panels.rs` | The hub view and its message protocol |
| `src/devtools.rs` | Tinker, Live Audit, debug tools, ecosystem and AI agents |
| `src/runner.rs` | Running PHP directly, through `php-win`, or through Docker Compose |
| `src/tools.rs`, `src/helpers.rs`, `src/mcp.rs`, `src/hubtabs.rs` | Tools on save, IDE helpers and path completion, the MCP sync, hub editor tabs |
| `php/` | Scripts run inside the app: `tinker.php`, `Recorder.php`, `install.php`, `mcp.php`, `cache.php`, `schema.php`, `request.php`, `create_file.php`, `selection.php`, `tools.php`, `guidelines.md` |
| `web/hub.html` | The hub's UI (one file: HTML, CSS and JavaScript), served as a web view with its icons from `web/icons/` |
| `templates/`, `icons/`, `languages/` | File templates, file and gutter icons, the Blade and NEON languages |
| `docs/snippets.md` | The list of snippets, generated from the same table as the code |
| `tests/` | Integration tests: the MCP server, and ignored tests against a real application |
| `scripts/` | Packaging and signing |

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

### Unit tests

`cargo test` runs the `#[test]` functions in `src/` against the SDK's `MockHost`, which fakes the IDE: files, settings, capabilities, editors, terminals, process runs, the clipboard, status items and the log. They need neither OpenIDEA nor PHP. Most features have a test that reads like a spec, for example `renaming_a_route_updates_its_usages_before_and_after_saving`, `each_window_shows_its_own_laravel_version` or `a_healthy_project_logs_nothing`.

### The MCP server test

`tests/mcp_server.rs` starts `php/mcp.php` with your `php`, speaks JSON-RPC to it over stdio and checks what each access level offers (`run_tinker` only at `full`, `format_code` not at `read`…). It skips itself when `php` isn't installed.

### End-to-end checks against a real application

Two ignored tests load a real Laravel project into the mock host and run the plugin on it:

```sh
LARAVEL_PROJECT=~/projects/my-app cargo test --release --test real_project -- --ignored --nocapture
LARAVEL_PROJECT=~/projects/my-app HUB_DUMP=/tmp/hub cargo test --release --test hub_dump -- --ignored
```

| Variable | Test | Effect |
| --- | --- | --- |
| `LARAVEL_PROJECT` | both | The application to load |
| `LARAVEL_MODELS` | `real_project` | Models to probe, such as `User,Post` |
| `LARAVEL_MODEL_FILES` | `real_project` | Model files to probe |
| `LARAVEL_MEMBER_PROBE` | `real_project` | `path::needle`: prints the member completion at that point of a file |
| `LARAVEL_MEMBER_INSERT` | `real_project` | Text to insert after the needle before probing |
| `ROUTES_OUT` | `real_project` | Writes every route name completion to this file |
| `LARAVEL_PROBE` | `real_project` (inspections) | PHP code added as `app/OpenideaProbe.php` and inspected with the rest |
| `HUB_DUMP` | `hub_dump` | Folder that receives a copy of the hub page with its icons and every section's data |

`real_project` prints index sizes, completion timings (first and cached call), every hub section and the inspections of `app/`, `routes/` and `.env`. `hub_dump` lets you work on `web/hub.html` in a browser with real data.

Never commit output from a private project: test fixtures must be made up.

## How the hub talks to the plugin

The hub page posts JSON messages (`{"type": "section", "id": "routes"}`) through `openidea.postMessage`. `panels.rs` answers with `host::post_view_message`. Requests are the `Request` enum in `panels.rs`, and the page handles replies in the `openidea.onMessage` switch at the end of `web/hub.html`. A `navigate` message (`{"type": "navigate", "id": "tinker", "code": "…", "run": true}`) opens a section with filters, a search, an Artisan command or a Tinker snippet.

### Adding a hub section

1. Build its table in `sections.rs` (code-based) or in `index_section` in `panels.rs` (index-based), returning columns, rows and facets.
2. Add the id to `CODE_SECTIONS` or `DEV_SECTIONS` when needed, and its actions to `section_actions`.
3. Add it to `SECTIONS` in `web/hub.html` with a group, an icon and a description, and to `SECTIONS` in `src/hubtabs.rs` so it can open in an editor tab.
4. Add the id to `tests/real_project.rs` and `tests/hub_dump.rs`.

## Packaging

| Script | Produces |
| --- | --- |
| `scripts/package.sh` | Builds the WebAssembly file, copies it to `dist/plugin.wasm`, and zips `plugin.toml`, `settings.schema.json`, `LICENSE`, `README.md`, `dist/`, `icons/`, `languages/`, `templates/` and `web/` into `build/laravel-openidea-<version>.zip`, with a `.sha256` next to it |
| `scripts/oiplugin.sh` | Packs that folder into `build/pedro.laravel-openidea-<version>.oiplugin`, the marketplace package, with the `openidea-plugin` CLI (downloaded for `SDK_REF` unless `OPENIDEA_PLUGIN_CLI` points at one), signs it as publisher `pedro` when `OPENIDEA_PUBLISHER_KEY` is set, checks it and writes its `.sha256`. Run `package.sh` first |
| `scripts/publisher-key.mjs` | Generates an Ed25519 publisher key into a file you name, refusing a path inside the repository, and prints the public key |

## CI and releases

`.github/workflows/ci.yml` runs on pushes to `main`, pull requests and by hand, with three jobs:

| Job | Steps |
| --- | --- |
| Rust (fmt, clippy, tests, wasm) | Sparse checkout of the SDK at `SDK_REF`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `package.sh`, `oiplugin.sh`, and the zip, `.oiplugin` and checksums as an artifact |
| PHP scripts | `php -l` on every `php/*.php` with PHP 8.3 |
| Hub script | Extracts the `<script>` of `web/hub.html` and runs `node --check` on it |

`.github/workflows/release.yml` runs on a `v*` tag: it checks the tag matches `version` in `plugin.toml`, runs the tests, packages, signs the marketplace package with the `OPENIDEA_PUBLISHER_KEY` secret, takes the release notes from the tag's section of `CHANGELOG.md` and publishes the GitHub release. A second job, *Open the registry pull request*, then lists the version in the marketplace.

### The marketplace registry

The OpenIDEA marketplace reads the community registry [l0ng-ai/openidea-plugins](https://github.com/l0ng-ai/openidea-plugins), not GitHub releases: a version shows in **Settings › Plugins › Available** only once the registry lists it in `plugins/pedro.laravel-openidea.toml`. The *Open the registry pull request* job does that after the release is published:

1. syncs the fork `yzPeedro/openidea-plugins` with the registry's `main` (`gh repo sync`);
2. on a branch `laravel-openidea-<version>` of the fork, adds a `[[versions]]` table to `plugins/pedro.laravel-openidea.toml` with the version, the URL of `pedro.laravel-openidea-<version>.oiplugin` on the release and its checksum from `build/*.oiplugin.sha256`, keeping the earlier tables;
3. pushes the branch to the fork and opens the pull request `Update pedro.laravel-openidea to <version>` in the registry, with the registry's template filled in: the plugin, the release link and both boxes checked.

A registry maintainer merges it after the checks pass; until then the marketplace offers the previous version. The job does nothing when the registry already lists the version or a pull request from that branch is open, so it can be run again from the workflow run (*Re-run failed jobs*) without publishing the release a second time.

It needs the `REGISTRY_TOKEN` secret and fails with "the REGISTRY_TOKEN secret is not set" without it. Use a classic personal access token of the fork's owner with the `public_repo` scope: fine-grained tokens can't write to public repositories their owner isn't a member of, so they can push to the fork but not open the pull request in `l0ng-ai/openidea-plugins`.

1. Create the token in **GitHub › Settings › Developer settings › Personal access tokens › Tokens (classic) › Generate new token (classic)**, with the `public_repo` scope and an expiry date.
2. Add it to this repository in **Settings › Secrets and variables › Actions › New repository secret**, named `REGISTRY_TOKEN`, or with `gh secret set REGISTRY_TOKEN --repo yzPeedro/laravel-openidea`.
3. Renew it before it expires; an expired token makes the job fail at `gh repo sync`.

To release, maintainers bump `version` in `plugin.toml` and `Cargo.toml`, move *Unreleased* in `CHANGELOG.md` to the new version, update `change-notes`, then tag:

```sh
git tag -a v0.15.0 -m "v0.15.0"
git push origin v0.15.0
```

## Conventions

- **No comments in code.** Names and small functions carry the meaning. Type annotations required by tools are fine.
- Keep the completion path fast: providers must answer within the IDE's 50 ms budget, so index work happens on activation and events, never per keystroke. See [How it works](How-It-Works.md#time-budgets).
- No new capabilities without a discussion.
- The recorder and installer never touch tracked files, and they mask secrets and personal data.
- UI text is English, short, and in sentence case.
- Formatting is `rustfmt`'s. PHP follows PSR-12. JavaScript in `hub.html` uses two-space indentation and double quotes.
- Every behaviour change comes with a test. One topic per pull request; open an issue first for a large change.
- Commit subjects in the imperative mood, under 72 characters. User-visible changes go under *Unreleased* in `CHANGELOG.md`.

The full guide is [CONTRIBUTING.md](../CONTRIBUTING.md).

## Documentation

Every feature has a page in this wiki, and a pull request that adds or changes behaviour updates the page in the same change. The pages live in the repository's `wiki/` folder, where they are reviewed with the code. `scripts/publish-wiki.py` publishes them to this GitHub wiki:

```sh
python3 scripts/publish-wiki.py            # converts wiki/ and pushes it to the GitHub wiki
python3 scripts/publish-wiki.py --dry-run  # leaves the converted copy in a temporary folder
```

The script:
- drops `.md` from links between pages and turns links to files outside `wiki/` (such as `docs/snippets.md` or the banner) into absolute GitHub URLs;
- builds the sidebar (`_Sidebar.md`) from the tables in `wiki/Home.md`;
- copies `wiki/images/`;
- commits as you and pushes to the wiki's own repository.

When nothing changed, it says so and pushes nothing. Screenshots are taken in OpenIDEA on a demo project, never a private one, and saved as WebP in `wiki/images/`.

## Related

- [Installation](Installation.md)
- [How it works](How-It-Works.md)
- [Troubleshooting and logs](Troubleshooting.md)
- [Settings](Settings.md)

---
[← Wiki home](Home.md)
