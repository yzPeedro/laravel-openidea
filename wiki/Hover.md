# Hover

Hover a Laravel string and the editor's tooltip says what it is: the route's URI and controller, the config value, the translation in each locale, the view's file, and where each one is defined. It shows below the PHP language server's hover, and it works in PHP and Blade files.

## How to use it

Rest the mouse on the string, or press the IDE's *Show Hover* shortcut with the cursor inside it:

```php
return redirect()->route('admin.users.index');   // Route · → UsersController@index · GET /admin/users · routes/web.php:12
$name = config('app.name');                      // Config · env('APP_NAME') · config/app.php:16
echo __('messages.welcome');                     // Translation · en: 'Welcome' · pt_BR: 'Bem-vindo'
return view('orders.show');                      // View · resources/views/orders/show.blade.php
```

```blade
<a href="{{ route('home') }}">{{ __('messages.home') }}</a>
<x-alert type="error" />
```

## What you see

The first line names the kind and the key, such as **Route** `admin.users.index`. Below it:

| Kind | Shows |
| --- | --- |
| Route | The controller method it runs (`→ UsersController@index`), the verb and URI, and the route file and line |
| Config key | The value written in the config file (`'local'`, `true`, `env('APP_NAME')`), and the file and line |
| Translation | The text in each locale that has the key, with its file and line |
| View, Blade component, Livewire component, Inertia page | The file |
| Env variable | Where it's declared (`.env` or `.env.example`) |
| Middleware, abilities, disks, guards, queues, mailers and the other names the plugin completes | Their detail and where they're defined |

At most eight definitions are listed, then *… and N more*.

## Details

- The hover covers every string the plugin completes and opens with Ctrl+click ([Go to definition](Go-to-Definition.md)), and only when the key is defined: an unknown name shows nothing (the [inspections](Inspections.md) report those).
- It's built from the plugin's index of your files only. It never starts PHP, so it stays fast, and it shows what the files say, not what the app resolves at runtime: a config value is the literal in the file, not the value after `.env` and the config cache.
- `.env` values are never shown. The plugin keeps the values of `.env.example` only, the same as completion.
- Concatenated or interpolated strings (`'admin.' . $name`, `"app.$key"`) have no hover.
- Requires OpenIDEA 0.1.40 or newer (hover provider, plugin API 1.3).
