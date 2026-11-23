# Find Usages and Rename

The IDE's **Find Usages** and **Rename** work on route names, config keys and translation keys. Find Usages lists the place that defines the key and every `route('…')`, `config('…')` or `__('…')` that uses it, in PHP and Blade. Rename changes the definition and every usage in one step, with a preview.

## How to use it

1. Put the cursor on the **last segment** of the key: `index` in `route('admin.users.index')`, `name` in `config('app.name')`, `welcome` in `__('messages.welcome')`. The definition works too: `->name('users.index')` in a route file, the `'name'` key in `config/app.php`, the `'welcome'` key in `lang/en/messages.php`.
2. Run **Find Usages** or **Rename** from the editor's right-click menu.

```php
// routes/web.php
Route::name('admin.')->group(function () {
    Route::get('/users', [UsersController::class, 'index'])->name('users.index');   // definition
});

// app/Http/Controllers/UsersController.php
return to_route('admin.users.index');                                             // usage

// resources/views/layouts/nav.blade.php
<a href="{{ route('admin.users.index') }}">Users</a>                              // usage
```

Renaming `index` to `list` writes `->name('users.list')` and `route('admin.users.list')` everywhere.

## What is covered

| Key | Defined in | Used in |
| --- | --- | --- |
| Route name | `->name('…')` in `routes/**/*.php`, under `Route::name('…')` group prefixes | `route()`, `to_route()`, `redirect()->route()`, `Route::has()`, `@routeIs` and the other calls that take a route name |
| Config key | The array keys of `config/**/*.php` (nested keys included) | `config()`, `Config::get()` and the other calls that take a config key |
| Translation key | The array keys of `lang/<locale>/*.php` and `resources/lang/<locale>/*.php`, in every locale | `__()`, `trans()`, `trans_choice()`, `@lang` and the other calls that take a translation key |

Usages are read in the same files as [Gutter usages](Gutter-Usages.md#what-counts-as-a-usage): `.php` files under `app/`, `database/`, `routes/`, `tests/`, `config/` and `resources/views/` (Blade included), plus `bootstrap/app.php` and `bootstrap/providers.php`.

## Details

### Why the last segment

Only the last segment of a key is written where it's defined: `config('app.name')` is the key `'name'` inside `config/app.php`, `__('messages.welcome')` is `'welcome'` inside `lang/en/messages.php`, and `admin.users.index` is `->name('users.index')` under a group named `admin.`. So that's the part Find Usages and Rename work on, on both sides. Renaming it keeps the definition and the usages in step; the file, the parent arrays and the group prefix stay as they are. On the other segments, Ctrl+click still opens the definition as before.

### Parent keys

A config or translation array that holds other keys is a key too: `'mail' => ['from' => …]` in `config/app.php` defines `app.mail` and `app.mail.from`. Its name is written inside every child key, so `mail` in `config('app.mail.from')` counts as a usage of `app.mail`. Find Usages on `mail` lists `config('app.mail')` and `config('app.mail.from')`, and renaming it to `contact` writes `'contact' => [...]`, `config('app.contact')` and `config('app.contact.from')`, so no child key is left pointing to an array that no longer exists. The file name (`app`) is never a key, and route names have no parents: a group prefix isn't defined anywhere on its own.

### What is left out

A usage is only indexed when the project defines its key literally, so Rename never changes the usages without the definition:

- routes from `Route::resource()` and `Route::apiResource()` (their names aren't written anywhere), Folio pages and names only in the route cache;
- route names whose group prefix doesn't end with a dot (`Route::name('admin')` + `->name('users')`);
- config keys that only exist in a package or the framework (`config('queue.default')` without a `config/queue.php`);
- JSON translations (`lang/pt_BR.json`), package translations (`__('package::file.key')`) and `lang/vendor/`;
- keys that contain a dot themselves (`'mail.from' => …`), concatenated or interpolated strings, and strings with escapes.

Views aren't indexed either: renaming one means moving its file, which Rename can't do from a string. [Hover](Hover.md) and Ctrl+click still work on all of these.

### Rename

- Rename always shows a preview, even when every change is in one file, and applies as one undoable change.
- It changes the strings the index knows. A key built at runtime (`config("app.$key")`) or read from somewhere else (a database, a JS file) isn't changed: check the preview, and search the project for the old key when in doubt.
- [Rename route](Rename-Route.md) and [Rename translation](Rename-Translation.md) still work the other way round: edit the name first, then Alt+Enter to update the usages.

### How it works

The keys come from the plugin's document indexer, the same one behind [Gutter usages](Gutter-Usages.md). The IDE keeps the results in memory and answers Find Usages and Rename from them, following your edits whether saved or not. Requires OpenIDEA 0.1.40 or newer (document indexer references, plugin API 1.3).
