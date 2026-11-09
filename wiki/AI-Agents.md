# AI agents and MCP

Laravel OpenIDEA ships a stdio MCP server, `laravel-openidea`, that boots your Laravel application on every call and answers with what it finds at runtime: routes, models, classes, events, bindings, schedule, queues, config with secrets masked, the database schema, translations, tests, logs and the Live Audit recorder, plus Artisan, PHPStan, Pint, tests and Tinker as the access level allows. One click on the hub's [AI Agents](Hub-AI-Agents.md) page registers it with the coding agents that read `.mcp.json` (Codex among them), Cursor, GitHub Copilot, Junie and Gemini, and adds a short guide for them to the project.

<p align="center"><img src="images/hub-ai.webp" alt="The hub's AI Agents page with the MCP server card, the access level and the tool list" width="100%"></p>
<p align="center"><sub>The AI Agents page of the hub</sub></p>

## How to use it

1. Open the hub's **AI Agents** section and click **Connect AI agents**.
2. Restart your agent's session so it loads the new server.
3. Ask the agent to call `app_overview` first. The guide written into the project already tells it to.
4. To allow more, set **MCP access** (`mcpAccess`) in **Settings › Plugins › Laravel OpenIDEA** to `write` or `full`. The plugin passes the new level to the server the next time you focus an editor in the project, and connected agents receive the new tool list.
5. To remove everything, click **Disconnect** on the same page.

You can also start the server by hand, as the agents do:

```
php storage/openidea/mcp.php
```

It speaks JSON-RPC 2.0, one message per line, on stdin and stdout.

## Access levels

The `mcpAccess` setting has three levels; `read` is the default, and any other value counts as `read`.

| Level | Adds |
| --- | --- |
| `read` | The inspection tools, `run_phpstan` and read-only Artisan commands |
| `write` | Generators (`make:*`), `migrate`, `db:seed`, cache commands, `format_code`, `run_tests`, `query_to_sql`, `audit_recorder` |
| `full` | `run_tinker` and every other Artisan command |

The level is stored in `storage/openidea/mcp-access.json` as `{"level":"read"}` and read again on every call, so a change applies to the next call without restarting the agent.

- `tools/list` only returns the tools the level allows.
- Calling a tool above the level fails with: `<tool> needs write access, and the user allowed read for this project. Only the user can change it, in OpenIDEA: Settings › Plugins › Laravel OpenIDEA › MCP access.`
- When the access file or the server file changes between two requests, the server sends `notifications/tools/list_changed` before answering, so clients that support it refresh their list.
- No tool installs, changes or removes the plugin, its files in `storage/openidea` or its settings. `format_code`, `run_phpstan` and `run_tests` refuse paths under `vendor`, `storage`, `bootstrap/cache`, `node_modules` and `.git`, absolute paths and paths with `..`.

Full access runs any PHP inside your app, like Tinker, so it can't be fenced in. And an agent with its own file and shell tools can edit the project regardless of this level: the level decides what this server offers, not what the agent can do by other means.

## Tools

### Read

