# Privacy and security

Laravel OpenIDEA reads your project to understand it, runs PHP only when you allow it, and writes only to a git-ignored folder unless you ask for more. Nothing it reads or records is sent anywhere. This page lists exactly what is read, what runs, what is written and what is masked.

## The sandbox

The plugin is a WebAssembly component (`dist/plugin.wasm`) that OpenIDEA runs in a sandbox: the Settings › Plugins page lists its code as "WebAssembly component (sandboxed)". It has no direct access to your disk, network or processes. Everything goes through the capabilities in `plugin.toml`:

- required: `project.read` (read files of the open project), `events` (file and editor events), `storage` (the plugin's own key-value store in the IDE);
- optional: `process:php`, `process:php-win`, `process:docker`, `editor.read`, `editor.write`, `clipboard`.

There is no network capability: the plugin can't send data anywhere. See [Permissions](Permissions.md) for what each capability unlocks.

## What is read

With `project.read` the plugin reads the files it indexes:

| Files | For |
| --- | --- |
| `app/`, `routes/`, `config/`, `database/`, `resources/`, `lang/`, `tests/`, `bootstrap/app.php`, `bootstrap/providers.php`, `bootstrap/cache/routes-v7.php` | Routes, views, config keys, translations, models, migrations, classes and the rest of the indexes |
| `composer.json`, `composer.lock`, `vendor/composer/installed.json` | The Laravel version, installed packages, PSR-4 namespaces |
| `vendor/laravel/framework` and a few other vendor files (`vendor/phpunit/phpunit/phpunit.xsd`…) | Rules, directives, casts and methods of the installed version |
| `.env`, `.env.example` | Environment keys |
| `pint.json`, `.php-cs-fixer*.php`, `phpstan.neon`, `vite.config.*`, `phpunit.xml`, front-end files | Completion in those files |

**`.env` values are never indexed.** The environment index keeps the *keys* of `.env` and the keys and values of `.env.example` only, so completion details and the hub's Environment section ("Example value") show example values, never your real secrets. The Config section shows each value as written in the config file (for example `env('DB_PASSWORD')`), not its runtime value.

Indexes live in the plugin's memory. The only things the plugin stores in the IDE (`storage`) are your Tinker tabs, your last 20 Artisan runs and the SQL to Laravel state.

## What runs

Nothing runs unless you allow `process:php`, `process:php-win` or `process:docker`. With them, the plugin runs `php` (or `php-win` on Windows, or `docker compose`) in the project root, through the [runner](Settings.md#artisanrunner) you chose:

| Run | When |
| --- | --- |
| `artisan <command>` | You run a command from the hub, the [commands](Commands.md) or a quick action |
| Tinker scripts (`php/tinker.php`) | You run code in the hub's Tinker, *Run Selection in Tinker*, or SQL to Laravel's run button |
| `php/cache.php` | You open the hub's Cache section, *Forget* a key or *Flush store* |
| `php/schema.php` | You click **Compare with database** or **Rows** in the Database section, or **EXPLAIN** in Live Audit. It only reads: rows are fetched with `select` and `limit`, secret and personal data columns come back as `•••`, and `EXPLAIN` takes a single `SELECT` inside a transaction that is rolled back |
| `php/install.php` | You turn Live Audit on or off, connect or disconnect AI agents, and when the plugin syncs `storage/openidea` |
| `php/create_file.php` | You extract a Form Request, or click **Create migration** on a missing index in Live Audit |
| `php/tools.php` running `vendor/bin/pint`, `vendor/bin/php-cs-fixer`, `vendor/bin/phpstan` | You turned on [tools on save](Tools-on-Save.md) (off by default) |
| `storage/openidea/mcp.php` | An AI agent you connected starts the MCP server |
| `composer require …` (in a terminal) | You install a package from the Ecosystem or AI Agents section |

The scripts are embedded in the plugin and passed to `php -r` (long ones travel on stdin, because OpenIDEA 0.1.39 limits a process argument to 4096 bytes). They are the plugin's own code; nothing is downloaded.

## What is written

### Without you asking: `storage/openidea`

The plugin keeps a few helper files in `storage/openidea`. Each script that creates the folder also writes `storage/openidea/.gitignore` containing `*`, so the whole folder stays out of git.

| File | Written when | Removed when |
| --- | --- | --- |
| `ide-helpers.php` | `ideHelpers` is on (default) and the stub is missing or out of date, on activation and editor focus | You turn `ideHelpers` off |
| `hub/*.laravel-hub`, `hub/sections` | On editor focus when they are missing or out of date, and when you open a hub section in an editor tab | Files of sections that no longer exist are deleted on the next sync |
| `mcp.php`, `install.php`, `recorder.source`, `mcp-access.json` | Kept in step with the plugin version and `mcpAccess` on editor focus, **only after you connected AI agents** | *Disconnect* |
| `selection.php` | *Run Selection in Tinker* | Overwritten by the next run |

When the folder ends up with only its `.gitignore`, it is removed.

### When you ask

| Action | Writes |
| --- | --- |
| A code action or quick fix you pick | The edits it names (rename, extract, add to `$fillable`, PHPDoc…), applied by the IDE so you can undo them |
| [Extract Form Request](Extract-Form-Request.md) | `app/Http/Requests/<Name>.php`. The script refuses paths with `..` or starting with `/`, and never overwrites an existing file |
| [Tools on save](Tools-on-Save.md) with `formatOnSave` | The saved file, reformatted by Pint or PHP CS Fixer. In `docker-run` mode the tools' temporary files go to `storage/framework/cache/openidea` |
| Live Audit *Turn on recorder* | `storage/openidea/Recorder.php` and a hook in `vendor/autoload.php` between two `/* openidea-recorder */` markers. Both are checked with `php -l` first, and *Turn off* removes them. Records go to `storage/logs/openidea-audit.jsonl` |
| AI Agents *Connect* | A `laravel-openidea` entry in `.mcp.json` and, when their folders exist, `.cursor/mcp.json`, `.vscode/mcp.json`, `.junie/mcp/mcp.json` and `.gemini/settings.json`; a block between `<!-- laravel-openidea:start -->` and `<!-- laravel-openidea:end -->` markers in each agent guideline file that exists (`AGENTS.md`, `GEMINI.md`, `.github/copilot-instructions.md`, `.junie/guidelines.md` and the other agents' root guideline file) (a new `AGENTS.md` when none does); with Laravel Boost, `.ai/guidelines/laravel-openidea.md` instead. *Disconnect* removes exactly these entries and blocks |
| SQL to Laravel *copy* / *insert* | The clipboard / the editor at the cursor |
| *Copy Query as MySQL* | The clipboard |

The installer, the recorder and the MCP files never touch tracked files, apart from the agent configs and guideline files you asked for with *Connect*. When a script runs as root (typical in Docker), it hands the files it creates back to the owner of the project folder.

## Live Audit masking

The recorder runs only when the app's environment is `local`, `development` or `dev`, unless `.env` sets `OPENIDEA_AUDIT=true` (which turns it on in any environment). `OPENIDEA_AUDIT=false` (or `0`) turns it off everywhere.

Before a value is written, its key is split into words (at `_`, `-`, `.` and other separators, at camelCase humps and before digits) and each word, and each pair of neighbouring words written together, is checked. Only whole words count, so `footprint`, `compass` and `accepted_terms` are kept:

| Category | Words matched (case-insensitive) | Shown as |
| --- | --- | --- |
| Secrets, always masked | `pass`, `passwd`, `password`, `passphrase`, `passcode`, `pwd`, `secret`, `token`, `apikey` (`api_key`, `apiKey`, `X-Api-Key`), `authorization`, `cookie`, `credential`, `private`, `signature`, `otp`, `cvv`, `cvc`, `card`, `cardholder`, `creditcard` (and their plurals), and any key whose last word is `key` (`app_key`, `stripe.key`) | `•••` |
| Personal data, masked by default | `email` (`e-mail` too), `cpf`, `cnpj`, `rg`, `document`, `phone`, `telefone`, `celular`, `mobile`, `birth`, `birthday`, `birthdate`, `nascimento`, `address`, `endereco`, `cep`, `zip`, `zipcode`, `ssn`, `passport` | `•••`, or the value when `OPENIDEA_AUDIT_SHOW_PII=true` (or `1`) |

HTTP client URLs have `token`, `key`, `secret`, `signature` and `password` query parameters masked. Strings are cut at 400 characters, arrays at 25 items and nesting at 3 levels. The log rotates to `openidea-audit.jsonl.1` above 8 MB. See [Live Audit](Hub-Live-Audit.md).

## MCP server

The MCP server runs locally over stdio, started by the agent you connected. Config values whose key looks like a secret (`password`, `secret`, `token`, `api_key`, `authorization`, `cookie`, `credential`, `private`, `signature`, `dsn` and keys ending in `key`) are masked, and passwords in URLs such as `mysql://user:secret@host` become `•••`. What agents may do depends on `mcpAccess`: `read` (the default) only inspects, `write` adds generators, migrations, seeding, cache commands, formatting and tests, and `full` adds Tinker and every Artisan command, which can run any PHP. The level decides what the server offers, not what an agent with its own file or shell tools could do anyway. No tool installs, changes or removes the plugin, its settings, its grants or its files in `storage/openidea`. See [AI agents and MCP](AI-Agents.md).

## The IDE log

The plugin writes failures to the IDE log as warnings and errors, each distinct message once, cut at 4000 characters. A failed PHP run includes its exit code and the last eight lines of its error and output, which can contain what your app printed. Nothing is logged on a healthy run. See [Troubleshooting and logs](Troubleshooting.md).

## Reporting a vulnerability

Don't open a public issue: use [GitHub's private vulnerability reporting](https://github.com/yzPeedro/laravel-openidea/security/advisories/new) and say what is affected (plugin, recorder, installer or MCP server), the version, how to reproduce it and the impact. You get an answer within a week. Only the latest release receives fixes.

## Related

- [Permissions](Permissions.md)
- [Settings](Settings.md)
- [How it works](How-It-Works.md)
- [Live Audit](Hub-Live-Audit.md)
- [AI agents and MCP](AI-Agents.md)
- [Troubleshooting and logs](Troubleshooting.md)

---
[← Wiki home](Home.md)
