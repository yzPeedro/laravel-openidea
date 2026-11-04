# Roadmap

Laravel OpenIDEA is an independent project. It reuses no code, branding or text from other IDE plugins.

## Done

- Completion and navigation for routes, views, config, env, translations, validation rules, middleware, gates and policies, rate limiters, disks, caches, queues, log channels, connections, mailers, guards, assets, Blade components, Livewire, Inertia, OpenAPI, Sanctum, Spatie Permission, Socialite and broadcast channels.
- Eloquent columns, relations, scopes, casts and model attributes from migrations and models.
- Blade language: highlighting, structure, folding and directive completion.
- Traits, interfaces and base classes completion.
- Inspections and quick fixes, *Generate @property PHPDoc*, related class or view.
- The hub: 40+ sections with filters and sorting, Artisan, Tinker, logs, Live Audit, profiles, debug calls, ecosystem and AI agents.
- MCP server for AI agents and Laravel Boost integration.

## Next

| Item | Notes |
| --- | --- |
| Storage path completion (`Storage::get('…')`) | From `storage/app` and the configured disks |
| Pivot attributes on `->pivot` | From `withPivot()` and pivot models |
| Hub: create missing classes | Quick fix to create a Livewire or Blade component class |
| Route actions from the hub | "Add route" when a name is used but not defined |
| Cached routes | Read `bootstrap/cache/routes-v7.php` when it exists |
| More packages | laravel-data, Filament resources, Nova, Folio pages |

## Needs new OpenIDEA plugin APIs

These need host APIs that OpenIDEA doesn't offer yet. They would be proposed upstream first.

| Item | API |
| --- | --- |
| Hover with config values, route URIs and translations | hover provider |
| Find usages of a route, view, config key or translation | references provider |
| Rename a route, view or config key | rename provider |
| Inline config and translation values | inlay hints provider |
| Typed model members everywhere | access to the IDE's PHP symbols |
