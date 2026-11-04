<p align="center">
  <img src="docs/media/banner.webp" alt="Laravel OpenIDEA" width="100%">
</p>

<p align="center">
  <a href="https://github.com/yzPeedro/laravel-openidea/actions/workflows/ci.yml"><img src="https://github.com/yzPeedro/laravel-openidea/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://github.com/yzPeedro/laravel-openidea/releases/latest"><img src="https://img.shields.io/github/v/release/yzPeedro/laravel-openidea?color=ff2d20&label=release" alt="Latest release"></a>
  <img src="https://img.shields.io/badge/OpenIDEA-plugin%20API%201.0-4f46e5" alt="OpenIDEA plugin API 1.0">
  <img src="https://img.shields.io/badge/Laravel-10%2B-ff2d20?logo=laravel&logoColor=white" alt="Laravel 10 and newer">
  <img src="https://img.shields.io/badge/built%20with-Rust%20%2B%20WebAssembly-orange?logo=rust" alt="Rust and WebAssembly">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-green" alt="MIT license"></a>
</p>

<p align="center">
  <b>The complete Laravel toolkit for <a href="https://github.com/l0ng-ai/openidea">OpenIDEA</a>.</b><br>
  Completion and navigation for every Laravel string, Blade and Eloquent intelligence, and a hub with Artisan, Tinker,<br>
  every resource of your app, live logs, a live audit of requests, queries and events, and an MCP server for AI agents.
</p>

https://github.com/user-attachments/assets/bc56e061-73f0-484d-8e1c-d9776cdc9692

<p align="center"><sub>Install in under a minute, then a tour of the features. <a href="docs/media/install.mp4">Download the MP4</a>.</sub></p>

---

## Contents

