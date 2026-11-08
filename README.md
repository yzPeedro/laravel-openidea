<p align="center">
  <img src="docs/media/banner.webp" alt="Laravel OpenIDEA" width="100%">
</p>

<p align="center">
  <a href="https://github.com/yzPeedro/laravel-openidea/actions/workflows/ci.yml"><img src="https://github.com/yzPeedro/laravel-openidea/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://github.com/yzPeedro/laravel-openidea/releases/latest"><img src="https://img.shields.io/github/v/release/yzPeedro/laravel-openidea?color=ff2d20&label=release" alt="Latest release"></a>
  <img src="https://img.shields.io/badge/OpenIDEA-0.1.38%2B-4f46e5" alt="OpenIDEA 0.1.38 and newer">
  <img src="https://img.shields.io/badge/Laravel-6%2B-ff2d20?logo=laravel&logoColor=white" alt="Laravel 6 and newer">
  <img src="https://img.shields.io/badge/built%20with-Rust%20%2B%20WebAssembly-orange?logo=rust" alt="Rust and WebAssembly">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-green" alt="MIT license"></a>
</p>

<p align="center">
  <b>The complete Laravel toolkit for <a href="https://github.com/l0ng-ai/openidea">OpenIDEA</a>.</b><br>
  Completion and navigation for every Laravel string, Blade and Eloquent intelligence, and a hub with Artisan, Tinker,<br>
  every resource of your app, live logs, a live audit of requests, queries and events, and an MCP server for AI agents.
</p>

https://github.com/user-attachments/assets/a653530b-f3d4-44e3-921e-bef9119cd659

<p align="center"><sub>An 87-second tour of 0.14: Live Audit, SQL to Laravel and back, Copy Query as MySQL, Form Request extraction, inspections with quick fixes, Eloquent chains, casts, the Cache browser, Tinker, and composer.json and phpunit.xml completion. <a href="docs/media/tour.mp4">Download the MP4</a>.</sub></p>

---

## Contents

