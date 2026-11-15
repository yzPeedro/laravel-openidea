# Unknown config keys

Reports a config key that its config file doesn't define, in `config('…')`, `Config::get('…')` and the other places a config key is read. A typo or a key that was removed shows up as a warning instead of a silent `null` at runtime.

## How to use it

The check runs by itself in PHP and Blade files:

```php
config('app.nope');                 // Config key 'app.nope' not found
config('services.mailgun.domain');  // Config key 'services.mailgun.domain' not found, when config/services.php has no mailgun
config('cors.paths');               // fine: Laravel's own default, even without config/cors.php
config('sanctum.guard');            // not checked: no config/sanctum.php, a package may provide it
```

## What you see

- **Severity:** Warning.
- **Code:** `laravel.unknown-config-key`.
- **Message:** `Config key '<key>' not found`.
- **Range:** the key inside the quotes.
- **Quick fix:** none.

## Details

- **Laravel's own defaults count.** Since Laravel 11 the framework ships its config files in `vendor/laravel/framework/config` and merges them with yours, so a project can read `config('cors.paths')` or `config('view.compiled')` without a `config/cors.php` or `config/view.php`. The plugin reads those files too. A top-level key you define in your own file replaces the framework's whole value, as Laravel does. Completion, Ctrl+click and [hover](Hover.md) show these keys as well, with the framework file as their location.
- **Only keys of a known file are checked.** When the first segment matches no config file of the project or the framework (`sanctum`, `horizon`, a package's config merged with `mergeConfigFrom()`), the key isn't reported.
- **Only literal arrays are checked.** A key is reported when its closest existing parent is an array written in the file. Values built by code (`'stores' => array_merge(…)`) and keys under a value that isn't an array are left alone.
- **Skipped:** numeric segments (`config('app.providers.0')`), concatenated or interpolated keys, and values set at runtime with `config([...])`.

## Settings and permissions

| Setting | Label | Default |
| --- | --- | --- |
| `unknownConfigInspection` | Report unknown config keys | on |

See [Settings](Settings.md). No optional permission is needed.

## Related

- [Inspections overview](Inspections.md)
- [Config keys](Config-Keys.md): completion and Ctrl+click on config keys