| Tool | Arguments | Returns |
| --- | --- | --- |
| `app_overview` | none | Laravel and PHP versions, environment, `app.debug`, URL, timezone, locale; the default database, cache, queue, session, mail, broadcasting, filesystem and log drivers; counts of routes, events with listeners, listeners, Artisan commands and scheduled tasks; installed versions of `laravel/*`, `livewire/*`, `inertiajs/*`, `spatie/*`, `filament/*`, `barryvdh/*`, `itsgoingd/*`, `laradumps/*`, `pestphp/*`, `larastan/*`, `nunomaduro/*` and `tightenco/*` packages from `composer.lock`; and a pointer to the hub |
| `access_level` | none | The current level, the tools of each level, the Artisan commands each level unlocks and where the user changes it |
| `list_routes` | `filter` (URI, name or action), `method` | Every route of the router with its methods (without HEAD), URI, name, action (`Controller@method`), gathered middleware and the controller method's `file:line` |
| `list_classes` | `kind`, `filter` (class name) | Without a kind, the number of classes of each kind. With one: each class with `file:line`, parent, own interfaces, public methods; enum cases (with values for backed enums), a command's signature, the type a listener's or observer's `handle()` takes. Kinds: `controller`, `request`, `resource`, `middleware`, `model`, `enum`, `cast`, `rule`, `scope`, `observer`, `event`, `listener`, `job`, `mail`, `notification`, `policy`, `provider`, `command`, `component`, `livewire`, `factory`, `seeder`, `exception`, `interface`, `trait` |
| `list_events_and_listeners` | `filter` | Every event with listeners as registered at runtime (discovery, `Event::listen`, subscribers, observers as `eloquent.*` events), sorted by event name. Closures show as `Closure file:line` |
| `list_models` | `filter` (model name) | Every non-abstract Eloquent model in `app/`: file, table, connection, primary key, fillable, guarded, hidden, casts, relations (method → type and related model, from public methods with a relation return type), local scopes (`scope*` and `#[Scope]`), global scopes and observers (`event → listener`) |
| `list_container_bindings` | `filter` (abstract) | Each binding with its abstract, whether it is shared, and the concrete class or the closure's location |
| `list_schedule` | none | Each scheduled task: cron expression, command (shortened to `artisan …`), description, next run, timezone, `withoutOverlapping` and `onOneServer` |
| `list_queues` | none | The default connection, the connection names, every non-abstract `ShouldQueue` class in `app/` with its file, queue, tries and timeout, and the failed jobs count when the failer is configured |
| `list_artisan_commands` | `filter` (name or description) | Every command with its description, synopsis and class |
| `run_artisan` | `command`, `arguments` | Runs `php artisan <command> … --no-interaction --no-ansi` for up to 300 seconds; returns exit code, timeout flag, output and errors. The command's level decides who may run it (see below) |
| `get_config` | `key` (default `app`) | The value of a config key or a whole file, with secrets masked |
| `database_schema` | `table`, `connection` | Without a table, every table with its rows and size when the driver reports them; with one, its columns, indexes and foreign keys from the live connection |
| `missing_translations` | `locale` | For each locale, how many keys other locales have and it lacks, and up to 300 of them. Reads JSON files and PHP files in `lang/` (or `resources/lang/`), skipping `vendor` |
| `check_env_keys` | none | Whether `.env` and `.env.example` exist, and the key names missing from each. Values are never read out |
| `list_tests` | `filter` (name or file) | Pest `it()`/`test()` calls and PHPUnit `test*` methods or `#[Test]` methods under `tests/`, with file, line, name and kind; stops after about 3,000 |
| `run_phpstan` | `paths`, `level` (0 to 10) | Runs `vendor/bin/phpstan analyse --error-format=json` (Larastan when `phpstan.neon` includes it) for up to 300 seconds; returns the totals, the errors per file with line, message, identifier and tip, and the general errors. Without paths it uses the config's |
| `read_logs` | `lines` (default 30, at most 500), `level` | The last entries of the most recently modified `storage/logs/*.log` (reading its last 2 MB), each with time, level and message (cut at 3,000 characters) |
| `audit_recent` | `types`, `limit` (default 200, at most 2,000), `filter` | The newest Live Audit records from `storage/logs/openidea-audit.jsonl` (its last 4 MB) and up to 50 insights: slow queries (100 ms or more), possible N+1 (the same SQL from the same line 5 times or more in one scope), exceptions and failed jobs. When the recorder never ran, it answers that it is off and how to turn it on |
| `hub_sections` | none | What each group of the hub offers, so the agent can point you to the right section |

### Write

| Tool | Arguments | Returns |
| --- | --- | --- |
| `format_code` | `paths` (required), `dry_run`, `tool` (`pint` or `php-cs-fixer`) | Runs `vendor/bin/pint` (with `--test` for a dry run) or `vendor/bin/php-cs-fixer fix` (with `--dry-run --diff`), whichever is installed, Pint first; returns its output |
| `run_tests` | `filter`, `path` | Runs `vendor/bin/pest` or else `vendor/bin/phpunit` with `--colors=never` for up to 600 seconds. Tests can write to the testing database |
| `query_to_sql` | `code` | The SQL an Eloquent or query builder expression runs, with bindings filled in, captured with `DB::pretend()` and model events off, so nothing runs |
| `audit_recorder` | `state`: `on`, `off` or `status` | Turns the Live Audit recorder on or off, or reports it, through the plugin's own installer in `storage/openidea` |

