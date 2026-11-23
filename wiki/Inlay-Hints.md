# Inlay hints

Short labels inside the code tell you what a Laravel string or route resolves to, without hovering or opening another file: the full URI and name of a route under group prefixes, the routes behind a controller method, the verb and URI of a `route('…')`, a config value and a translation's text. They come from the plugin's index of your files, never start PHP and never show `.env` values.

## What you see

### Route files

At the end of a route that a group changes, the full URI and name it ends up with:

```php
Route::prefix('admin')->name('admin.')->group(function () {
    Route::get('/stats', [StatsController::class, 'index'])->name('stats');   // GET /admin/stats · admin.stats
});

Route::resource('categories', CategoryController::class)->only(['index', 'update']);   // /categories · categories.{index,update}
```

A route outside any group, or in a group that changes neither its URI nor its name, gets no hint: there is nothing more to say than what's written.

### Controllers

After the signature of each public method that a route points to, its verb and URI, several separated by commas:

```php
public function index()                                        // GET /categories
public function update(Request $request, Category $category)   // PUT /categories/{category}
```

Resource routes count too (`Route::resource()` and `Route::apiResource()`, with `only` and `except`).

### Route names, config keys and translations

Right after the string, in PHP and Blade:

| Code | Hint |
| --- | --- |
| `route('shop.index')`, `to_route()`, `redirect()->route()`… | `GET /shop` |
| `config('app.env')` | `= 'local'`, the value written in the config file |
| `config('app.name')` | `= env('APP_NAME')`: the `.env` value itself is never shown |
| `__('shop.title')`, `trans()`, `@lang`… | `= 'Shop'`, the text in your preferred locale (hover the hint to see which) |

A key the project doesn't define, a config key that holds an array, a translation whose value isn't plain text, and concatenated or interpolated strings (`route('admin.' . $name)`) have no hint.

## Settings

Three switches in **Settings › Plugins › Laravel OpenIdea**, all on by default:

| Key | Turns on or off |
| --- | --- |
| `inlayRoutes` | Route files, controller methods and `route('…')` usages |
| `inlayConfig` | Config values |
| `inlayTranslations` | Translation text |

The IDE's own **Inlay Hints** settings apply on top: the plugin's hints are of the *other* kind. The translation text follows the `locale` setting (see [Settings](Settings.md)).

## Details

- Hints are computed for the visible part of the file and asked again after edits and scrolling, so they follow unsaved changes.
- Labels longer than 60 characters are cut with `…`.
- Requires OpenIDEA 0.1.40 or newer (inlay hints provider, plugin API 1.3).

See also [Hover](Hover.md), which shows the same information and where each key is defined.
