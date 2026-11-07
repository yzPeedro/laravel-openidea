# Changelog

All notable changes to Laravel OpenIDEA. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses [semantic versioning](https://semver.org/).

## Unreleased

- Model members: after `$var->` the columns (with types) and relations of the variable's model, and after `$var->relation->` those of the related model. Models are inferred from typed parameters, `Model::create/find/first…`, `new Model`, `foreach` over relations, `@var` docblocks, `$this` in models and `auth()->user()`. Relations show their kind and target apart from columns; Ctrl+click on a relation opens the related model and on a column its migration.
- Cast columns show what they become (`status · string → OrderStatus`), and Ctrl+click on them opens the enum, or the class or interface the cast's `get()` returns (the cast class when it returns a scalar), resolved through `use` statements and the composer PSR-4 map.
- SQL to Laravel in the hub: SQL in any dialect (MySQL, PostgreSQL, SQL Server, SQLite, Oracle) becomes Eloquent or query builder code as you type, with copy and insert-at-cursor buttons (optional `clipboard`, `editor.read` and `editor.write` permissions).
- Model and relation indexes are only rebuilt when a model file changes, so completion stays within the editor's time budget while you type.
- The hub opens on Windows and macOS: it is now a web view loaded from the plugin's own files, so the IDE's content security policy no longer blocks its scripts and leaves it stuck on *Loading* ([#4](https://github.com/yzPeedro/laravel-openidea/issues/4)).

## 0.13.1

- Blade files with multibyte characters (such as `…` or `—`) before a directive or comment no longer crash the plugin, which also left the hub stuck on *Loading* ([#4](https://github.com/yzPeedro/laravel-openidea/issues/4)).

## 0.13.0

- Runs on official OpenIDEA 0.1.37 and newer, which ships the plugin APIs this plugin needs (views open files at a line, SVG tool window icons); no fork is needed anymore.
- Folio: pages under the paths of `FolioServiceProvider` (or `resources/views/pages`) are routes, with `{param}`, model binding and `...` segments; named pages complete in `route()` and open with Ctrl+click, and the hub's Routes section lists every page.
- Named routes from the route cache (`bootstrap/cache/routes-v7.php`), so package routes such as Sanctum, Horizon or Filament complete after `php artisan route:cache`; the hub shows their actions.
- Pivot attributes: after `->pivot->`, or the accessor set with `->as()`, the pivot keys, `withPivot()` columns and timestamps of `belongsToMany`, `morphToMany` and `morphedByMany` relations.

## 0.12.0

- Hub sections can be opened from messages (section, filters, search, Tinker snippets).
- Tinker resolves App\Models and App classes by short name even before composer dump-autoload.
- Live Audit names Tinker and Artisan runs, ignores eval frames and opens at the top with its insights.
- Route tests are matched by URI pattern.
- Filter selects keep their labels.

## 0.11.0

- AI Agents: a laravel-openidea MCP server (app overview, routes, events and listeners, models with relations, scopes and observers, bindings, schedule, queues, commands, config with secrets masked, schema, missing translations, logs, Live Audit records with N+1 and slow query insights, Tinker) connected in one click to Claude Code, Codex, Cursor, Copilot, Junie and Gemini with guidelines in AGENTS.md and CLAUDE.md, next to Laravel Boost (install, boost:install, boost:update).
- Ecosystem: starter kit, Sanctum, Wayfinder, Horizon, Reverb, Octane, Pulse, Telescope and other packages with details and actions.
- Gutter link from route() usages to the route, tests per route in the hub, Go to Related Class or View for Livewire and Blade components, Generate @property PHPDoc for models from migrations, casts and relations.

## 0.10.0

- Tinker in the hub: editor tabs with PHP highlighting, Ctrl+Enter to run (Ctrl+Shift+Enter for the selection), snippets, Tinker-style coloured results per statement with echo output, dump(), SQL and timing.
- Tabs are remembered.
- Live Audit: a local-only recorder (storage/openidea + vendor/autoload.php hook, no tracked file touched) streams requests, SQL with the calling line, events with payloads and listeners, jobs, cache, mail, notifications, HTTP client, gates, logs, exceptions and dumps, with type filters, request grouping and N+1, slow query and failure insights.
- Secrets and personal data are masked.
- Profiles from Clockwork and Debugbar storage, leftover debug calls (dd, dump, ray, ds, var_dump…) and the installed debug toolbox.
- Artisan output keeps its colours.

## 0.9.0

- Hub: Laravel icon in the sidebar.
- Listeners (every listener and the events it listens to: discovered handle() types incl. unions, Event::listen, subscribers, $listen, queued flag), Observers, Scopes (local and global), Casts, Enums, Form Requests, API Resources, Controllers, Service Providers, Container Bindings, Validation Rules, Factories & Seeders, Macros, Drivers & Connections, Frontend and All Classes sections.
- Filters on every section (locale, method, prefix, kind, folder, group…) with counts, sortable columns.
- Log level chips with counts (click for one level, Ctrl+click to combine).
- Completion of Laravel, package and project traits after use, interfaces after implements and base classes after extends, ranked by folder.

## 0.8.0

- Laravel OpenIDEA hub: overview, Artisan, routes, models, database, views, components, config, env, translations, middleware, gates and policies, queues and workers, scheduling, events, mail and notifications, tests and live logs, with Material icons.
- Every row opens its file.
- Actions run in the terminal or show their output in place.
- Artisan runs in a throwaway Docker container when the project has a compose file.
- Commands declared with #[Signature] and generator commands are listed.

## 0.7.0

- Tool windows: Artisan (every command of the app and its packages, arguments and options as a form, run in the IDE terminal, history.
- Php or docker compose), Laravel Logs (live tail of storage/logs with level filters, search and stack traces), Routes (every route with method, URI, name, action and file).

## 0.6.0

- Blade highlighting and {{-- --}} comments (no HTML mode taking over).
- Material Icon Theme icons for Laravel files.
- .env completion of keys and values (connections, stores, disks, channels, mailers, locales, booleans) with navigation to the config that reads them.
- Column completion for array keys still being typed in create() and update().

## 0.5.0

- Laravel file icons (Blade, artisan, routes, controllers, requests, policies, migrations, seeders, factories, providers, resources, middleware).
- Blade directive completion after @, including custom Blade::directive and Blade::if.
- Ctrl+click on a route name or URI opens its controller method.

## 0.4.0

- Blade language with its own highlighting, structure and folding.
- Model attributes (#[Fillable], #[Hidden], #[Guarded], #[Table]…).
- Model member completion ( with every column, casts() from migrations, belongsTo/hasMany from foreign keys).
- Relation items no longer insert ().

## 0.3.0

- OpenAPI (swagger-php attributes): $ref completion, navigation and unknown-ref inspection, tags, security schemes, path params, required properties, in/type/format values.
- Sanctum guards and token abilities, Spatie roles and permissions, Socialite drivers, Artisan commands, broadcast channels, queue names.

## 0.2.0

- Eloquent columns and relations, migrations, validation rules, middleware, gates and policies, disks, cache stores, queues, log channels, guards, Blade components, sections, stacks, Livewire, Inertia, assets, controller route markers, add-translation quick fix.

## 0.1.0

- Routes, views, config, env and translations: completion, Go to Definition, view gutter markers, unknown view/route inspections, Artisan commands and run configurations.
