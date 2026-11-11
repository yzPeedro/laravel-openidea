# How it works

Laravel OpenIDEA is a Rust library compiled to a WebAssembly component and loaded by OpenIDEA through its WIT plugin API, version 1.4. It answers completion, definition, gutter and code action requests from cached indexes of your project, keeps those indexes fresh from file events, and runs PHP in your app only for the runtime features. This page explains the pieces, the time budgets they live within and the PHP scripts the plugin carries.

## Architecture

```
OpenIDEA ── WIT plugin API 1.4 ──► dist/plugin.wasm (Rust, sandboxed)
  │                                  ├─ index/         cached indexes: routes, config, views, models, migrations, translations…
  │  completion, definition,         ├─ framework.rs   rules, directives, casts and methods of the installed laravel/framework
  │  gutter, code actions,           ├─ refs.rs        which helper, facade or directive points to which index
  │  Blade and NEON syntax           ├─ php.rs         PHP and Blade tokenizer with the call context of each string
  │                                  ├─ indexer.rs     the document indexer behind the gutter usages
  │  document indexer ◄────────────► ├─ checks.rs, inspections.rs   inspections and quick fixes
  │                                  ├─ renames.rs, formrequest.rs, modeldoc.rs, querysql.rs   refactorings and actions
  │  web view messages ◄───────────► ├─ panels.rs, sections.rs      the hub and its sections
  │                                  ├─ devtools.rs    Tinker, Live Audit, debug tools, ecosystem, AI agents
  │                                  ├─ routetools.rs, runtime.rs   send request, generate test, the PHP runtime
  │                                  └─ runner.rs      php, php-win or Docker Compose
  └─ processes (php / docker) ──► php/tinker.php · Recorder.php · install.php · mcp.php · cache.php · schema.php · request.php · tools.php…
```

The hub is `web/hub.html`, one file of HTML, CSS and JavaScript shown as a web view. It talks to the plugin with JSON messages (`{"type": "section", "id": "routes"}`); `panels.rs` answers each one. Live Audit updates are kept under about 900 KB, dropping the oldest records of a very large first load, so the IDE accepts them.

## What the plugin contributes

`plugin.toml` declares what OpenIDEA wires up:

| Contribution | For |
| --- | --- |
| Providers: completion, definition, line markers, code actions | `php`, `blade` and `dotenv` files; the code actions include file actions (`file-actions` export), which create a file and open it, as *Generate a feature test* does |
| Providers: completion and code actions | `json` and `neon` files (`composer.json`, `pint.json`, translation JSON, `phpstan.neon`) |
| Provider: completion | `xml` (`phpunit.xml`), `javascript`, `typescript`, `javascriptreact`, `typescriptreact`, `vue`, `svelte` |
| Languages | Blade (`languages/blade.toml`) and NEON (`languages/neon.toml`), with highlighting, structure and folding for Blade from the plugin |
| View | The hub tool window (`laravel`, icon at the bottom of the left tool bar) |
| Custom editor | `*.laravel-hub` files open as the hub in an editor tab |
| Marker icon | The Laravel icon for `laravel-usages#` markers, tooltip "Where it's used" |
| Document indexer | `php` files, for [gutter usages](Gutter-Usages.md) |
| Commands and menus | The eleven [commands](Commands.md), four of them in the editor's right-click menu |
| File templates, run detectors, file icons | [File templates](File-Templates.md), [Run configurations](Run-Configurations.md), [File icons](File-Icons.md) |
| Settings | `settings.schema.json` ([Settings](Settings.md)) |

The plugin activates on the first PHP or Blade file, or when the project has an `artisan` file.

## Which projects it handles

