## Laravel OpenIDEA

This project is developed in OpenIDEA with the Laravel OpenIDEA plugin. Its `laravel-openidea` MCP server gives a live view of the application: every call boots the app again, so answers follow the code as it changes. Prefer it over guessing from files:

- `app_overview` first: versions, environment, drivers, packages and counts. `access_level` tells what the user allowed (read, write or full) and what each level unlocks.
- `list_routes` (filter by URI, name, action or method) to find the controller method behind an endpoint.
- `list_classes` lists controllers, form requests, resources, middleware, enums, casts, rules, observers, events, listeners, jobs, mail, notifications, policies, providers, commands, components, factories and seeders.
- `list_events_and_listeners` for which listeners run for an event (discovered, `Event::listen`, subscribers, observers as `eloquent.*`).
- `list_models` for tables, fillable, casts, relations, local and global scopes and observers of a model.
- `list_container_bindings`, `list_schedule`, `list_queues`, `list_artisan_commands`, `get_config` (secrets masked), `database_schema`, `missing_translations`, `check_env_keys`, `list_tests`, `read_logs`.
- `run_phpstan` returns PHPStan (and Larastan) errors; `run_artisan` runs read-only commands such as `route:list` or `model:show`.
- `audit_recent` reads the Live Audit recorder: requests with input and timing, SQL with bindings and the calling line, events with payloads and listeners, jobs, cache, mail, notifications, HTTP client calls, gate checks, logs, exceptions and dumps, plus N+1, slow query, exception and failed job insights. When debugging behaviour, turn the recorder on with `audit_recorder` (write access) or ask the user to do it in the hub, reproduce, then call it.
- With write access: `run_artisan` also runs `make:*`, `migrate`, `db:seed` and the cache commands; `format_code` runs Pint or PHP CS Fixer; `run_tests` runs Pest or PHPUnit; `query_to_sql` shows the SQL of an Eloquent expression without running it.
- With full access: `run_tinker` runs any PHP in the booted app and `run_artisan` runs any command. Do not write or delete data unless the user asks.

When a tool needs a higher level, tell the user; only they can change it, in Settings › Plugins › Laravel OpenIDEA › MCP access. Never try to change the level, the plugin or its files in `storage/openidea`.

The hub (Laravel OpenIDEA tool window) shows the same information with filters: Artisan and Tinker; routes, controllers, form requests, API resources, middleware; models, scopes, observers, casts, enums, database, factories and seeders; views, components, frontend, translations; all classes, providers, bindings, rules, macros; config, environment, drivers and connections, auth and policies; queues, scheduling, events, listeners, mail, tests; live audit, logs, profiles, debug calls and debug tools; ecosystem packages. Point the user to the right section when it helps.