- [Highlights](#highlights)
- [Installation](#installation)
- [Features](#features)
  - [Editor intelligence](#editor-intelligence)
  - [The Laravel OpenIDEA hub](#the-laravel-openidea-hub)
  - [Cache](#cache)
  - [Tinker](#tinker)
  - [SQL to Laravel](#sql-to-laravel)
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
| 🧠 **Eloquent that follows the chain** | `$order->items->`, `$order->items()->`, `Product::published()->`, `->first()->`: each step completes what it returns (collection methods, relation methods, query methods and scopes, the model again), and enum or value-object casts complete their own members, traits included. |
| 🧭 **Navigation** | Ctrl+click on a route name or URI opens the controller method; views, config keys, env keys and translations open where they are defined. Gutter icons link `route()` calls and views. |
| 🧩 **A hub for your whole app** | 40+ sections: routes, controllers, requests, models, scopes, observers, events and listeners, jobs, schedule, translations, config, env, bindings and more. Every section has filters, and every row opens its file. |
| 🐘 **Tinker in tabs** | Write PHP like in an editor, press Ctrl+Enter, get Tinker-style coloured output with dumps, SQL and timing. |
| 🔄 **SQL to Laravel and back** | Type SQL in any dialect and watch it turn into Eloquent or query builder code as you type, or swap sides to see the MySQL your Laravel code runs; run it in Tinker, copy it or insert it at the cursor. Right-click a query in the editor to copy it as MySQL. |
| 🔍 **Live Audit** | Requests, SQL with the line that ran it, events with payloads and listeners, jobs, cache, mail, HTTP client calls, gates, logs, exceptions and dumps, with N+1 and slow query insights. |
| 🗄️ **Cache browser** | The keys, values, sizes and expiry of the file, database and redis stores, page by page, with *Forget* and *Flush store*. |
| 🧾 **Config files too** | `composer.json` (packages and versions from `composer.lock`), `phpunit.xml` (attributes from your installed PHPUnit, env values from your config), `vite.config` entries and Laravel route names and translation keys in JavaScript and TypeScript. |
| 🤖 **AI agents that know your app** | A live MCP server with the hub's runtime data, Artisan, PHPStan, Pint, tests and Tinker behind an access level you choose (read, write or full), connected in one click next to Laravel Boost. |
| 🐳 **Docker-aware** | Artisan, Tinker and the recorder run with `php` or in a throwaway Docker Compose container, detected automatically. |

## Installation

**Requirements:** [OpenIDEA](https://github.com/l0ng-ai/openidea/releases/latest) 0.1.38 or newer and a Laravel project. Completion and inspections follow the Laravel version installed in `vendor/` (tested from Laravel 6 to 13); the runtime features are tested on Laravel 10 and newer. PHP or Docker Compose is needed for the runtime features (Artisan, Tinker, Live Audit).

1. **Download** `laravel-openidea-<version>.zip` from the [latest release](https://github.com/yzPeedro/laravel-openidea/releases/latest).
2. In OpenIDEA open **Settings › Plugins › ⚙ › Install Plugin from Package File…** and pick the zip.

   <img src="docs/media/settings-plugins-menu.webp" alt="Settings, Plugins, gear menu, Install Plugin from Package File" width="80%">

3. **Allow `process:php` or `process:docker`** in the plugin's permissions. They are optional: completion and navigation work without them, while Artisan, Tinker and Live Audit need one of them. On Windows, also allow `process:php-win` (the `php-win.exe` next to `php.exe`): the hub, Tinker, the Cache browser and the other background calls then run without flashing a console window.

   <img src="docs/media/settings-plugins.webp" alt="Plugin permissions in Settings" width="80%">

4. **Open a Laravel project** and click the Laravel icon at the bottom of the left tool bar.

<details>
<summary><b>Build from source</b></summary>

```sh
git clone https://github.com/l0ng-ai/openidea --branch v0.1.38
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

**Snippets**: about 200 PHP and Laravel snippets (`pubf`, `ctorp`, `foreach`, `hasmany`, `scope`, `rget`, `qwhere`, `cremember`, `postjson`, `it`…) expand with Tab where they make sense (outside a class, in a class body or in code), add the `use` statements they need, and Blade block directives come with their closing tag. Every snippet is listed in [docs/snippets.md](docs/snippets.md).

| Routes, with verb and URI | Eloquent columns from migrations |
| :---: | :---: |
| <img src="docs/media/editor-route-completion.webp" alt="Route name completion"> | <img src="docs/media/editor-column-completion.webp" alt="Column completion"> |
| **Traits, interfaces and base classes** | **Gutter links to routes and views** |
| <img src="docs/media/editor-trait-completion.webp" alt="Trait completion"> | <img src="docs/media/editor-gutter.webp" alt="Gutter icons"> |

<p align="center"><img src="docs/media/editor-member-completion.webp" alt="Model columns and relations after ->" width="70%"><br><sub>Model columns and relations after <code>-&gt;</code></sub></p>

- **Strings that mean something:** `route()`, `to_route()`, `redirect()->route()`, `view()`, `config()`, `env()`, `__()`, `trans()`, `@lang`, `Route::has`, `routeIs`, `Storage::disk`, `Cache::store`, `Queue`, `Log::channel`, `DB::connection`, `Mail::mailer`, `auth()->guard`, middleware, gates, policies and rate limiters.
- **Model members:** after `$order->` the model's columns (with their types) and relations, and after `$order->user->` the related model's columns. The model comes from typed parameters, `Order::create/find/first…`, `new Order`, `foreach ($order->items as $item)`, `/** @var Order $x */`, `$this` in a model and `auth()->user()`. Relations are listed apart from columns with their kind and target (`belongsTo → User`), and cast columns show what they become (`string → OrderStatus`). Ctrl+click on a relation opens the related model; on a cast column, the enum, or the class (or interface) its cast's `get()` returns, falling back to the cast class; on any other column, its migration.
- **Chains:** each step completes what it returns. `$order->items->` (hasMany, belongsToMany…) lists the collection's methods typed with the item model, `$order->items()->` the relation's own methods (`create`, `createMany`, `save` for hasMany and hasOne; `attach`, `sync`, `toggle`, `wherePivot` for belongsToMany; `associate`, `dissociate` for belongsTo) and the query ones, `Product::`, `Product::where(…)->`, `->latest()->` or a scope call the query methods with the model's local scopes first, and `->first()`, `->sole()`, `->find(…)` or `->get()` bring back the model or the collection. Calls with arguments, closures included, are followed.
- **Cast members:** `$order->status->` on a column cast to an enum completes `name`, `value` (backed enums) and the enum's methods, including those of the traits it uses; a column cast to a class (a value object returned by a cast's `get()`) completes its public properties and methods.
- **Model inheritance:** a model that extends another model (`class Ebook extends DigitalProduct`) inherits its parent's relations and scopes in completion and navigation.
- **Eloquent:** columns and relations in `where`, `with`, `has`, `whereHas`, `orderBy`, `create`, `update`, `fill` and `pluck`, model attributes (`#[Fillable]`, `#[Hidden]`, `#[Table]`…), `casts()`, scopes, and member snippets for `$fillable`, `$casts` and relations. Untyped closure parameters get their model too: `whereHas('posts', fn ($q) => $q->where('…'))` and `with(['posts' => fn ($q) => …])` complete the related model, `where(fn ($q) => …)` and `when()` the same model, `each(fn ($post) => $post->…)` the items, and chains through relations such as `$user->posts()->where('…')` the related model.
- **Gates:** Ctrl+click on the ability in `Gate::define('…')` lists where it is used: `Gate::allows`/`denies`/…, `authorize()`, `can()`/`cannot()`, `@can` and `can:` middleware.
- **Scopes:** Ctrl+click on `scopePublished()` (or a `#[Scope]` method) lists where it is called, and Ctrl+click on a call such as `Product::published()` or `$query->published()` opens its declaration.
- **Casts:** the values of `casts()` and `$casts` complete every cast Laravel ships (`boolean`, `datetime`, `decimal`, `encrypted:array`, `json:unicode`, `AsCollection`, `AsEnumCollection`…), and after the colon the arguments that fit: decimal places, date formats, `encrypted:` types and `AsBinary` formats.
- **Validation:** rule names inside `'required|…'` strings and rule arrays, plus custom rules.
- **Blade:** syntax highlighting, structure and folding, directives after `@` (including your own `Blade::directive` and `Blade::if`), `<x-…>` components, `<livewire:…>` tags, sections, stacks and assets.
- **OpenAPI and packages:** swagger-php `$ref`s, tags and security schemes, Sanctum token abilities, Spatie roles and permissions, Socialite drivers, broadcast channels, Inertia pages and Livewire components.
- **Types:** Laravel, package and project traits after `use`, interfaces after `implements` and base classes after `extends`, ranked by folder (models get `HasFactory` and `SoftDeletes` first, jobs get `ShouldQueue`, and so on).
- **Inspections and quick fixes:** unknown views, routes, rules, middleware and OpenAPI refs; N+1 queries (a relation read on each item of a `foreach` over a query that doesn't eager load it, fixed by adding `with()`); columns that don't exist in the model's table in `where`, `orderBy`, `pluck` and similar calls, with the closest column as the fix; `env()` outside `config/`, replaced by the `config()` key that reads the same variable; `create()` keys that `$fillable` leaves out, added to the model's `$fillable`; keys repeated in the same array (PHP keeps the last one), with a fix that removes the overwritten entry; keys missing between `.env` and `.env.example`, copied over (a new key goes to `.env.example` without its value); *Add translation*; *Generate @property PHPDoc* for models from migrations, casts and relations; *Laravel: Go to Related Class or View* for Livewire and Blade components.
- **Refactorings (Alt+Enter):** change a route's name in its `->name('…')` or a translation key in a `lang/` file, and *Rename route* or *Rename translation* updates every usage across PHP and Blade, and for translations the same key in the other locales; it works before and after saving. A view that no longer exists (renamed or moved to another folder) offers the closest existing view, in that usage or in every usage. *Extract the validation into a Form Request* turns `$request->validate([…])`, `request()->validate(…)` or `$this->validate($request, …)` into `app/Http/Requests/<Method><Model>Request.php` with `rules()` (and `messages()`/`attributes()`), types the parameter with it, calls `validated()` and tidies the imports (needs `process:php` or `process:docker` to create the file). From the right-click menu, *Laravel: Extract Form Request…* works with the cursor in the call or with the call selected, and asks for the class name first, filled in with the suggested one (a subfolder such as `Admin/StoreOrderRequest` sets the namespace).
- **Tools on save (opt-in):** set `formatOnSave` to `pint` or `php-cs-fixer` and turn on `phpstanOnSave` to format and analyse each PHP file you save, using the project's own `vendor/bin` tools and config (Larastan works through `phpstan.neon`). PHPStan errors show as warnings on their lines. They run in the background with a progress item in the status bar and stop after 15 seconds; a tool that times out or fails is reported once.
- **Tool configs:** completion for presets, rules and options in `pint.json`, rule sets and rules in `.php-cs-fixer.php`, and parameters, levels and includes (Larastan's too) in `phpstan.neon`, with NEON highlighting.
- **Pest:** dataset names in `->with('…')` and groups in `->group('…')`, `->groups('…')` and `#[Group('…')]`.
- **`composer.json`:** top-level keys, package names in `require`, `require-dev`, `conflict`… (installed packages from `composer.lock` with their description and version, the Laravel ecosystem and PHP extensions), version constraints from the installed version, `autoload` keys with PSR-4 namespaces and folders, script events and the usual Laravel script commands, `config` keys and values, `allow-plugins` from the installed Composer plugins and `extra.laravel` keys.
- **`phpunit.xml`:** elements and every `<phpunit>` attribute read from your installed `vendor/phpunit/phpunit/phpunit.xsd` (booleans and `executionOrder` values included), `<env>` and `<server>` names from `.env`, `.env.example` and the config files with their values (connections, stores, drivers, booleans…), suite names and test folders and files in `<directory>` and `<file>`.
- **Front-end:** files under `resources/` in the strings of `vite.config.js` or `.ts`, route names in `route('…')` (Ziggy) and translation keys in `__('…')`, `trans('…')`, `$t('…')`, `t('…')` and `trans_choice('…')` in JavaScript, TypeScript, JSX, TSX, Vue and Svelte files.
- **File templates:** *New File* offers a Laravel model, controller, Form Request, policy, job, event, listener, middleware and Pest test.
- **Helpers:** `auth()->` completes the guard methods and `auth()->user()` is your user model, without Intelephense's "Undefined method" (a stub the plugin keeps in `storage/openidea/ide-helpers.php`, ignored by git; turn it off with `ideHelpers`). Path helpers and `Storage` calls suggest the files and folders under their own folder or disk root; `logs()`, `app()`/`resolve()`, `url()`/`redirect()`, `old()`, `request()` and `$request->input()` complete channels, container ids, route URIs and validated fields.
- **Folio:** pages under the Folio paths of `FolioServiceProvider` are routes: their `name()` completes in `route()`, Ctrl+click opens the page, and the hub's Routes section lists every page with its URI.
- **Cached routes:** after `php artisan route:cache`, named routes from packages (Sanctum, Horizon, Filament…) are read from `bootstrap/cache/routes-v7.php`.
- **Pivot attributes:** after `->pivot->`, or the name given with `->as()`, the keys, `withPivot()` columns and timestamps of many-to-many relations.
- **`.env` files:** key and value completion (connections, stores, disks, channels, mailers, locales) with navigation to the config that reads each key.
- **File icons** from the Material Icon Theme for Laravel files.

### The Laravel OpenIDEA hub

One tool window for everything your app has. It refreshes from your files and opens every row in the editor. Every table section, Logs and Live Audit show 100 rows per page (50 to 500) with a pager that stays at the bottom; searching, filtering or sorting goes back to the first page.

**Everything about a model**: the gutter icon on a model's `class` line opens a hub tab with its table and migrations, relations, scopes, casts, factory and seeders, and every controller, Form Request, API Resource, policy, observer, job, event, listener, notification, mail, Livewire or Filament class, command, route and test that uses it.

**Search the whole hub**: the box above the sections searches every table at once (routes, models, tables and columns, views, config, env, translations, classes, events, listeners, jobs and the rest), shows the first matches of each section with a link to its file, offers the sections whose name matches, and *Show all* opens a section searched for the same text.

**In editor tabs**: *Open in editor* in the hub's header opens the current section (or, from the Overview, the whole hub) in an editor tab, and *Laravel: Open the Hub in an Editor Tab* does it from the command palette. Each tab keeps its own state, so you can drag tabs into a split to see two sections side by side. Ctrl+click on a cache key (`Cache::get('…')`, `cache('…')`, `cache()->remember('…')`…) opens the Cache section in a tab, searched for that key and expanded to show its value and when it expires. The tabs are small files in `storage/openidea/hub`, which the plugin writes with `process:php` (or `process:docker`).

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
| Develop | Artisan, Tinker, SQL to Laravel |
| HTTP | Routes, Controllers, Form Requests, API Resources, Middleware |
| Data | Models, Scopes, Observers, Casts, Enums, Database, Cache, Factories & Seeders |
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

### Cache

Pick a store and see its keys with their type, value, size and expiry, page by page, with search; a click shows the whole value. *Forget* removes one key and *Flush store* empties the store after a second click. The file, database and redis stores are listed whatever their driver configuration; the file store keeps only a hash of each key, and stores that can't be listed (array, memcached, dynamodb…) say why. It runs your app with `php` or Docker Compose like Tinker.

<img src="docs/media/hub-cache.webp" alt="Cache">

### Tinker

As many tabs as you like, with PHP highlighting, snippets and remembered code. **Ctrl+Enter** runs the tab and **Ctrl+Shift+Enter** runs the selection. Each statement shows its value the way Tinker does, with echo output, `dump()` calls, SQL queries and timing. Models resolve by their short name.

In a PHP file, select some code and pick **Laravel: Run Selection in Tinker** from the right-click menu: a terminal opens `artisan tinker` (with `php` or Docker Compose) with the file's `use` imports, dumps the value of each statement and stays open with the selection's variables defined.

<img src="docs/media/hub-tinker.webp" alt="Tinker">

### SQL to Laravel

Write a query in the dialect you have at hand (MySQL, PostgreSQL, SQL Server, SQLite, Oracle) and the Laravel code updates on the right as you type, highlighted like the editor. Switch between **Eloquent**, which uses the model of each table, and the **Query Builder**. Joins, nested `AND`/`OR`, `IN`, `BETWEEN`, `EXISTS` and subqueries, dates, aggregates, `GROUP BY`/`HAVING`, `LIMIT`/`TOP`/`FETCH`, locks, `UNION`, `INSERT` (with upserts and `INSERT IGNORE`), `UPDATE` (with `increment`), `DELETE` and `TRUNCATE` become the matching builder calls; anything without one falls back to `whereRaw` or `DB::raw`, and the notes under the code say where. The copy button puts the code on the clipboard and the insert button writes it at the cursor of the editor you were in, indented like the line (they need the optional `clipboard` and `editor.read`/`editor.write` permissions). The run button opens the code in a new Tinker tab and runs it; code that changes data asks first.

<img src="docs/media/hub-sql.webp" alt="SQL to Laravel">

The swap button turns it around: write Eloquent or query builder code and the MySQL it runs appears on the right as you type. The tables come from your models, soft-deleted rows are left out the way Eloquent does, local scopes (`scopeActive()` or `#[Scope]`) are applied from the model with their arguments, and variables become `?` placeholders listed in the notes, together with anything that isn't translated (eager loading, relation queries, the count query of `paginate()`).

<img src="docs/media/hub-sql-reverse.webp" alt="Laravel to MySQL">

In the editor, right-click a query and pick **Laravel: Copy Query as MySQL**: the statement under the cursor (or the selection) is copied as MySQL, translated the same way.

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

**Connect AI agents** installs the `laravel-openidea` MCP server and registers it in `.mcp.json` and, when their folders exist, in the Cursor, VS Code (Copilot), Junie and Gemini configs, with a short guide in `AGENTS.md`. Every call boots the app again, so answers follow your code as it changes, and the plugin keeps the server files in step with the plugin version.

What agents may do is up to you, with the `mcpAccess` setting:

| Level | Tools |
| --- | --- |
| **read** (default) | `app_overview` · `access_level` · `list_routes` · `list_classes` · `list_events_and_listeners` · `list_models` · `list_container_bindings` · `list_schedule` · `list_queues` · `list_artisan_commands` · `get_config` (secrets masked) · `database_schema` · `missing_translations` · `check_env_keys` · `list_tests` · `run_phpstan` · `read_logs` · `audit_recent` · `hub_sections` · `run_artisan` for read-only commands (`route:list`, `about`, `migrate:status`, `model:show`…) |
| **write** | adds `format_code` (Pint or PHP CS Fixer), `run_tests`, `query_to_sql` (the SQL of an Eloquent expression, captured with `DB::pretend()` without running it), `audit_recorder` and `run_artisan` for `make:*`, `migrate`, `db:seed`, cache and optimize commands |
| **full** | adds `run_tinker` and `run_artisan` for every other command (`migrate:fresh`, `db:wipe`, `key:generate`…) |

A tool above the level tells the agent to ask you, and connected clients get the new tool list when you change it. No tool installs, changes or removes the plugin, its settings, its grants or its files in `storage/openidea`.

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
| `unknownColumnInspection` | on | Report columns that don't exist in the model's table |
| `envCallInspection` | on | Report `env()` calls outside config files |
| `fillableInspection` | on | Report `create()` keys that `$fillable` leaves out |
| `nPlusOneInspection` | on | Report relations loaded once per item in a `foreach` |
| `envKeysInspection` | on | Report keys missing between `.env` and `.env.example` |
| `duplicateKeyInspection` | on | Report keys repeated in the same array |
| `formatOnSave` | `off` | Format the saved PHP file with `pint` or `php-cs-fixer` from `vendor/bin` |
| `ideHelpers` | on | Keep `storage/openidea/ide-helpers.php` so Intelephense types `auth()` and your user model |
| `mcpAccess` | `read` | What AI agents may do through the MCP server: `read`, `write` or `full` (see [AI agents](#ai-agents)) |
| `phpstanOnSave` | off | Analyse the saved PHP file with PHPStan (and Larastan) when the project has a `phpstan.neon` |

Live Audit reads two environment variables from your app: `OPENIDEA_AUDIT=false` turns the recorder off and `OPENIDEA_AUDIT_SHOW_PII=true` shows personal data.

## Privacy and security

- The plugin is a sandboxed WebAssembly component. It reads project files, and it runs processes only through the `process:php` and `process:docker` permissions you grant. It writes to the editor (`editor.read`, `editor.write`) and the clipboard (`clipboard`) only when you press the insert or copy button of SQL to Laravel, and only if you allowed it.
- The Live Audit recorder runs only in the `local`, `development` and `dev` environments. It lives in `storage/openidea`, which has its own `.gitignore`, and loads through a marked hook in `vendor/autoload.php`, so no tracked file changes. *Turn off* removes both.
- Passwords, tokens, keys, cookies and signatures are always masked in recorded payloads. Personal data such as e-mail, CPF, phone, documents and addresses is masked by default.
- Pint, PHP CS Fixer and PHPStan only run on save when you turn them on, and they're the project's own `vendor/bin` tools, run like Artisan.
- The MCP server masks secrets in config values and only reads at the default `read` level. `write` lets agents generate files, migrate, seed and run tests; `full` runs any PHP through Tinker, which can't be fenced in, so give it only to agents you trust. The level decides what the server offers, not what an agent with its own file or shell tools could do anyway.
- Nothing is sent anywhere: records stay in `storage/logs/openidea-audit.jsonl` on your machine.

## How it works

```
OpenIDEA ── WIT (openidea:plugin@1.0) ──► laravel-openidea.wasm (Rust)
  │                                         ├─ index/      cached indexes of routes, config, views, models, migrations…
  │                                         ├─ framework.rs rules, directives, casts and methods of the installed laravel/framework
  │  completion, definitions, gutter,       ├─ refs.rs     which helper, facade or directive points to which index
  │  code actions, Blade syntax             ├─ php.rs      PHP and Blade tokenizer with call context
  │                                         ├─ panels.rs   the hub (HTML view) and its sections
  │  web view messages ◄──────────────────► ├─ devtools.rs Tinker, Live Audit, debug tools, ecosystem, agents
  │                                         └─ runner.rs   php or Docker Compose
  └─ processes (php / docker) ──► php/tinker.php · php/Recorder.php · php/install.php · php/mcp.php
```

Failures go to the IDE log as warnings and errors, each distinct message once: a command, hub request or code action that fails, a php or Docker run that can't start or returns no result, a tool on save that fails or times out, a sync of `storage/openidea` that didn't take, a file the index couldn't read, a scan cut at its file limit and an index that took over two seconds. The plugin logs nothing else, so the log is an audit trail to attach to a bug report.

See [CONTRIBUTING.md](CONTRIBUTING.md) for the development setup, tests and conventions.

## Contributing

Issues and pull requests are welcome. Start with [CONTRIBUTING.md](CONTRIBUTING.md), and see the [roadmap](ROADMAP.md) for what's next. Please follow the [code of conduct](CODE_OF_CONDUCT.md), and report security issues as described in [SECURITY.md](SECURITY.md).

## License

[MIT](LICENSE) © Pedro Pessoa. File icons come from the [Material Icon Theme](https://github.com/material-extensions/vscode-material-icon-theme) (MIT, see `icons/LICENSE-material-icon-theme`).

Laravel is a trademark of Taylor Otwell. This project is not affiliated with Laravel, JetBrains or the OpenIDEA authors.
