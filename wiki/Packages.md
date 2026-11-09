# Packages

Beyond the framework, the plugin understands the strings of the packages Laravel apps use most: Livewire components, Inertia pages, Folio pages, Sanctum guards and token abilities, Spatie Permission roles and permissions, Socialite drivers, swagger-php OpenAPI attributes and the pivot attributes of many-to-many relations. Each one completes where the package reads it, and Ctrl+click opens the file or definition behind it.

None of these features need the package's code to run: the plugin reads your project's files. Most of them also work before `composer install`.

## Livewire

| Where | Suggests |
| --- | --- |
| `<livewire:` in Blade | Livewire components |
| `@livewire('…')` | Livewire components |
| `->layout('…')` in a component | views |

Components come from the classes in `app/Livewire/` and `app/Http/Livewire/` (`UserTable.php` → `user-table`, `Admin/Users.php` → `admin.users`) and from the views in `resources/views/livewire/` (folders become dots, a trailing `index` is dropped). Ctrl+click on the name in a tag or in `@livewire()` opens the class or view.

With `livewire/livewire` installed, `@livewireStyles`, `@livewireScripts`, `@livewireScriptConfig`, `@persist`/`@endpersist` and `@teleport`/`@endteleport` complete after `@`; with `livewire/volt`, `@volt`/`@endvolt`.

*Laravel: Go to Related Class or View* switches between a component class and its view ([Go to Related Class or View](Related-Class-or-View.md)).

## Inertia

| Call | Argument |
| --- | --- |
| `inertia('…')` | 1st |
| `Inertia::render('…')` | 1st |
| `Route::inertia('/uri', '…')` | 2nd |

```php
return Inertia::render('Products/Index', ['products' => $products]);
Route::inertia('/about', 'About');
```

Pages are the `.vue`, `.tsx`, `.jsx`, `.svelte` and `.ts` files under `resources/js/Pages/`, `resources/js/pages/` and `resources/ts/Pages/`. The name is the path below that folder without the extension (`Products/Index`); the detail is the file. Ctrl+click opens the page.

`@inertia` and `@inertiaHead` complete after `@` when `inertiajs/inertia-laravel` is installed.

## Folio

