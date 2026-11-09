# Troubleshooting and logs

When something doesn't work, the IDE log usually says why: the plugin writes every failure there, once, and nothing else. This page explains what gets logged, lists the common problems with their fixes, and shows how to report a bug.

## The IDE log

The plugin writes to OpenIDEA's log, tagged with its id (`pedro.laravel-openidea`), only when something fails:

| Logged as | Message starts with | What failed |
| --- | --- | --- |
| warning or error | `command <id>:` | A `Laravel:` command, such as `command copy-query-sql: …` |
| warning or error | `hub request on <view>:` / `hub <kind>:` | A request from the hub, or a problem a hub script reported |
| warning or error | `code action "<title>":` | Resolving a code action you picked, such as a rename that found no usage |
| error | `<tool> on save: no results for <file>: exit code …` | Tools on save produced no answer, with the exit code and the last lines of the output |
| warning | `<tool> on save: failed on <file>: …` / `timed out after 15 seconds on <file>` | A tool on save that failed or ran out of time |
| error or warning | `IDE helper sync:` / `MCP server sync:` / `hub tabs:` | A sync of `storage/openidea` that didn't take |
| warning | `reading <path>:` | A file the index couldn't read |
| warning | `index: <glob> matched N files or more, so only the first N were read` | A scan cut at its file limit |
| warning | `index: indexing <kind> took N ms (M entries)` / `index: reading vendor/laravel/framework took N ms` | An index or the framework that took over two seconds |
| error | `subscribing to …` / `listing the open projects` | The plugin couldn't set itself up |

Rules:

- **Each distinct message once.** The plugin remembers the last 512 messages it wrote and skips repeats, so a failure that happens on every keystroke shows once.
- **Cut at 4000 characters.**
- **Errors are failures, warnings are refusals.** A denied permission or a missing file is a warning; a process that ran and failed is an error. A cancelled task logs nothing.
- **A healthy session logs nothing.** Opening files, saving, refreshing the index: no info lines. The log is an audit trail you can attach to a bug report.

A failed PHP run includes its exit code (or "killed by a signal") and the last eight non-empty lines of its error output and of its standard output.

### Notifications

Tools on save also show a notification, once per session for each kind of problem: "Laravel OpenIDEA couldn't run the tools on save", "&lt;tool&gt; took longer than 15 seconds" and "&lt;tool&gt; failed on save". Commands show their error in the IDE's usual way.

## Common problems

### Nothing happens in a Laravel project

No completion, no status bar item, an empty hub. The plugin only handles a project whose root has an `artisan` file and a `composer.json` that mentions `laravel/framework` or `laravel/laravel`. Open the folder that contains `artisan` as the project, not a parent or a subfolder. A file belongs to the open project with the longest matching root.

### Completion or navigation shows old data

The indexes refresh on save, on changes the IDE reports, on git branch switches, and when `.env`, `.env.example`, `composer.lock`, `vendor/composer/installed.json`, `bootstrap/cache/routes-v7.php`, `bootstrap/app.php` or `bootstrap/providers.php` change size or modification time. If something still looks stale, run **Laravel: Refresh Index** or click the Laravel version in the [status bar](Status-Bar.md).

### A rule, directive or method is missing, or reported as unknown

Completion and inspections follow the Laravel version in `vendor/laravel/framework`. A rule added in a later Laravel isn't offered, and is reported as unknown, in an older project. Without `vendor/` the plugin uses built-in lists: run `composer install`. Package directives (Livewire, Volt, Inertia, Ziggy, Spatie Permission) are only offered when the package is installed.

### "Allow "process:php" for Laravel OpenIDEA in Settings › Plugins…"

The feature runs PHP and the permission is off. Allow `process:php` (or `process:docker` with a Docker runner) in **Settings › Plugins › Laravel OpenIDEA**. Other messages name `clipboard`, `editor.read` or `editor.write`. See [Permissions](Permissions.md).

