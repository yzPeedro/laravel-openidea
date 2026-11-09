# Tools on save

Turn on `formatOnSave` and `phpstanOnSave` and every PHP file you save is formatted with Pint or PHP CS Fixer and analysed with PHPStan (Larastan included, through your `phpstan.neon`), using the project's own tools from `vendor/bin` and its own config. PHPStan's errors show as warnings on their lines. Both are off by default.

## How to use it

1. Install the tools in the project, as usual: `composer require --dev laravel/pint` (or `friendsofphp/php-cs-fixer`), and `larastan/larastan` or `phpstan/phpstan` with a `phpstan.neon`.
2. Open **Settings › Plugins › Laravel OpenIdea** and set:
   - **Format PHP files on save with** (`formatOnSave`) to `pint` or `php-cs-fixer`;
   - **Run PHPStan (and Larastan) on the saved file and show its errors** (`phpstanOnSave`) on.
3. Allow `process:php` (or `process:docker`).
4. Save a PHP file. A task such as **Laravel: pint and phpstan** shows in the status bar while they run.

## What you see

- **Formatting:** the file is rewritten on disk by the formatter, with your `pint.json` or `.php-cs-fixer.php` rules.
- **PHPStan errors:** one warning per error, spanning the line from its first non-blank character to its end. The message is PHPStan's, with its tip in parentheses when it gives one, the source is **PHPStan**, and the code is the error's identifier (such as `missingType.return`). They appear next to the plugin's own [inspections](Inspections.md) and stay until the file is saved again.

## Details

### Which files

Files that end in `.php` and are not Blade views, in a Laravel project, outside `vendor/`, `storage/`, `bootstrap/cache/` and `node_modules/`. The tools run on the saved file only, never on the whole project.

### Which tools run

| Setting | Runs when | Command |
| --- | --- | --- |
| `formatOnSave` = `pint` | `vendor/bin/pint` exists | `php vendor/bin/pint <file> --no-interaction` |
| `formatOnSave` = `php-cs-fixer` | `vendor/bin/php-cs-fixer` exists | `php vendor/bin/php-cs-fixer fix <file> --quiet --no-interaction` |
| `phpstanOnSave` on | `vendor/bin/phpstan` exists **and** the project has `phpstan.neon`, `phpstan.neon.dist` or `phpstan.dist.neon` | `php vendor/bin/phpstan analyse --error-format=json --no-progress --no-interaction --memory-limit=1G <file>` |

A tool that isn't installed, or PHPStan without a config, is skipped silently. The plugin passes no config of its own, so Larastan, the level and any baseline come from your PHPStan config.

The formatter runs first, so PHPStan analyses the formatted file.

### How they run

The plugin starts `php/tools.php` with your [runner](Settings.md#artisanrunner) (`php`, or a Docker Compose container), which runs each tool with the same PHP binary, in the project root, one after the other:

- **15 seconds in total.** The tools share one deadline. A tool still running at the deadline is stopped, and a tool that would start with less than 0.2 seconds left is skipped as timed out.
- **Docker `run` mode.** With the throwaway container (`docker-run`), the tools' `TMPDIR` points at `storage/framework/cache/openidea` inside the project, so their temporary files outlive the container; files created there as root are given back to the project's owner.
- The run is an IDE task, so it may take up to 60 seconds of the plugin's time without hitting the five-second limit of a plugin call.

### When something goes wrong

Each problem is shown once per session as a notification, and written to the IDE log:

| Problem | Notification | Log |
| --- | --- | --- |
| The plugin couldn't start PHP (permission denied, no PHP…) | Laravel OpenIDEA couldn't run the tools on save | `<tools> on save: …` |
| A tool ran out of time | &lt;tool&gt; took longer than 15 seconds: "It was stopped after 15 seconds. Run it from the terminal for the full result; later saves are usually faster thanks to its cache." | `<tool> on save: timed out after 15 seconds on <file>` |
| A tool failed | &lt;tool&gt; failed on save, with the last six lines of its output, "It exited with code N and printed nothing." or "It didn't start." | `<tool> on save: failed on <file>: …` |
| The script returned nothing | | `<tools> on save: no results for <file>: exit code …` (an error) |

**Failed or found problems?** Pint and PHP CS Fixer fail when they exit with a code other than 0. PHPStan exits with 1 when it finds errors, so it counts as failed only when its output has no JSON report (`"totals"`), as with a fatal error or a memory limit. When PHPStan fails, the file's previous PHPStan warnings are cleared.

### Turning it off

Set `formatOnSave` to `off` and `phpstanOnSave` off. The next save of a file also clears its PHPStan warnings.

## Settings and permissions

| Setting | Default | Effect |
| --- | --- | --- |
| `formatOnSave` | `off` | `pint`, `php-cs-fixer` or `off` |
| `phpstanOnSave` | off | Run PHPStan on the saved file |
| `artisanRunner`, `dockerService` | `auto`, empty | Where the tools run |

Needs `process:php`, or `process:docker` with a Docker runner. See [Settings](Settings.md) and [Permissions](Permissions.md).

## Related

- [Pint, PHP CS Fixer and PHPStan configs](Tool-Configs.md) (completion in `pint.json`, `.php-cs-fixer.php` and `phpstan.neon`)
- [Inspections overview](Inspections.md)
- [Settings](Settings.md)
- [Troubleshooting and logs](Troubleshooting.md)
- [Status bar](Status-Bar.md)

---
[← Wiki home](Home.md)