[Laravel Folio](https://laravel.com/docs/folio) pages are routes. Their URIs appear in the hub's Routes section, and pages that call `name('…')` complete in `route('…')` and open with Ctrl+click. The plugin reads the mounts from `app/Providers/FolioServiceProvider.php` (`Folio::path(resource_path('…'))` or `Folio::route(…)`, with `->uri('…')` or `uri: '…'`), or uses `resources/views/pages` at `/` when that provider is missing and `composer.json` requires `laravel/folio`.

| Page file | URI |
| --- | --- |
| `index.blade.php` | `/` |
| `users/index.blade.php` | `/users` |
| `users/[id].blade.php` | `/users/{id}` |
| `users/[User].blade.php` | `/users/{user}` |
| `posts/[.App.Models.Post:slug].blade.php` | `/posts/{post:slug}` |
| `docs/[...pages].blade.php` | `/docs/{pages...}` |
| `dashboard.blade.php` mounted with `uri: '/admin'` | `/admin/dashboard` |

Files and folders whose name starts with `_` are skipped. See [Route names](Route-Names.md#folio-pages).

## Sanctum

- **Guard**: when `composer.json` mentions `laravel/sanctum` and `config/auth.php` has no `sanctum` guard, `sanctum` is added to the guards, so `auth:sanctum` and `auth('sanctum')` complete (detail *Laravel Sanctum*).
- **Token abilities** complete in:

  | Where | Argument |
  | --- | --- |
  | `->tokenCan('…')`, `->tokenCant('…')` | 1st |
  | `->createToken('name', ['…'])` | 2nd, inside the array |
  | `abilities:` and `ability:` middleware | every parameter |

  ```php
  $token = $user->createToken('mobile', ['orders:read', 'orders:write']);
  if ($request->user()->tokenCan('orders:write')) { /* … */ }
  Route::middleware(['auth:sanctum', 'abilities:orders:read'])->get(…);
  ```

  The abilities are the strings in the array of every `->createToken('…', [...])` in files under `app/` and `routes/`, with the file as detail. Ctrl+click opens the place they are created.

## Spatie Laravel Permission

**Roles** complete in:

| Where | Argument |
| --- | --- |
| `->hasRole()`, `->hasAnyRole()`, `->hasAllRoles()`, `->hasExactRoles()`, `->assignRole()`, `->removeRole()`, `->syncRoles()`, `->role()` (the scope, `User::role('admin')`) | any, arrays too |
| `Role::findByName('…')`, `Role::findOrCreate('…')` | 1st |
| `@role`, `@hasrole`, `@hasanyrole`, `@hasallroles`, `@unlessrole`, `@hasexactroles` | 1st, arrays too |
| `role:` and `role_or_permission:` middleware | every parameter |

**Permissions** complete in:

| Where | Argument |
| --- | --- |
| `->hasPermissionTo()`, `->hasAnyPermission()`, `->hasAllPermissions()`, `->givePermissionTo()`, `->revokePermissionTo()`, `->syncPermissions()`, `->checkPermissionTo()`, `->hasDirectPermission()`, `->permission()` | any, arrays too |
| `Permission::findByName('…')`, `Permission::findOrCreate('…')` | 1st |
| `@haspermission`, `@hasanypermission`, `@hasallpermissions` | 1st, arrays too |
| `permission:` middleware | every parameter |
| every ability context: `Gate::allows()`, `->can()`, `@can`, `can:`… | as for abilities |

```php
$user->assignRole('editor');
$user->givePermissionTo(['edit articles', 'publish articles']);
Route::middleware('role:admin')->group(…);
```

The names are read from files under `database/seeders/`, `database/migrations/` and `app/` that import Spatie's models (they contain `Permission\`):

- `Role::create(['name' => '…'])`, `Role::firstOrCreate(['name' => '…'])`, `Role::updateOrCreate(['name' => '…'])`;
- `Role::findOrCreate('…')` and `Role::create('…')`;
- the same calls on `Permission`.

The detail is the file, and Ctrl+click opens the place where the role or permission is created. With the package installed, the `@role` family of directives completes after `@` (see [Blade](Blade.md#directives)).

## Socialite

`Socialite::driver('…')`, `Socialite::with('…')` and `Socialite::buildProvider('…')` complete:

1. every service in `config/services.php` that has a `client_id` (detail `config/services.php`; Ctrl+click opens it);
2. the providers Socialite ships: `bitbucket`, `facebook`, `github`, `gitlab`, `google`, `linkedin`, `linkedin-openid`, `slack`, `slack-openid`, `twitch`, `twitter`, `twitter-oauth-2` and `x` (detail *Socialite provider*).

## OpenAPI (swagger-php attributes)

In swagger-php attributes and annotations (`#[OA\Get(…)]`, `new OA\Property(…)`, written with the `OA` alias or a namespace ending in `OpenApi\Attributes` or `OpenApi\Annotations`), the plugin completes these named arguments:

| Argument | Where | Suggests |
| --- | --- | --- |
| `ref:` | any | `#/components/schemas/…`, `#/components/responses/…`, `#/components/parameters/…`, `#/components/requestBodies/…`, `#/components/headers/…`, `#/components/securitySchemes/…`, `#/components/examples/…` |
| `tags:` | any | tag names |
| `security:` | any, as array keys | security scheme names |
| `in:` | any | `path`, `query`, `header`, `cookie` |
| `type:` | any | `string`, `integer`, `number`, `boolean`, `array`, `object`, `null` |
| `format:` | any | `int32`, `int64`, `float`, `double`, `byte`, `binary`, `date`, `date-time`, `time`, `duration`, `password`, `email`, `idn-email`, `hostname`, `ipv4`, `ipv6`, `uri`, `uri-reference`, `url`, `uuid`, `regex` |
| `required:` | `Schema`, `JsonContent`, `XmlContent`, `Items`, `Property`, `AdditionalProperties` | the `property:` names of the `OA\Property` entries inside the same attribute |
| `path:` | `Get`, `Post`, `Put`, `Patch`, `Delete`, `Options`, `Head`, `Trace`, `PathItem` | your route URIs |
| `name:` | `Parameter` | the `{placeholders}` of the nearest `path:` before it |

```php
#[OA\Post(
    path: '/users',
    tags: ['Users'],
    security: [['bearerAuth' => []]],
    parameters: [new OA\Parameter(name: 'id', in: 'path', schema: new OA\Schema(type: 'integer', format: 'int64'))],
    responses: [new OA\Response(response: 404, content: new OA\JsonContent(ref: '#/components/schemas/UserNotFound'))],
)]
```

Components are read from files under `app/`, `docs/` and `routes/` that contain `OA\` or `OpenApi\`: `schema:` of `Schema`, `response:` of `Response` (except status codes and `default`), `parameter:` of `Parameter`, `request:` of `RequestBody`, `header:` of `Header`, `securityScheme:` of `SecurityScheme`, `example:` of `Examples`, and tags from `OA\Tag(name: …)` and every `tags: [...]`. Ctrl+click on a `ref:` opens the component's definition. The `unknownOpenApiRefInspection` setting reports refs to components that don't exist ([Unknown OpenAPI refs](Inspection-Unknown-OpenAPI-Refs.md)).

## Pivot attributes

After `->pivot->` (or the accessor renamed with `->as('…')`), the plugin completes the pivot table's attributes:

```php
foreach ($user->roles as $role) {
    $role->pivot->expires_at;
}
```

The attributes are read from every public method of your models that returns a `belongsToMany`, `morphToMany` or `morphedByMany` relation:

| Relation | Attributes |
| --- | --- |
| `belongsToMany(Role::class)` | the two keys: the 3rd and 4th arguments, or `user_id` and `role_id` by convention |
| `morphToMany(Tag::class, 'taggable')` | `taggable_id`, `taggable_type` and the related key (`tag_id`) |
| `morphedByMany(Post::class, 'taggable')` | `taggable_id`, `taggable_type` and this model's key |
| `->withPivot('…', '…')` | every column named |
| `->withTimestamps()` | `created_at` and `updated_at` (or the names given) |
| `->as('subscription')` | the accessor becomes `subscription` instead of `pivot` |

Each attribute shows *pivot of `User::roles()`* as detail. Suggestions only appear when some relation defines that accessor; the accessor is matched by name, whatever the variable before it.

## Details

- Facade and method names are matched case-insensitively; fully qualified facades work.
- Package directives (`@livewire…`, `@inertia`, `@routes`, `@role`…) are only suggested when the package's `vendor/<package>/composer.json` exists; without `vendor/` at all, they are always suggested.
- Inertia pages are reindexed when a file under `resources/js/` changes; roles and permissions when a file under `database/` or `app/` changes; OpenAPI components and token abilities when a file under `app/`, `docs/`, `routes/` or `config/` changes.

## Settings and permissions

- `unknownOpenApiRefInspection` (on by default) reports `$ref`s to missing components.
- `unknownAuthInspection` (off by default) also covers permissions used as abilities.
- Everything here only needs `project.read`. See [Settings](Settings.md) and [Permissions](Permissions.md).

## Related

- [Blade](Blade.md): components, Livewire tags and package directives
- [Route names](Route-Names.md): Folio and the route cache
- [Middleware, gates and policies](Middleware-Gates-and-Policies.md)
- [Eloquent columns and relations](Eloquent-Columns-and-Relations.md)
- [Unknown OpenAPI refs](Inspection-Unknown-OpenAPI-Refs.md)
- [Ecosystem in the hub](Hub-Ecosystem.md) and [Frontend in the hub](Hub-Frontend.md)

---
[← Wiki home](Home.md)