A project is a Laravel project when it has an `artisan` file and its `composer.json` mentions `laravel/framework` or `laravel/laravel`. The answer is cached per project root. Several projects can be open at once, in one window or several: every index, cache and status is kept per root, and a file belongs to the open project with the longest matching root. Paths are compared without regard to `\` or `/`, the `\\?\` prefix, or letter case on Windows.

## Indexes

Completion and navigation read **indexes**: lists of entries (a key, a detail and a location) for each kind of thing. There are 41 kinds: config keys, routes and route actions, views, env variables, translations, disks, cache stores, queue connections, log channels, database connections, mailers, guards, password brokers, broadcasters, middleware, abilities, rate limiters, tables, columns, models, relations, scopes, controller methods, Blade components, Livewire components, Inertia pages, sections, stacks, assets, validation rules, OpenAPI components, token abilities, broadcast channels, Artisan commands, queues, roles, permissions, Socialite drivers, Blade directives and pivot attributes.

Each index is built the first time something asks for it and kept until a file it depends on changes. For example, routes depend on `routes/`, `bootstrap/app.php`, `bootstrap/cache/`, `app/Providers/` and `resources/views/` (Folio pages); columns on `database/migrations/` and `app/Models/`; translations on `lang/` and `resources/lang/`; env variables on `.env*`. Disks, cache stores, connections, mailers and guards are derived from the config index.

On top of the indexes the plugin keeps:

- **The framework**: what the installed `vendor/laravel/framework` has (validation rules, Blade directives, casts, middleware aliases, container ids, facades, collection, query, relation and factory methods), plus which Laravel classes and packages exist, checked without reading their files. It is reread when `composer.lock` or `vendor/composer/installed.json` changes. Without `vendor/` the built-in lists are used.
- **The project files**: every PHP file under `app/`, `database/`, `routes/`, `tests/`, `config/`, `resources/views/`, plus `bootstrap/app.php` and `bootstrap/providers.php` (up to 20,000 per folder), read once and then updated one file at a time. They feed class completion, the hub's code-based sections and Ctrl+click on a class name.
- **Inspection facts**: models, columns and the config keys that read each env variable, kept per project so switching windows doesn't rebuild them.

A scan that hits its file limit, a file that can't be read and an index that takes more than two seconds to build are written to the IDE log.

## Time budgets

OpenIDEA gives each call into a plugin **5 seconds**. While the plugin has a background task open (shown in the status bar), the call may run for up to **60 seconds**. Completion has its own, much shorter budget of about 50 ms, so the plugin never builds anything large while you type; at most 500 completion items are returned.

The plugin spreads its work to stay inside these limits:

| When | What runs | Budget |
| --- | --- | --- |
| First contact with a project (any feature, a file opened, the hub) | Read the framework and build every index, as the background task *Indexing the Laravel project* | Up to 30 seconds |
| A file opened or an editor focused, while anything is still missing | Continue the indexes, then the project files | Slices of about 1.5 seconds, then 250 ms of file reading |
| After each batch of events | Rebuild what the changes invalidated, then publish diagnostics | About 1.5 seconds for the whole batch |
| Gutter markers of an open file | Read a few more project files | 60 ms |
| A hub section built from code | Finish reading the project files | 2.5 seconds |
| *Laravel: Refresh Index* | Rebuild everything, then reread the project files | 3.5 seconds in total, the rest in the background |

Since 0.15.0 a large project, or a second Laravel window, no longer restarts the plugin: the first indexing runs as a background task, which OpenIDEA lets run past the five-second limit, and later refreshes run in slices. Reading the framework takes about 2 seconds on a large project (it used to take about 4).

## Keeping indexes fresh

The plugin subscribes to five events:

| Event | What happens |
| --- | --- |
| **File saved** | The indexes that depend on the file are dropped and rebuilt on demand; the project file store updates that file; route names and translation keys are compared before and after, to offer [Rename route](Rename-Route.md) and [Rename translation](Rename-Translation.md); [tools on save](Tools-on-Save.md) run; saving `config/auth.php` refreshes the [IDE helper stub](IDE-Helpers.md) |
| **File opened** | Indexing continues in the background, the [status bar](Status-Bar.md) shows the project's version, and route and translation files are remembered as they are on disk (for renames before saving) |
| **Files changed** | Files created, changed or removed on disk (by git, Composer, a terminal…). The event is filtered to `app/**`, `routes/**`, `config/**`, `database/**`, `resources/**`, `lang/**`, `tests/**`, `bootstrap/app.php`, `bootstrap/providers.php`, `bootstrap/cache/routes-v7.php`, `composer.lock`, `vendor/composer/installed.json`, `.env` and `.env.example`. Each path invalidates its indexes; more than 200 paths at once drop everything, to be rebuilt on demand |
| **VCS changed** | A branch switch or similar: every index is dropped |
| **Editor focused** | Indexing continues, the status bar is updated, and `storage/openidea` is synced (IDE helper stub, hub tab files, MCP server files) |

**Git-ignored files.** OpenIDEA doesn't report changes to git-ignored files, and `.env`, `vendor/composer/installed.json` and `bootstrap/cache/routes-v7.php` usually are. So the plugin also remembers the size and modification time of the single files it watches (`.env`, `.env.example`, `composer.lock`, `vendor/composer/installed.json`, `bootstrap/cache/routes-v7.php`, `bootstrap/app.php`, `bootstrap/providers.php`) and compares them on every completion, every Ctrl+click and every batch of events. A key added to `.env` from a terminal shows up in `env('` completion on the next keystroke, without an IDE restart.

## Live diagnostics

Inspections run on the editor's text while you type: when the IDE asks for gutter markers or code actions of an open PHP file, the plugin inspects that version of the document and publishes the warnings for it, once per version. Saved files are inspected again from disk after each batch of events. PHPStan results from [tools on save](Tools-on-Save.md) are kept per file and published together with the inspections.

## The document indexer

`[[contributes.documentIndexers]]` lets OpenIDEA index every PHP file in the background through the plugin. For each file in `app/`, `database/`, `routes/`, `tests/`, `config/` or `resources/views/`, the plugin returns:

- **definitions**: each place the file uses a project class (resolved through `namespace` and `use`, limited to your PSR-4 namespaces) or calls a model scope, skipping comments, strings and `use` imports;
- **markers**: for files in `app/` and `database/`, each class, interface, trait and enum declared, and each scope declared in a model.

OpenIDEA keeps the project-wide graph from these answers and draws the Laravel gutter icon on each marker with its usages, following your edits and changes on disk. See [Gutter usages](Gutter-Usages.md).

## Running PHP

The runtime features run PHP in your app through `runner.rs`, with the [runner](Settings.md#artisanrunner) you chose: `php` in the project root, `docker compose run --rm --no-deps --entrypoint php <service>` or `docker compose exec <service> php`. On Windows with the `php` runner, background calls go through `php-win.exe` when `process:php-win` is allowed, and fall back to `php` otherwise. Terminals always use the visible program.

The plugin carries its PHP scripts inside the WebAssembly file and passes them to `php -r`. A script longer than 1 KB travels on stdin with a small loader as the argument, because OpenIDEA 0.1.39 accepts process arguments of at most 4096 bytes. Scripts answer with JSON after a marker line (`@@openidea@@`) at the end of their output, so warnings your app prints don't break the reply.

| Script | What it does |
| --- | --- |
| `php/tinker.php` | Boots the app and runs code from the hub's Tinker, statement by statement, with the value, echo output, `dump()` calls, SQL queries and timing of each |
| `php/Recorder.php` | The Live Audit recorder: a service provider that records requests, queries, events, jobs, cache, mail, HTTP client calls, gates, logs, exceptions and dumps to `storage/logs/openidea-audit.jsonl`, masking secrets and personal data |
| `php/install.php` | Installs and removes the recorder and its `vendor/autoload.php` hook, writes the IDE helper stub and the hub tab files, connects and disconnects AI agents, syncs the MCP server files and clears the audit log |
| `php/mcp.php` | The `laravel-openidea` MCP server, copied to `storage/openidea/mcp.php` and started by your AI agent; every call boots the app again |
| `php/cache.php` | Lists, reads, forgets and flushes cache keys for the hub's Cache section |
| `php/schema.php` | Reads the live schema and pending migrations, the rows of a table (masked) and the `EXPLAIN` of a query for the hub's Database section and Live Audit |
| `php/request.php` | Runs one request inside the app through the HTTP kernel for the hub's request panel: CSRF middleware swapped for a pass-through, optional user on the default guard, a rolled-back transaction on the default connection, and the status, headers (cookies masked), body (up to 200 KB), time and query count as the reply |
| `php/create_file.php` | Creates a new file with the content on stdin, refusing to overwrite (used by Extract Form Request, Generate test from the hub or the command, and Create migration; from Alt+Enter the IDE creates the test file itself) |
| `php/selection.php` | Writes `storage/openidea/selection.php` for *Run Selection in Tinker* |
| `php/tools.php` | Runs Pint, PHP CS Fixer and PHPStan for tools on save, one after the other, stopping at a shared deadline |
| `php/guidelines.md` | The guide written to `AGENTS.md` (or `.ai/guidelines` with Laravel Boost) when you connect AI agents |

`src/routetools.rs` reads what a route expects without running PHP: the fields of its Form Request `rules()` or inline `validate([...])`, its route model bindings and sample values, and writes the Pest or PHPUnit test; only the route's middleware comes from `route:list --json`. `src/runtime.rs` names the PHP install from the probe's `PHP_BINARY` and words the advice when PHP doesn't start.

When a script runs as root (as in many Docker images), it gives the files it creates back to the owner of the project folder.

## Logging

The plugin writes to the IDE log only when something fails, as a warning or an error, and each distinct message only once: a command, hub request or code action that fails, a php or Docker run that can't start or returns no result, a tool on save that fails or times out, a sync of `storage/openidea` that didn't take, a file the index couldn't read, a scan cut at its file limit and an index that took over two seconds. A healthy session logs nothing. See [Troubleshooting and logs](Troubleshooting.md).

## Related

- [Permissions](Permissions.md)
- [Privacy and security](Privacy-and-Security.md)
- [Status bar](Status-Bar.md)
- [Gutter usages](Gutter-Usages.md)
- [The hub](Hub.md)
- [Development](Development.md)

---
[← Wiki home](Home.md)
