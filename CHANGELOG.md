# Changelog

All notable changes to Laravel OpenIDEA. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses [semantic versioning](https://semver.org/).

## Unreleased

- Send request: when **Roll back database changes** is on and no transaction can be started on the default connection, the request is no longer sent. The panel says *Nothing was sent: …* with the reason, and turning the option off sends it for real after the second click. Before, the failure was ignored and the route ran without a transaction, so a `POST`, `PUT`, `PATCH` or `DELETE` could write to the database while rollback was on. A failure to count the queries no longer turns the rollback off: the response then says *queries not counted*.

## 0.16.0

- The migrations are followed in order: columns removed by `dropColumn()`, `dropMorphs()`, `dropTimestamps()`, `dropSoftDeletes()`, `dropRememberToken()` and `dropConstrainedForeignId()`, columns renamed by `renameColumn()` and tables removed or renamed by `Schema::drop()`, `Schema::dropIfExists()` and `Schema::rename()` leave completion, the unknown column inspection, the hub and *Generate @property PHPDoc*. The `down()` method of a migration is ignored. Columns are listed in the order the migrations add them, so `id` comes first.
- Hub › Database: **Indexes** and **Foreign keys** columns, read from `index()`, `unique()`, `primary()`, `fullText()`, `foreignId()->constrained()`, `foreign()->references()->on()` and the rest. **Compare with database** boots the app and lists pending migrations, tables only in the migrations or only in the database, and missing columns and indexes. **Rows** shows the rows of a table, read only and paged, with secret and personal data columns masked.
- Hub › Schema diagram: a new section draws every table with its primary, foreign and unique keys and the model that uses it, linked by foreign keys (solid) and model relations without one (dashed). Drag to pan, scroll to zoom, **Fit**, filter by table, model or column, hover a table to highlight its links, click its title to open the migration.
- Security inspections: `$request->all()` (and `input()`, `request()->all()`, `Request::all()`) passed to `create()`, `update()`, `fill()` and the other mass assignment methods, with *Use validated()* on a Form Request; variables in the SQL of `whereRaw()`, `DB::select()`, `DB::raw()` and the other raw calls; request data printed with `{!! !!}` in Blade, with *Escape with {{ }}*. These are the first inspections of Blade files. Setting `securityInspection`.
- Hub › Security: a new section with those findings for the whole project, models with `$guarded = []`, `Model::unguard()`, the URIs excluded from the CSRF check, and **Check routes**, which runs `route:list --json` and lists POST, PUT, PATCH and DELETE routes without an auth middleware.
- Missing translations: a key that some locales have and others don't is reported, with a fix that adds it to each missing locale's file with the existing text. Hub › Translations has a **Missing in** column and filter. Setting `missingTranslationInspection`.
- Hub › Unused: named routes, views, translation keys, config keys of your own files, events without listeners and jobs nothing dispatches, from the strings and class names used in PHP, Blade and `resources/js`.
- Upgrade: code that breaks on the next major Laravel version is underlined as info, from 63 changes of the official upgrade guides (6 → 7 to 12 → 13), with a fix for plain renames such as `Bus::dispatchNow` → `dispatchSync`. Hub › Upgrade lists every version ahead. Setting `upgradeInspection`.
- Live Audit: **N+1?** and **Slow query** cards say *No index on orders.status* when none of the columns the query filters on starts an index in the migrations, with **Create migration**, which writes and opens a migration that adds the index. **EXPLAIN** shows the plan of a `select` (MySQL, MariaDB, PostgreSQL, SQLite) in the output drawer, from a card or an expanded query.
- Hub › Routes: **Request** on each route opens a request panel that runs the route inside the app through its HTTP kernel, without a server: method, the URI's `{parameters}`, query, a JSON body prefilled from the Form Request `rules()` or the inline `validate()`, **Act as user id**, **Accept JSON** and **Roll back database changes** (on by default; with it off, a write asks for a second click). The response shows status, time, query count, size, redirect, headers (cookies masked) and body. CSRF checks are off for that call only. **Copy as cURL** copies the same request.
- Generate test: **Test** on a route, **Generate test** in the request panel, Alt+Enter on a controller method (*Generate a feature test for POST /products*) or *Laravel: Generate Feature Test…* in its right-click menu writes `tests/Feature/<Controller><Method>Test.php` in Pest or PHPUnit and opens it: a valid request built from the validation rules, with models from their factories for route bindings and `exists:` rules and `actingAs()` when the route has an auth, `verified` or `can:` middleware, plus one case per required field. From Alt+Enter the IDE creates the file, so one undo removes it. An existing file is opened instead of overwritten. `Route::resource()` actions get their own verb and URI.
- Hub › Overview: a chip names the PHP that runs the app and its version (`Herd · PHP 8.3.12`, `Sail · PHP 8.4.1`, `pvm · PHP 8.5.12`…), with the binary and `php.ini` in its tooltip. When PHP can't start, **PHP not found** and a card say what to do: install PHP or Herd and restart OpenIDEA, or switch to the Docker runner when the project has Sail or a Compose file.
- Related files: a Laravel icon in the gutter links the files that belong together, with a list to pick from or a direct jump when there is only one. A model's class line lists its migrations, factory, seeders, scopes, JSON Resources, policy, observers, Form Requests, controller, events and their listeners, jobs, mail and notifications that take it, enum and class casts, and the models it has relations with; each of those files leads back to the model and the rest of the family. A controller method lists its routes and the tests that call them (this replaces the route icon of controller methods). A Blade view gets the icon on its first line, leading to the controllers, routes, Livewire and Blade components, Mailables and Notifications that render it, and those classes lead to their views. Events and listeners lead to each other. A model's class line no longer shows the usages icon.

## 0.15.1

- *Rename route* and *Rename translation* are offered after the file was saved, autosave included. On Windows OpenIDEA's file watcher reports the save before the save itself, which refreshed the plugin's copy of the file first, so the rename was lost. Renaming twice before applying keeps the first name, and renaming back to the original forgets the rename.
- `$this->app->make('…')`, `$this->app->path('…')` and the other container calls on `$this->app` complete container ids and paths, as in a service provider, and `$this->request->input('…')` completes request fields. They were listed but never recognised.
- The N+1 quick fix on a loop over `Post::paginate()`, `Post::cursor()` or another collecting call made right after the model writes `Post::with('…')->paginate()`. It used to insert `->with('…')` before the `::`.
- The unknown validation rule inspection checks every rule of a pipe-separated string, and rules with parameters: `'strng|required'` and `'requird|max:255'` are reported. It used to look only at the last rule, and only when it had no parameters. A `|` inside a `regex:` pattern doesn't start a new rule.
- Live Audit masks a value when a whole word of its key is a secret or personal data word (`password_confirmation`, `accessToken`, `user_email`, `date_of_birth`), instead of when the key merely contains one, so `footprint`, `compass`, `accepted_terms` and `org` are no longer masked. Keys are split at separators, camelCase humps and digits. `passport` is now personal data, shown with `OPENIDEA_AUDIT_SHOW_PII=true`. An installed recorder keeps the old rules until it is turned off and on again in the hub.
- Hub: the Artisan command list is read again when a command class under `app/` or the installed packages change, the section asks for it each time it opens, and a **Refresh** button reads it by hand; before, it was read once per session. Array arguments say `space separated`, as they are split, and array options `comma separated`. Counts read `1 item`, `1 key`, `1 record`, `1 query` and `1 entry` instead of `1 items`. Tinker and AI Agents no longer show the header's filter box, which did nothing there.
- *Run Selection in Tinker* keeps its terminal open when the selection fails. A PHP `Error` in the selection, such as an unknown class, ended `artisan tinker` (PsySH only catches exceptions in included files) and OpenIDEA closed the tab at once; the error is now shown as an `ErrorException` before the Tinker prompt. A selection with a syntax error opens no terminal and says *The selection isn't valid PHP: …*. `use` statements in the selection are moved to the top of the script.

## 0.15.0

- Completion and inspections follow the Laravel version the project has installed, read from `vendor/laravel/framework`, instead of a fixed list written for Laravel 10 and newer. Validation rules, Blade directives, cast types, the built-in middleware aliases, container ids, the methods after collection, query and relation chains, and the Laravel classes, interfaces and traits after `extends`, `implements` and `use` are the ones that version has, so a rule or directive added in a later version is not suggested, and is reported as unknown, in an older project. Tested with Laravel 6, 8, 9, 10, 11, 12 and 13.
- Ctrl+click on a validation rule opens its `validate…` method in the framework.
- The model member snippet for casts writes the `protected $casts` property on Laravel versions without the `casts()` method.
- Directives from Livewire, Volt, Inertia, Ziggy and Spatie Permission are only suggested when the package is installed.
- Without `vendor/` (before `composer install`) the plugin falls back to the built-in lists.
- On Windows the hub, Tinker, the Cache browser, Live Audit, the MCP sync and the other background PHP calls run through `php-win.exe` when the new optional `process:php-win` permission is allowed, so no console window flashes on screen. Without it the plugin keeps using `php`.
- Clicking a model in the hub's Models section shows everything that defines or uses the model: its table and migrations (the one that creates it, the ones that change it and the foreign keys pointing at it), relations, scopes, enum casts, factory and seeders, controllers, Form Requests, API Resources, policies, observers, middleware, jobs, events, listeners, notifications, mail, Livewire, Filament and Nova classes, commands, routes and tests, each opening its file. Mentions in comments and `use` imports don't count.
- A Laravel icon in the gutter next to every class in `app/` and `database/` and next to every model scope opens the list of every place that uses it to pick one; Ctrl+click on the class name lists them too. Comments, strings and `use` imports don't count. The usages come from a document indexer, so the IDE keeps them up to date in the background; this needs OpenIDEA 0.1.40 or newer, the first release that accepts the marker icon's `markers` and `tooltip`, refuses half renames of indexed code identifiers and finds indexed usages on Windows.
- Facades, enums and classes with only static methods complete with `::` (`Cache::`, `OrderStatus::`), and enums and cast classes complete as `OrderStatus::class` in `$casts` and `casts()`, with the `use` import added. The facades are read from the installed Laravel (`Facade::defaultAliases()`, or the `aliases` of `config/app.php` on Laravel 8 and older).
- Requires OpenIDEA 0.1.40 or newer (plugin API 1.4), for the plugin's own gutter icon, the usages index and the Laravel version of each window.
- The status bar shows the Laravel version of each window's own project, set when a file is opened or focused or the hub is used there. It used to show the first Laravel project opened in every window.
- Opening a large project, or a second Laravel window, no longer restarts the plugin. A project is indexed as a background task the first time any feature touches it, which OpenIDEA lets run past the five-second limit of a single call; later refreshes run in slices of about a second and a half. The Laravel classes and packages the plugin looks for are checked without reading their files (reading the framework went from about 4 to 2 seconds on a large project), and the inspections' project facts are kept per project, so switching windows doesn't rebuild them. *Laravel: Refresh Index* stops after a few seconds and lets the rest finish in the background.
- Changes to `.env`, `vendor/composer/installed.json`, `bootstrap/cache/routes-v7.php` and the other single files the plugin reads are picked up when they are made outside the editor too. OpenIDEA doesn't report changes to git-ignored files, so a key added to `.env` from a terminal didn't show up in `env()` completion until the IDE restarted.
- *Laravel: Artisan optimize:clear* and *Laravel: Artisan migrate* run in a terminal tab like *Open Tinker* and *Artisan serve*. They used to wait for PHP inside the plugin, and a command slower than five seconds (`optimize:clear` takes about seven on a Laravel 8 project with Xdebug) restarted the plugin and stopped the gutter's usages until the window was reloaded.
- Relation snippets in a model write the relations your migrations suggest, with real models and method names: `belongsTo` from the table's `*_id` columns, `hasMany` / `hasOne` from tables that point at the model, `belongsToMany` from pivot tables, `morphTo` from `*_type` / `*_id` pairs and `morphMany` / `morphOne` from `*able` columns, leaving out relations the model already has. When nothing fits, they offer a choice of the project's models.
- Factories: after `User::factory()->` the factory's own states come first, then the Factory methods of the installed Laravel, and `create()` / `make()` lead back to the model's members; `User::` offers `factory()` when the model has one, and the `hasfactory` snippet adds the trait. Ctrl+click on `factory` in `User::factory()` opens the model's factory (from `#[UseFactory]`, `newFactory()` or the naming convention), or the framework's `factory()` method when there is none.
- Typed closures for callbacks: in a callback argument, `fn` or `function` completes the closure with its parameter typed and imported, such as `fn (User $user) =>` in `->map()`, `->each()`, `->filter()` or `afterCreating()`, `fn (Builder $query) =>` in `where()`, `whereHas()` or `when()`, `fn (Collection $users) =>` in `chunk()`, model events in `booted()`, `fn (Blueprint $table) =>` in `Schema::create()`, and `DB::transaction()`, `Route::group()`, `Cache::remember()`, `Gate::define()`, `Event::listen()`, `RateLimiter::for()` and more.
- The hub's PHP calls (Tinker, the Cache browser, Live Audit, the hub tabs and the rest) work again on OpenIDEA 0.1.39, which accepts process arguments of at most 4096 bytes: long scripts now travel on stdin.
- Picking a completion inside a string replaces the whole value (or the current rule or middleware), so a selected value is replaced instead of kept after the new one.
- Inspections run while you type, so the enum value warning and the others appear and go away without saving or reopening the file.
- The file cache store finds a key by its name when you search for the whole key, so Ctrl+click on a cache key shows its value and expiry there too.
- Typing after an accented character no longer crashes the plugin.
- *Add to $fillable* writes the key inside the array of a `#[Fillable([...])]` attribute instead of after it, and `#[Guarded([...])]` is read the same way.
- Live inspections keep running after a file is saved; before, a save left them waiting until the project was indexed again.
- The model gutter icon appears even when the hub tab files were never written in that project: the icon writes them itself, and a denied PHP permission is logged.
- Ctrl+click on `HasFactory` in a model opens the model's factory, or the `HasFactory` trait when the model has none.
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
