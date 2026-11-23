# Missing controller methods

Reports a route that points to a controller method that doesn't exist, in the route files: a `[Controller::class, 'method']` action, an invokable controller without `__invoke()`, and the actions of a `Route::resource()` or `Route::apiResource()` (following `only` and `except`) that the controller doesn't have. Alt+Enter writes the missing methods.

## How to use it

The check runs by itself in the PHP files under `routes/`:

```php
Route::get('/gallery', [PhotoController::class, 'gallery']);          // Method PhotoController::gallery() not found
Route::resource('photos', PhotoController::class)
    ->only(['index', 'store', 'destroy']);                             // PhotoController has no store(), destroy()
Route::get('/report', ReportController::class);                       // Method ReportController::__invoke() not found
```

## What you see

- **Severity:** Warning.
- **Code:** `laravel.missing-controller-method`.
- **Message:** `Method <Controller>::<method>() not found`, or `<Controller> has no <method>(), <method>()` for a resource.
- **Range:** the method name, the controller class of an invokable route, or the resource name.

**Quick fix.** With the cursor on the underlined text, Alt+Enter offers *Create &lt;Controller&gt;::&lt;method&gt;()*. It adds empty public methods at the end of the controller, with the signatures `make:controller --resource` writes (`store(Request $request)`, `show(string $id)`, `update(Request $request, string $id)`, `destroy(string $id)`, plain `name()` for anything else), adds `use Illuminate\Http\Request;` when a method needs it, and opens the controller. When the file already names the HTTP request under an alias (`use Illuminate\Http\Request as HttpRequest;`), the methods use that alias. When `Request` already means another class, such as the `Illuminate\Support\Facades\Request` facade, nothing is imported and the methods write `\Illuminate\Http\Request` in full, so the controller keeps compiling. One undo reverts it.

## Details

- **Where the controller comes from.** The class is resolved from the route file's `use` imports or its fully qualified name, and its file from the PSR-4 `autoload` of `composer.json`.
- **Inheritance is followed inside the project.** Public methods of parent classes in your own code count (`Controller` and any base controller of yours). When a parent class lives outside the project, or the controller uses a trait, the route isn't checked: the method may come from there.
- **Only public methods count**, as Laravel only calls public actions.

## Settings and permissions

| Setting | Label | Default |
| --- | --- | --- |
| `missingControllerMethodInspection` | Report routes that point to controller methods that don't exist | on |

See [Settings](Settings.md). No optional permission is needed.

## Related

- [Inspections overview](Inspections.md)
- [Inlay hints](Inlay-Hints.md): the routes behind each controller method
