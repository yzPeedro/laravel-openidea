# Unknown route names

Reports a route name that no route defines, in `route()`, `to_route()`, `redirect()->route()`, `Route::has()`, `routeIs()` and the other calls that take a route name. A misspelt or renamed route turns into a warning instead of a `Route [x] not defined` exception. The check is **off by default**.

## How to use it

Turn on **Report unknown route names** in **Settings › Plugins › Laravel OpenIdea** (setting `unknownRouteInspection`). The name is then checked in PHP and Blade files in these places:

| Where | Argument checked |
| --- | --- |
| `route('…')`, `to_route('…')` | first |
| `Route::has('…')` | first, also each name of an array |
| `URL::route('…')`, `URL::signedRoute('…')`, `URL::temporarySignedRoute('…')` | first |
| `Redirect::route('…')`, `Redirect::signedRoute('…')`, `Redirect::temporarySignedRoute('…')` | first |
| `->route('…')`, `->signedRoute('…')`, `->temporarySignedRoute('…')` on any object (`redirect()->route()`, `url()->route()`…) | first |
| `->routeIs('…')`, `->named('…')` | first, also each name of an array |

```php
return redirect()->route('orders.shwo', $order);   // Route 'orders.shwo' not found

if (request()->routeIs('admin.dashbord')) { … }     // Route 'admin.dashbord' not found
```

```blade
<a href="{{ route('profile.edti') }}">Profile</a>
```

`->route('…')` on `$request`, `request()` or `$this` is not a route name (it reads a route parameter) and is never checked.

## What you see

- **Severity:** Warning.
- **Code:** `laravel.unknown-route`.
- **Message:** `Route '<name>' not found`.
- **Range:** the route name inside the quotes.

There is no quick fix. To rename a route and every usage at once, change its `->name()` and use [Rename route](Rename-Route.md).

## Details

- **Where the route names come from:**
  - every file under `routes/` (up to 1,000 files): `->name('…')`, `'as' => '…'`, the name prefixes of `Route::name('admin.')->group(…)` and `Route::group(['as' => …])`, and the seven names of `Route::resource()` (five for `Route::apiResource()`), narrowed by `->only()` and `->except()`;
  - named [Folio](Packages.md) pages;
  - the route cache, `bootstrap/cache/routes-v7.php`, when it exists: every named route in it that the route files don't already define, which brings in the routes packages register (Sanctum, Horizon, Fortify…). Names that start with `generated::` are ignored.
- **Why it is off by default.** Routes registered by packages, or by service providers outside `routes/`, are only known through the route cache. Without `php artisan route:cache` their names would be reported as missing.
- **Refresh.** The route index is rebuilt when a file under `routes/`, `bootstrap/app.php`, `bootstrap/cache/`, `app/Providers/` or `resources/views/` is saved or changes on disk. The route cache is watched even though it is usually git-ignored.
- **Skipped strings.** Patterns such as `routeIs('admin.*')` (anything with `*`), names with `$` or `{`, concatenated names (`'admin.' . $section`), empty strings and strings you are still typing.
- **Comments are skipped**, and in Blade only the PHP parts are read.

## Settings and permissions

| Setting | Label | Default |
| --- | --- | --- |
| `unknownRouteInspection` | Report unknown route names | off |

See [Settings](Settings.md). No optional permission is needed.

## Related

- [Inspections overview](Inspections.md)
- [Route names](Route-Names.md): completion and Ctrl+click on route names
- [Rename route](Rename-Route.md)
- [Routes in the hub](Hub-Routes.md)

---
[← Wiki home](Home.md)
