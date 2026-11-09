# Unknown middleware and abilities

Reports middleware aliases that no one defines (`->middleware('admn')`) and authorization abilities that no gate or policy defines (`Gate::allows('edit-post')`, `@can('publsh')`). Both fail only at runtime, with `Target class [admn] does not exist` or a silent "forbidden". The check is **off by default**, because abilities and aliases can come from places the plugin doesn't read.

## How to use it

Turn on **Report unknown middleware and abilities** in **Settings › Plugins › Laravel OpenIdea** (setting `unknownAuthInspection`). One setting covers both checks, in PHP and Blade files.

**Middleware** names are checked in:

| Where | Argument |
| --- | --- |
| `Route::middleware('…')`, `Route::withoutMiddleware('…')` | first, also each string of an array |
| `->middleware('…')`, `->withoutMiddleware('…')` on any object (routes, groups, controllers) | first, also each string of an array |
| `$middleware = [...]` (a property or variable named `$middleware`) | each string |
| `return [...]` inside a method named `middleware()` | each string |

**Abilities** are checked in:

| Where | Argument |
| --- | --- |
| `Gate::allows`, `denies`, `check`, `any`, `none`, `authorize`, `inspect`, `has`, `raw` | first, also each string of an array |
| `->can()`, `->cannot()`, `->cant()`, `->authorize()` on any object (`$user->can()`, `$this->authorize()`) | first, also each string of an array |
| `->authorizeForUser($user, '…')` | second |
| `@can`, `@cannot`, `@canany`, `@elsecan`, `@elsecannot` in Blade | first, also each string of an array |
| the `can:` middleware, `->middleware('can:…')` | the ability, when it is the only parameter |

```php
Route::get('/admin', AdminController::class)->middleware(['auth', 'admn']);   // Middleware 'admn' not found
Route::put('/posts/{post}', …)->middleware('can:publsh');                      // Ability 'publsh' not found

if (Gate::denies('edit-settings')) { … }                                       // Ability 'edit-settings' not found
$this->authorize('updte', $post);                                               // Ability 'updte' not found
```

```blade
@can('delete', $post) … @endcan
```

## What you see

| Check | Severity | Code | Message |
| --- | --- | --- | --- |
| Middleware | Warning | `laravel.unknown-middleware` | `Middleware '<name>' not found` |
| Ability | Warning | `laravel.unknown-ability` | `Ability '<name>' not found` |

The range is the name only. There is no quick fix.

## Details

### Where the middleware aliases come from

- `bootstrap/app.php`: the keys of `$middleware->alias([...])`, and the group names of `group()`, `appendToGroup()` and `prependToGroup()`.
- `app/Http/Kernel.php` (Laravel 10 and older): the keys of `$middlewareAliases`, `$routeMiddleware` and `$middlewareGroups`.
- The installed Laravel: the keys of `defaultAliases()` and `getMiddlewareGroups()` in `Illuminate\Foundation\Configuration\Middleware` (Laravel 11 and newer: `auth`, `can`, `throttle`, `verified`, `web`, `api`…).
- Without `vendor/laravel/framework`, a built-in list: `auth`, `auth.basic`, `auth.session`, `cache.headers`, `can`, `guest`, `password.confirm`, `precognitive`, `signed`, `throttle`, `verified`, `web`, `api`, `bindings`.

The list is rebuilt when `bootstrap/app.php`, `app/Http/Kernel.php`, `composer.lock` or `vendor/composer/installed.json` changes. Aliases that a package or service provider registers in code (`$router->aliasMiddleware()`) are not seen, and are reported.

### Middleware with parameters

A middleware string with `:` is not checked by its name: in `auth:sanctum`, `throttle:60,1` or `role:admin` the name before the colon is left alone. The only parameter that is checked is the ability in `can:<ability>`, and only when it is the sole parameter: `can:publish` is checked, `can:update,post` is not.

Middleware given as a class (`EnsureUserIsAdmin::class`) is not a string and is never checked.

### Where the abilities come from

- `Gate::define('…')` in any file under `app/` (up to 5,000 files).
- Every public method of every class under an `app/**/Policies/` folder, except `before`, `after` and `__construct`: `update` in `PostPolicy` makes `update` a known ability.

The list is rebuilt when a file under `app/` is saved or changes on disk. Abilities that only exist elsewhere are reported: `Gate::before()` callbacks that allow everything, policies outside a `Policies` folder, gates defined outside `app/`, and permission names that a package such as Spatie Permission resolves through `can()`.

### Skipped strings

Names with `$`, `{` or `*`, concatenated strings, empty strings, strings still being typed and comments. In Blade only the PHP parts are read.

## Settings and permissions

| Setting | Label | Default |
| --- | --- | --- |
| `unknownAuthInspection` | Report unknown middleware and abilities | off |

See [Settings](Settings.md). No optional permission is needed.

## Related

- [Inspections overview](Inspections.md)
- [Middleware, gates and policies](Middleware-Gates-and-Policies.md): completion and Ctrl+click on the same names
- [Middleware in the hub](Hub-Middleware.md)
- [Auth & Policies in the hub](Hub-Auth-and-Policies.md)
- [Packages](Packages.md): Sanctum token abilities and Spatie roles and permissions

---
[← Wiki home](Home.md)