`query_to_sql` only accepts one expression on models or `DB` with literal values: variables (other than an arrow function's parameters), function calls (other than `now()` and `today()`), strings that name something callable and methods such as `getPdo`, `macro`, `extend`, `make`, `app`, `call` or `dispatch` are refused. Class names resolve as given, then under `App\Models\`, then under `App\`, and must be Eloquent models.

### Full

| Tool | Arguments | Returns |
| --- | --- | --- |
| `run_tinker` | `code` | Runs PHP in the booted app through PsySH (or `eval` without it): the result (as an array when it has `toArray()`), its type, echo output, the error if any and the SQL queries it ran |

### Artisan commands per level

`run_artisan` is a read tool, but each command has its own level:

| Level | Commands |
| --- | --- |
| read | `about`, `list`, `help`, `env`, `inspire`, `route:list`, `migrate:status`, `schedule:list`, `event:list`, `model:show`, `db:show`, `db:table`, `db:monitor`, `channel:list`, `queue:failed`, `queue:monitor` |
| write | `make:*`, `migrate`, `db:seed`, `cache:clear`, `config:cache`, `config:clear`, `route:cache`, `route:clear`, `view:cache`, `view:clear`, `event:cache`, `event:clear`, `optimize`, `optimize:clear`, `storage:link`, `queue:retry`, `queue:restart`, `vendor:publish`, `lang:publish`, `stub:publish`, `package:discover`, `schedule:clear-cache`, `test` |
| full | Every other command, such as `migrate:fresh`, `migrate:rollback`, `db:wipe`, `key:generate`, `config:show` or `schedule:run` |

Commands that keep running or need a terminal are refused at every level with a hint: `tinker` (use `run_tinker`), `serve`, `queue:work`, `queue:listen`, `schedule:work`, `pail` (use `read_logs`), `horizon`, `reverb:start`, `octane:start` and `dev`.

## Masking

- `get_config` replaces the value of any key whose name matches `pass`, `password`, `secret`, `token`, `api_key` / `api-key` / `apikey`, `authorization`, `cookie`, `credential`, `private`, `signature`, `dsn` or ends in `key` (`app.key`, `stripe_key`) with `•••`, at any depth, unless the value is empty. Asking for such a key directly (`app.key`) is masked too. A URL with credentials keeps everything but the password: `mysql://user:•••@host/db`. Objects show as `{ClassName}`, closures as `{Closure}`, and nesting deeper than 6 levels as `[…]`.
- `check_env_keys` reads only the names of the keys.
- `audit_recent` returns what the Live Audit recorder wrote, which is masked when recorded: secret-like keys, personal data keys (email, phone, document, address, birth date, CPF, passport and similar) and a model's hidden attributes become `•••`, and tokens in HTTP client URLs are hidden. Setting `OPENIDEA_AUDIT_SHOW_PII=true` in the app's environment shows the personal data keys. See [Live Audit](Hub-Live-Audit.md).
- `read_logs`, `run_artisan`, `run_tinker`, `run_tests` and `database_schema` return what Laravel returns; they are not masked.

## What connecting writes

**Connect AI agents** runs the plugin's setup script with the project's runner and:

1. Creates `storage/openidea/` with a `.gitignore` containing `*`, so nothing in it is committed.
2. Writes `storage/openidea/mcp.php` (the server), `install.php` (the recorder installer used by `audit_recorder`), `recorder.source` (the Live Audit recorder) and `mcp-access.json` (the level).
3. Adds a `laravel-openidea` server to the agent config files, keeping every other entry:

| File | Client | Key | Written |
| --- | --- | --- | --- |
| `.mcp.json` | Agents that read the project's `.mcp.json`, and Codex through ACP | `mcpServers` | Always (created when missing) |
| `.cursor/mcp.json` | Cursor | `mcpServers` | When `.cursor/` or the file exists |
| `.vscode/mcp.json` | GitHub Copilot in VS Code | `servers`, with `"type": "stdio"` | When `.vscode/` or the file exists |
| `.junie/mcp/mcp.json` | Junie | `mcpServers` | When `.junie/mcp/` or the file exists |
| `.gemini/settings.json` | Gemini | `mcpServers` | When `.gemini/` or the file exists |

   A file that isn't valid JSON is left alone. The entry is `{"command": …, "args": […]}`:

| Runner | Command |
| --- | --- |
| php | `php <project>/storage/openidea/mcp.php` |
| `docker-run` (or `auto` with a compose file) | `docker compose -f <project>/compose.yaml run --rm --no-deps -T --entrypoint php <service> storage/openidea/mcp.php` |
| `docker` | `docker compose -f <project>/compose.yaml exec -T <service> php storage/openidea/mcp.php` |

   The `-f` part names whichever compose file the project has, and is left out when there is none.

4. Writes the guide for agents:
   - Without Laravel Boost: a block between `<!-- laravel-openidea:start -->` and `<!-- laravel-openidea:end -->` in each agent guideline file that already exists (`AGENTS.md`, `GEMINI.md`, `.github/copilot-instructions.md`, `.junie/guidelines.md` and the other agents' root guideline file), replacing the previous block. When none of them exists, it creates `AGENTS.md`.
   - With Laravel Boost (`vendor/laravel/boost` exists): `.ai/guidelines/laravel-openidea.md` instead, so `php artisan boost:update` merges it into the guidelines Boost maintains.

The guide tells agents to start with `app_overview` and `access_level`, which tool answers which question, how to debug with `audit_recorder` and `audit_recent`, what write and full access add, to tell the user when a tool needs a higher level instead of trying to change it, never to touch `storage/openidea`, and where things are in the hub.

**Update connection** does the same again. **Disconnect** removes the `laravel-openidea` entry from every config file (deleting a file that ends up empty), removes the marked block from the guideline files (deleting a file that ends up empty), deletes `.ai/guidelines/laravel-openidea.md` and the four server files, and removes `storage/openidea/` when nothing else is left in it.

## Staying in step

Whenever you focus an editor in a Laravel project, and when the AI Agents page loads, the plugin compares the four files in `storage/openidea` with its own version and the current `mcpAccess`. If `mcp.php` exists and anything differs, it rewrites them; it never creates them in a project you haven't connected. A plugin update or a new access level therefore reaches the running server without reconnecting, and the server announces the new tool list. The plugin tries once per level and version per session; a failure is written to the IDE log.

## How the server works

- The process the agent starts reads requests line by line. It answers `initialize` (server `laravel-openidea`, with `tools.listChanged`) and `ping` itself, and ignores notifications.
- Every other request (`tools/list`, `tools/call`) runs in a fresh PHP worker (`mcp.php --worker`), and the tools boot the app there (`vendor/autoload.php`, `bootstrap/app.php`, the console kernel). Answers follow the code as it changes, and a crash can't take the server down.
- When a worker dies on a PHP fatal or parse error in a project file (not in `vendor`), the server retries up to 10 times, skipping each file that fails in the class scans, and appends `Left out because PHP cannot load them: …` to the answer.
- Results are JSON text; one longer than 400,000 characters is cut with `… truncated; use a filter`. A tool error comes back as a result with `isError: true` and `ExceptionClass: message`; an unknown tool is a JSON-RPC error.
- The server finds the project from the current directory when it has an `artisan` file, otherwise from its own location (two levels above `storage/openidea`).
- Class scans (`list_models`, `list_classes`, `list_queues`) read Composer's classmap and every PHP file under `app/` (and `database/` for `list_classes`), so they find classes before `composer dump-autoload`.
- When the server runs as root (a Docker container), after write and full tools and `run_artisan` it gives files created by the call back to the project folder's owner, so generated files aren't root-owned on the host.

## Laravel Boost

[Laravel Boost](https://github.com/laravel/boost) is Laravel's own MCP server and guideline kit. The two servers run side by side and agents get both: Boost brings documentation search for your package versions, database queries, browser logs, Tinker and versioned guidelines; this server brings the hub's view of the app, Live Audit and the access levels. The AI Agents page installs Boost (`composer require laravel/boost --dev`), runs `boost:install` and `boost:update`, and when Boost is installed the guide goes to `.ai/guidelines/` so Boost's update keeps it.

## Settings and permissions

| Setting | Effect |
| --- | --- |
| `mcpAccess` | `read` (default), `write` or `full` |
| `artisanRunner`, `dockerService` | Which command the agents run (php or Docker Compose, and the service) |

Connecting and the automatic sync run PHP, so they need `process:php` (or `process:docker`); on Windows `process:php-win` avoids a console window. The server itself runs in the agent's process, not in the IDE. See [Settings](Settings.md) and [Permissions](Permissions.md).

## Related

- [AI Agents in the hub](Hub-AI-Agents.md)
- [Live Audit](Hub-Live-Audit.md)
- [Tinker](Hub-Tinker.md)
- [Copy Query as MySQL](Copy-Query-as-MySQL.md)
- [Privacy and security](Privacy-and-Security.md)
- [How it works](How-It-Works.md)

---
[← Wiki home](Home.md)