### A console window flashes on Windows

Each background PHP call starts `php.exe`, which opens a console. Allow `process:php-win`: the plugin then uses `php-win.exe`, which has none.

### PHP runs in Docker, or in the wrong container

With `artisanRunner` on `auto`, a `compose.yaml`, `compose.yml`, `docker-compose.yaml` or `docker-compose.yml` at the project root makes the plugin use `docker compose run`. If you run PHP on your machine, set `artisanRunner` to `php`. If the wrong service is picked, set `dockerService` (the detection prefers `app`, `laravel.test`, `laravel`, `php`, `api` and `web`). See [Settings](Settings.md#artisanrunner).

### Intelephense says "Undefined method" on `auth()->user()`

Check that `ideHelpers` is on, that a process permission is allowed, and that `storage/openidea/ide-helpers.php` exists. The stub is written when the plugin starts and when you focus an editor. See [IDE helpers](IDE-Helpers.md).

### Tools on save don't run

They need `formatOnSave` or `phpstanOnSave`, the tool in `vendor/bin`, a `phpstan.neon`, `phpstan.neon.dist` or `phpstan.dist.neon` for PHPStan, and a PHP file outside `vendor/`, `storage/`, `bootstrap/cache/` and `node_modules/` (Blade files are skipped). A run that takes longer than 15 seconds is stopped. See [Tools on save](Tools-on-Save.md).

### *Rename route* or *Rename translation* isn't offered

They are offered after you change the name or key, with the cursor in the new value. Before saving, the rest of the file must match the file on disk. See [Rename route](Rename-Route.md) and [Rename translation](Rename-Translation.md).

### The gutter usages icon is missing

It needs OpenIDEA 0.1.40 or newer, and appears once the IDE's background index has read the project. See [Gutter usages](Gutter-Usages.md).

### The status bar shows "Laravel" without a version

`composer.lock` is missing or doesn't list `laravel/framework`. Run `composer install`.

### A changed `locale` has no effect

The locale is read when the plugin starts: reload the plugins or restart the IDE.

### The hub is stuck on *Loading*

Fixed in 0.13.1 (Blade files with multibyte characters crashed the plugin) and 0.13.2 (the IDE's content security policy blocked the hub's scripts on Windows and macOS). Update the plugin and use OpenIDEA 0.1.40 or newer. If it still happens, the plugin probably crashed: report it as below.

### The plugin restarted after a slow command or on a large project

Fixed in 0.15.0: *Artisan optimize:clear* and *Artisan migrate* run in a terminal, and the first indexing runs as a background task. Before, a call longer than OpenIDEA's five-second limit restarted the plugin. See [How it works](How-It-Works.md#time-budgets).

## Report a bug

Open an issue on [GitHub](https://github.com/yzPeedro/laravel-openidea/issues/new/choose). The forms ask for what's needed:

| Form | Use it for |
| --- | --- |
| **Bug report** | Something that doesn't work as described |
| **Crash report** | OpenIDEA showed "Plugin Laravel OpenIDEA crashed", or the hub is stuck on *Loading*. Paste the full text of the notification, including the file and line (such as `panicked at src/blade_syntax.rs:362:16: …`), what you were doing, the smallest file that triggers it, the plugin and OpenIDEA versions, Laravel and PHP versions, the OS, and the IDE log around the crash |
| **Feature request** | Something new |

Questions go to [Discussions](https://github.com/yzPeedro/laravel-openidea/discussions), and vulnerabilities to [private reporting](https://github.com/yzPeedro/laravel-openidea/security/advisories/new).

Never paste secrets, `.env` values or personal data from your project. Sample files should be made up.

## Related

- [How it works](How-It-Works.md)
- [Permissions](Permissions.md)
- [Settings](Settings.md)
- [Status bar](Status-Bar.md)
- [Privacy and security](Privacy-and-Security.md)

---
[← Wiki home](Home.md)
