# Changelog

All notable changes to Laravel OpenIDEA. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses [semantic versioning](https://semver.org/).

## Unreleased

- Completion and inspections follow the Laravel version the project has installed, read from `vendor/laravel/framework`, instead of a fixed list written for Laravel 10 and newer. Validation rules, Blade directives, cast types, the built-in middleware aliases, container ids, the methods after collection, query and relation chains, and the Laravel classes, interfaces and traits after `extends`, `implements` and `use` are the ones that version has, so a rule or directive added in a later version is not suggested, and is reported as unknown, in an older project. Tested with Laravel 6, 8, 9, 10, 11, 12 and 13.
- Ctrl+click on a validation rule opens its `validate…` method in the framework.
- The model member snippet for casts writes the `protected $casts` property on Laravel versions without the `casts()` method.
- Directives from Livewire, Volt, Inertia, Ziggy and Spatie Permission are only suggested when the package is installed.
- Without `vendor/` (before `composer install`) the plugin falls back to the built-in lists.
- On Windows the hub, Tinker, the Cache browser, Live Audit, the MCP sync and the other background PHP calls run through `php-win.exe` when the new optional `process:php-win` permission is allowed, so no console window flashes on screen. Without it the plugin keeps using `php`.
- A search box for the whole hub: it searches every table section at once, lists the first matches of each with a link to the file, the sections whose name matches, and *Show all* to open a section with the same search.
- Hub sections, and the whole hub, open in editor tabs (*Open in editor* in the hub's header, or *Laravel: Open the Hub in an Editor Tab*), each with its own state, so two sections can sit side by side in an editor split. Ctrl+click on a cache key opens the Cache section in a tab, searched for the key and expanded to show its value and expiry.
- About 200 PHP and Laravel snippets (class members, control flow, models and relations, queries, controllers, Form Requests, jobs, middleware, policies, migrations, routes, helpers, PHPUnit and Pest), offered only where they fit and adding the imports they need, plus Blade block directives with their closing tag. They are listed in `docs/snippets.md`, generated from the same table.
- `namespace` completes with the PSR-4 namespace of the file's folder, read from `autoload` and `autoload-dev` in `composer.json`.
- Values compared with an enum-cast column are checked: `Order::where('status', 'refused')`, `where('status', '!=', …)`, `orWhere`, `whereNot`, `firstWhere` and the `whereIn` family warn when the backed enum has no such value, with a quick fix to the closest one, and the value argument completes the enum's values. The `enumValueInspection` setting turns it off.

## 0.14.1

- Listed in the OpenIDEA marketplace (Settings › Plugins › Available): the plugin has its own icon, the Frameworks, Tools and AI categories and a fuller description.
- Releases also ship `pedro.laravel-openidea-<version>.oiplugin`, the signed package the marketplace installs.

## 0.14.0

- Requires OpenIDEA 0.1.38 or newer (plugin API 1.2), which the Form Request name prompt uses.
- Model members: after `$var->` the columns (with types) and relations of the variable's model, and after `$var->relation->` those of the related model. Models are inferred from typed parameters, `Model::create/find/first…`, `new Model`, `foreach` over relations, `@var` docblocks, `$this` in models and `auth()->user()`. Relations show their kind and target apart from columns; Ctrl+click on a relation opens the related model and on a column its migration.
- Cast columns show what they become (`status · string → OrderStatus`), and Ctrl+click on them opens the enum, or the class or interface the cast's `get()` returns (the cast class when it returns a scalar), resolved through `use` statements and the composer PSR-4 map.
- SQL to Laravel in the hub: SQL in any dialect (MySQL, PostgreSQL, SQL Server, SQLite, Oracle) becomes Eloquent or query builder code as you type, with copy and insert-at-cursor buttons (optional `clipboard`, `editor.read` and `editor.write` permissions).
- SQL to Laravel works both ways: the swap button turns Eloquent or query builder code into MySQL as you type, with the models' tables, soft deletes and local scopes applied and a note for what isn't translated; the run button opens the Laravel code in a new Tinker tab and runs it, asking first when it changes data.
- *Laravel: Copy Query as MySQL* in the editor's context menu copies the query under the cursor (or the selection) as MySQL.
- *Laravel: Run Selection in Tinker* in the editor's context menu opens `artisan tinker` in a terminal (php or Docker Compose) with the file's imports, dumps each statement of the selection and stays open with its variables.
- Inspections with quick fixes: N+1 queries (a relation read on each item of a `foreach` over a query that doesn't eager load it → add `with()`), columns that don't exist in the model's table (→ the closest column), `env()` outside `config/` (→ the `config()` key that reads the same variable), `create()` keys outside `$fillable` (→ add them to the model) and keys missing between `.env` and `.env.example` (→ copy them over, without the value when going to `.env.example`). Each one can be turned off in the plugin's settings.
- Refactorings with Alt+Enter: after changing a route's `->name()` or a translation key, *Rename route* / *Rename translation* updates every usage (and the key in the other locales), before or after saving; a renamed or moved view offers the closest existing view in one usage or in all of them; *Extract the validation into a Form Request* creates the request class from `validate()` and rewrites the controller. *Laravel: Extract Form Request…* in the editor's context menu works with the cursor in the call or the call selected, and first asks for the class name, filled in with the suggested one; a subfolder such as `Admin/StoreOrderRequest` sets the namespace.
- Tools on save, off by default: `formatOnSave` runs Pint or PHP CS Fixer on the saved PHP file and `phpstanOnSave` runs PHPStan (and Larastan, through your `phpstan.neon`) on it, with its errors as warnings in the editor. They run with php or in Docker Compose as a status bar task of up to 15 seconds; a tool that times out or fails is reported once.
- Completion in tool configs: presets, rules and their options in `pint.json`; rule sets and rules in `.php-cs-fixer.php` and `.php-cs-fixer.dist.php`; parameters (Larastan's included), levels and extension includes in `phpstan.neon` and other `.neon` files, which are now highlighted.
- Pest: dataset names in `->with('…')` and group names in `->group('…')`, `->groups('…')` and `#[Group('…')]`, from the project's tests.
- File templates for Laravel classes in *New File*: model, controller, Form Request, policy, job, event, listener, middleware and Pest test.
- `auth()->user()`, `auth()->check()`, `auth()->id()` and the other guard methods no longer show "Undefined method" in Intelephense, and complete after `auth()->`: the plugin keeps a stub in `storage/openidea/ide-helpers.php` (ignored by git) that types `auth()` as the auth manager and its guard, with `user()` returning your user model from `config/auth.php`. The `ideHelpers` setting turns it off.
- Path helpers suggest the files and folders under their own folder: `base_path()`, `app_path()`, `config_path()`, `database_path()`, `lang_path()`, `public_path()`, `resource_path()` and `storage_path()`, the same methods on the application (`app()->storagePath()`, `App::publicPath()`…), and `Storage` calls (`get`, `put`, `exists`, `delete`, `url`, `path`, `files`, `copy`, `move`…) from the root of the default disk or of `Storage::disk('…')`, read from `config/filesystems.php`.
- More helper arguments complete: `logs('…')` (log channels), `app('…')` / `resolve('…')` / `app()->make('…')` (container ids), `url('…')`, `redirect('…')` and `->to('…')` (route URIs), and `old('…')`, `request('…')` and `$request->input()`, `get()`, `boolean()`, `has()`, `only()`… with the fields validated in the file or in its Form Request (in Blade, every Form Request's fields).
- MCP server: every call boots the app again, so agents see routes, models, config and code as they are now, and the server picks up plugin updates without restarting the agent. The plugin keeps `storage/openidea` in step whenever you focus an editor.
- MCP access levels (`mcpAccess`, read by default): read gives the inspection tools, now with `list_classes` (controllers, requests, enums, jobs, policies… by kind), `check_env_keys`, `list_tests`, `run_phpstan` and read-only Artisan commands; write adds `make:*`, `migrate`, `db:seed`, cache commands, `format_code`, `run_tests`, `query_to_sql` (the SQL of an Eloquent expression, captured with `DB::pretend()` without running it) and `audit_recorder`; full adds `run_tinker` and every Artisan command. Agents are told when a tool needs a higher level, connected clients get the new tool list when it changes, and no tool installs, changes or removes the plugin, its files or its settings. The AI Agents page shows the level and what each one unlocks.
- MCP `list_models` and `list_classes` find every class in `app/`, not only the ones in Composer's classmap.
- Model and relation indexes are only rebuilt when a model file changes, so completion stays within the editor's time budget while you type.
- Untyped closure parameters get their model: in `whereHas('posts', fn ($q) => $q->where('…'))`, `whereDoesntHave()`, `with(['posts' => fn ($q) => …])` and nested closures the related model's columns and relations complete; in `where(fn ($q) => …)`, `when()` and `tap()` the same model's; and in `each()`, `map()`, `filter()`… `$item->` completes the items' columns and relations. Chains through relations, such as `$user->posts()->where('…')`, complete the related model.
- Ctrl+click on the ability in `Gate::define('…')` lists where it is used: `Gate::allows`/`denies`/…, `authorize()`, `can()`, `@can` and `can:` middleware.
- Relation and query chains complete what they return: after `$order->items->` (hasMany, belongsToMany…) the collection's methods with the item model in their types, after `$order->items()->` the relation's own methods (`create`, `createMany`, `save` for hasMany/hasOne; `attach`, `sync`, `toggle`, `wherePivot` for belongsToMany; `associate`, `dissociate` for belongsTo) and the query ones, after `Product::`, `Product::where(…)->`, `->latest()->` or a scope call the query methods and the model's local scopes, and after `->first()`, `->sole()`, `->find(…)`… or `->get()` the model or the collection again. Calls with arguments, closures included, are followed.
- Local scopes both ways: Ctrl+click on `scopePublished()` (or a `#[Scope]` method) lists where it is called, and Ctrl+click on a call (`Product::published()`, `$query->published()`, `Product::query()->published()`) opens its declaration.
- Models that extend another model (`class Ebook extends DigitalProduct extends Product`) are recognised, and inherit the parent's relations and scopes in completion and navigation.
- `composer.json` completion: top-level keys; package names in `require`, `require-dev`, `conflict`… (installed packages from `composer.lock` with their description and version, the Laravel ecosystem and PHP extensions); version constraints from the installed version; `autoload` keys, PSR-4 namespaces and folders; script events and the usual Laravel script commands; `config` keys and values, `allow-plugins` from the installed Composer plugins; `extra.laravel` keys and `dont-discover` packages.
- `phpunit.xml` completion read from the project's own `vendor/phpunit/phpunit/phpunit.xsd`: elements, every `<phpunit>` attribute with booleans and `executionOrder` values, `<env>` / `<server>` names from `.env`, `.env.example` and the config files, their values (connections, stores, drivers, booleans…), suite names and test folders and files in `<directory>` / `<file>`.
- Front-end completion: files under `resources/` in the strings of `vite.config.js` / `.ts` (`input`, `refresh`), route names in `route('…')` (Ziggy) and translation keys in `__('…')`, `trans('…')`, `$t('…')`, `t('…')`, `trans_choice('…')` and `wTrans('…')` in JavaScript, TypeScript, JSX/TSX, Vue and Svelte files.
- The plugin declares the `storage` permission it needs: Tinker tabs, the Artisan history and the SQL to Laravel state are now saved between sessions.
- Live Audit sends at most ~900 KB per update, dropping the oldest records of a very large first load, so the IDE no longer rejects it and the list stays live.
- *Laravel: Open Tinker*, *Laravel: Artisan serve*, *Artisan optimize:clear* and *Artisan migrate* run with Docker Compose when the project uses it, like the rest of the plugin.
- Inspection for keys repeated in the same array (the last value wins), with a quick fix that removes the overwritten entry. The `duplicateKeyInspection` setting turns it off.
- Cast values in `casts()` and `$casts` complete every cast Laravel ships (`boolean`, `datetime`, `decimal`, `encrypted:array`, `json:unicode`, `AsCollection`…) instead of the columns, and after the colon the arguments that fit: decimal places, date formats, `encrypted:` types, `json:unicode` and `AsBinary` formats.
- Hub *Cache* section: the keys, values, sizes and expiry of the file, database and redis stores, page by page, with search, the whole value on click, *Forget* per key and *Flush store*. Stores that can't be listed (array, memcached, dynamodb…) say so.
- Hub pagination: every table section, Logs and Live Audit show 100 rows per page (50 to 500), with first, previous, next and last page in a pager that stays at the bottom; searching, filtering or sorting goes back to the first page, and moving between pages turns Follow off.
- Terminals opened from the hub and the editor are titled with the full command they run (`artisan test --filter=…`, `artisan tinker`, `composer require …`).
- Failures are written to the IDE log as warnings and errors (failed commands, hub requests and code actions, php or Docker runs with no result, tools on save, `storage/openidea` syncs, unreadable files, scans cut at their file limit, slow index builds), each distinct message once. The routine info messages are gone.

## 0.13.2

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