- [Highlights](#highlights)
- [Installation](#installation)
- [Features](#features)
  - [Editor intelligence](#editor-intelligence)
  - [The Laravel OpenIDEA hub](#the-laravel-openidea-hub)
  - [Tinker](#tinker)
  - [Live Audit](#live-audit)
  - [Logs, Artisan and the rest](#logs-artisan-and-the-rest)
  - [AI agents](#ai-agents)
- [Configuration](#configuration)
- [Privacy and security](#privacy-and-security)
- [How it works](#how-it-works)
- [Contributing](#contributing)
- [License](#license)

## Highlights

| | |
| --- | --- |
| ⚡ **Completion everywhere** | Routes (with verb and URI), views, config, env, translations, validation rules, middleware, gates, Eloquent columns and relations, Blade components and directives, assets, OpenAPI refs, Sanctum abilities, Spatie roles and permissions, queues, disks, and more. |
| 🧭 **Navigation** | Ctrl+click on a route name or URI opens the controller method; views, config keys, env keys and translations open where they are defined. Gutter icons link `route()` calls and views. |
| 🧩 **A hub for your whole app** | 40+ sections: routes, controllers, requests, models, scopes, observers, events and listeners, jobs, schedule, translations, config, env, bindings and more. Every section has filters, and every row opens its file. |
| 🐘 **Tinker in tabs** | Write PHP like in an editor, press Ctrl+Enter, get Tinker-style coloured output with dumps, SQL and timing. |
| 🔍 **Live Audit** | Requests, SQL with the line that ran it, events with payloads and listeners, jobs, cache, mail, HTTP client calls, gates, logs, exceptions and dumps, with N+1 and slow query insights. |
| 🤖 **AI agents that know your app** | An MCP server that answers with the hub's runtime data, connected in one click next to Laravel Boost. |
| 🐳 **Docker-aware** | Artisan, Tinker and the recorder run with `php` or in a throwaway Docker Compose container, detected automatically. |

## Installation

**Requirements:** OpenIDEA with plugin API 1.0 and a Laravel 10 (or newer) project. PHP or Docker Compose is needed for the runtime features (Artisan, Tinker, Live Audit).

> [!NOTE]
> The plugin uses the `open-location` host function so hub rows can open files at a line. Until it lands in upstream OpenIDEA, run OpenIDEA built from the [`fix/plugin-completion-and-definitions`](https://github.com/yzPeedro/openidea/tree/fix/plugin-completion-and-definitions) branch.

1. **Download** `laravel-openidea-<version>.zip` from the [latest release](https://github.com/yzPeedro/laravel-openidea/releases/latest).
2. In OpenIDEA open **Settings › Plugins › ⚙ › Install Plugin from Package File…** and pick the zip.

   <img src="docs/media/settings-plugins-menu.webp" alt="Settings, Plugins, gear menu, Install Plugin from Package File" width="80%">

3. **Allow `process:php` or `process:docker`** in the plugin's permissions. They are optional: completion and navigation work without them, while Artisan, Tinker and Live Audit need one of them.

   <img src="docs/media/settings-plugins.webp" alt="Plugin permissions in Settings" width="80%">

4. **Open a Laravel project** and click the Laravel icon at the bottom of the left tool bar.

<details>
<summary><b>Build from source</b></summary>

```sh
git clone https://github.com/yzPeedro/openidea --branch fix/plugin-completion-and-definitions
git clone https://github.com/yzPeedro/laravel-openidea
cd laravel-openidea
rustup target add wasm32-wasip2
./scripts/package.sh          # builds dist/plugin.wasm and build/laravel-openidea-<version>.zip
```

Install the zip as above, or link the folder for development with `openidea-plugin link` (see [CONTRIBUTING.md](CONTRIBUTING.md)).

</details>

## Features

### Editor intelligence

Completion and navigation come from cached indexes of your project files, built to answer within the IDE's 50 ms completion budget.

| Routes, with verb and URI | Eloquent columns from migrations |
| :---: | :---: |
| <img src="docs/media/editor-route-completion.webp" alt="Route name completion"> | <img src="docs/media/editor-column-completion.webp" alt="Column completion"> |
| **Traits, interfaces and base classes** | **Gutter links to routes and views** |
| <img src="docs/media/editor-trait-completion.webp" alt="Trait completion"> | <img src="docs/media/editor-gutter.webp" alt="Gutter icons"> |

- **Strings that mean something:** `route()`, `to_route()`, `redirect()->route()`, `view()`, `config()`, `env()`, `__()`, `trans()`, `@lang`, `Route::has`, `routeIs`, `Storage::disk`, `Cache::store`, `Queue`, `Log::channel`, `DB::connection`, `Mail::mailer`, `auth()->guard`, middleware, gates, policies and rate limiters.
- **Eloquent:** columns and relations in `where`, `with`, `has`, `whereHas`, `orderBy`, `create`, `update`, `fill` and `pluck`, model attributes (`#[Fillable]`, `#[Hidden]`, `#[Table]`…), `casts()`, scopes, and member snippets for `$fillable`, `$casts` and relations.
- **Validation:** rule names inside `'required|…'` strings and rule arrays, plus custom rules.
- **Blade:** syntax highlighting, structure and folding, directives after `@` (including your own `Blade::directive` and `Blade::if`), `<x-…>` components, `<livewire:…>` tags, sections, stacks and assets.
- **OpenAPI and packages:** swagger-php `$ref`s, tags and security schemes, Sanctum token abilities, Spatie roles and permissions, Socialite drivers, broadcast channels, Inertia pages and Livewire components.
- **Types:** Laravel, package and project traits after `use`, interfaces after `implements` and base classes after `extends`, ranked by folder (models get `HasFactory` and `SoftDeletes` first, jobs get `ShouldQueue`, and so on).
- **Inspections and quick fixes:** unknown views, routes, rules, middleware and OpenAPI refs; *Add translation*; *Generate @property PHPDoc* for models from migrations, casts and relations; *Laravel: Go to Related Class or View* for Livewire and Blade components.
- **`.env` files:** key and value completion (connections, stores, disks, channels, mailers, locales) with navigation to the config that reads each key.
- **File icons** from the Material Icon Theme for Laravel files.

### The Laravel OpenIDEA hub

One tool window for everything your app has. It refreshes from your files and opens every row in the editor.

<img src="docs/media/hub-overview.webp" alt="Hub overview">

| Routes, filtered by method, prefix, controller and tests | Listeners and the events they handle |
| :---: | :---: |
| <img src="docs/media/hub-routes.webp" alt="Routes"> | <img src="docs/media/hub-listeners.webp" alt="Listeners"> |
| **Models with tables, relations and scopes** | **Observers with their models and hooks** |
| <img src="docs/media/hub-models.webp" alt="Models"> | <img src="docs/media/hub-observers.webp" alt="Observers"> |
| **Translations, filtered by locale and group** | **Queues and workers** |
| <img src="docs/media/hub-translations.webp" alt="Translations"> | <img src="docs/media/hub-queues.webp" alt="Queues"> |

<details>
<summary><b>Every section</b></summary>

| Group | Sections |
| --- | --- |
| Project | Overview, Ecosystem (starter kit, Sanctum, Wayfinder, Horizon, Reverb, Octane, Pulse, Telescope…), AI Agents |
| Develop | Artisan, Tinker |
| HTTP | Routes, Controllers, Form Requests, API Resources, Middleware |
| Data | Models, Scopes, Observers, Casts, Enums, Database, Factories & Seeders |
| Views | Views, Components, Frontend (Vite assets, Inertia pages), Translations |
| App | All Classes, Service Providers, Container Bindings, Validation Rules, Macros |
| Configure | Config, Environment, Drivers & Connections, Auth & Policies |
| Run | Queues & Workers, Scheduling, Events, Listeners, Mail & Notifications, Tests |
| Observe | Live Audit, Logs, Profiles (Clockwork, Debugbar), Debug Calls, Debug Tools |

| Events | Scopes | Database |
| :---: | :---: | :---: |
| <img src="docs/media/hub-events.webp" alt="Events"> | <img src="docs/media/hub-scopes.webp" alt="Scopes"> | <img src="docs/media/hub-database.webp" alt="Database"> |
| **Config** | **Container bindings** | **All classes** |
| <img src="docs/media/hub-config.webp" alt="Config"> | <img src="docs/media/hub-bindings.webp" alt="Bindings"> | <img src="docs/media/hub-classes.webp" alt="Classes"> |
| **Scheduling** | **Ecosystem** | **Artisan** |
| <img src="docs/media/hub-schedule.webp" alt="Schedule"> | <img src="docs/media/hub-ecosystem.webp" alt="Ecosystem"> | <img src="docs/media/hub-artisan.webp" alt="Artisan"> |

</details>

### Tinker

As many tabs as you like, with PHP highlighting, snippets and remembered code. **Ctrl+Enter** runs the tab and **Ctrl+Shift+Enter** runs the selection. Each statement shows its value the way Tinker does, with echo output, `dump()` calls, SQL queries and timing. Models resolve by their short name.

<img src="docs/media/hub-tinker.webp" alt="Tinker">

### Live Audit

Turn the recorder on and every request, Artisan command, queued job and Tinker run in the local environment is recorded:

- requests with route, middleware, input and timing;
- SQL with bindings and the line of your code that ran it;
- events with their payload and the listeners that received them;
- jobs queued, processed and failed, cache hits and misses, mail, notifications, HTTP client calls, gate checks, logs, exceptions and `dump()` output.

The hub groups records by request, filters them by type, and flags **N+1 queries**, slow queries, exceptions and failed jobs.

<img src="docs/media/hub-audit.webp" alt="Live Audit">

### Logs, Artisan and the rest

- **Logs:** every file in `storage/logs`, live, with level chips and counts (click one level, Ctrl+click to combine), search, stack traces with clickable frames.
- **Artisan:** every command of the app and its packages, with arguments and options as a form. Run it in the terminal or show the coloured output in place, with history.
- **Debug toolbox:** Clockwork and Debugbar profiles, `dd()`/`dump()`/`ray()`/`ds()` calls left in the code, and the debug tools you have installed.

| Logs, every level | Logs, only errors |
| :---: | :---: |
| <img src="docs/media/hub-logs.webp" alt="All logs"> | <img src="docs/media/hub-logs-errors.webp" alt="Error logs"> |

### AI agents

**Connect AI agents** installs the `laravel-openidea` MCP server and registers it in `.mcp.json` and, when their folders exist, in the Cursor, VS Code (Copilot), Junie and Gemini configs, with a short guide in `AGENTS.md`. Agents then call tools that answer from the running app:

`app_overview` · `list_routes` · `list_events_and_listeners` · `list_models` · `list_container_bindings` · `list_schedule` · `list_queues` · `list_artisan_commands` · `get_config` · `database_schema` · `missing_translations` · `read_logs` · `audit_recent` · `run_tinker` · `hub_sections`

[Laravel Boost](https://github.com/laravel/boost) installs and updates from the same page; with Boost the guide goes to `.ai/guidelines` so `boost:update` merges it.

<img src="docs/media/hub-ai.webp" alt="AI Agents">

## Configuration

*Settings › Plugins › Laravel OpenIDEA*:

| Setting | Default | What it does |
| --- | --- | --- |
| `artisanRunner` | `auto` | `php`, `docker-run` (throwaway `docker compose run --rm --no-deps`), `docker` (`docker compose exec`) or `auto` (Docker Compose when the project has a compose file) |
| `dockerService` | detected | Compose service that runs PHP (`app`, `laravel.test`, `php`… are detected) |
| `locale` | `en` | Preferred locale for translation completion |
| `unknownViewInspection` | on | Report views that don't exist |
| `unknownRouteInspection` | off | Report unknown route names |
| `unknownRuleInspection` | on | Report unknown validation rules |
| `unknownAuthInspection` | off | Report unknown middleware and abilities |
| `unknownOpenApiRefInspection` | on | Report unknown OpenAPI `$ref` components |

Live Audit reads two environment variables from your app: `OPENIDEA_AUDIT=false` turns the recorder off and `OPENIDEA_AUDIT_SHOW_PII=true` shows personal data.

## Privacy and security

- The plugin is a sandboxed WebAssembly component. It reads project files, and it runs processes only through the `process:php` and `process:docker` permissions you grant.
- The Live Audit recorder runs only in the `local`, `development` and `dev` environments. It lives in `storage/openidea`, which has its own `.gitignore`, and loads through a marked hook in `vendor/autoload.php`, so no tracked file changes. *Turn off* removes both.
- Passwords, tokens, keys, cookies and signatures are always masked in recorded payloads. Personal data such as e-mail, CPF, phone, documents and addresses is masked by default.
- The MCP server masks secrets in config values. Its `run_tinker` tool runs code in your local app, so only connect agents you trust.
- Nothing is sent anywhere: records stay in `storage/logs/openidea-audit.jsonl` on your machine.

## How it works

```
OpenIDEA ── WIT (openidea:plugin@1.0) ──► laravel-openidea.wasm (Rust)
  │                                         ├─ index/      cached indexes of routes, config, views, models, migrations…
  │  completion, definitions, gutter,       ├─ refs.rs     which helper, facade or directive points to which index
  │  code actions, Blade syntax             ├─ php.rs      PHP and Blade tokenizer with call context
  │                                         ├─ panels.rs   the hub (HTML view) and its sections
  │  web view messages ◄──────────────────► ├─ devtools.rs Tinker, Live Audit, debug tools, ecosystem, agents
  │                                         └─ runner.rs   php or Docker Compose
  └─ processes (php / docker) ──► php/tinker.php · php/Recorder.php · php/install.php · php/mcp.php
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for the development setup, tests and conventions.

## Contributing

Issues and pull requests are welcome. Start with [CONTRIBUTING.md](CONTRIBUTING.md), and see the [roadmap](ROADMAP.md) for what's next. Please follow the [code of conduct](CODE_OF_CONDUCT.md), and report security issues as described in [SECURITY.md](SECURITY.md).

## License

[MIT](LICENSE) © Pedro Pessoa. File icons come from the [Material Icon Theme](https://github.com/material-extensions/vscode-material-icon-theme) (MIT, see `icons/LICENSE-material-icon-theme`).

Laravel is a trademark of Taylor Otwell. This project is not affiliated with Laravel, JetBrains or the OpenIDEA authors.
