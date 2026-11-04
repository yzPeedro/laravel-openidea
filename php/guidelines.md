## Laravel OpenIDEA

This project is developed in OpenIDEA with the Laravel OpenIDEA plugin. Its `laravel-openidea` MCP server gives a runtime view of the application. Prefer it over guessing from files:

- `app_overview` first: versions, environment, drivers, packages and counts.
- `list_routes` (filter by URI, name, action or method) to find the controller method behind an endpoint.
- `list_events_and_listeners` for which listeners run for an event (discovered, `Event::listen`, subscribers, observers as `eloquent.*`).
- `list_models` for tables, fillable, casts, relations, local and global scopes and observers of a model.
- `list_container_bindings`, `list_schedule`, `list_queues`, `list_artisan_commands`, `get_config` (secrets masked), `database_schema`, `missing_translations`, `read_logs`.
- `audit_recent` reads the Live Audit recorder: requests with input and timing, SQL with bindings and the calling line, events with payloads and listeners, jobs, cache, mail, notifications, HTTP client calls, gate checks, logs, exceptions and dumps, plus N+1, slow query, exception and failed job insights. When debugging behaviour, ask the user to turn the recorder on in the hub (Live Audit), reproduce, then call it.
- `run_tinker` runs PHP in the booted app for quick checks. Do not write data unless the user asks.

The hub (Laravel OpenIDEA tool window) shows the same information with filters: Artisan and Tinker; routes, controllers, form requests, API resources, middleware; models, scopes, observers, casts, enums, database, factories and seeders; views, components, frontend, translations; all classes, providers, bindings, rules, macros; config, environment, drivers and connections, auth and policies; queues, scheduling, events, listeners, mail, tests; live audit, logs, profiles, debug calls and debug tools; ecosystem packages. Point the user to the right section when it helps.
