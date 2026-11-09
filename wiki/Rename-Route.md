# Rename route

Change a route's name in its `->name('…')` call, press Alt+Enter, and *Rename route* updates every `route('…')`, `to_route('…')`, `redirect()->route('…')` and other usage of the old name across your PHP and Blade files. It works right after you type the new name, before saving, and after you saved.

## How to use it

1. Open a route file (any PHP file under `routes/`) and change the name inside `->name('…')`:

   ```php
   Route::get('/products', [ProductController::class, 'index'])->name('products.list');
   //                                                    was 'products.index' ↑
   ```

2. With the cursor inside the new name, press **Alt+Enter**.
3. Pick **Rename route 'products.index' to 'products.list' in every usage**.

Every usage of `products.index` in the project becomes `products.list`:

```php
return redirect()->route('products.list');
```

```blade
<a href="{{ route('products.list') }}">Products</a>
```

The action is offered **after** you change the name, not before: the plugin works out the old name from what you changed. There is no rename dialog to open first.

## What you see

One refactoring in the Alt+Enter list, titled with both full names:

> Rename route '&lt;old&gt;' to '&lt;new&gt;' in every usage

Applying it edits every file at once through the IDE, so a single undo reverts it. The route definition itself isn't touched again: you already changed it.

## Details

### How the old name is found

**Before saving.** When a route file is opened, the plugin remembers its text as it was on disk (it keeps up to 64 route and translation files). When you ask for code actions, it compares the editor's text with that copy. The action is offered when:

- the cursor is in a string that is the argument of `->name(` (or `?->name(`) in a file under `routes/`;
- the only difference between the two texts is inside that string: the rest of the file must still match the file on disk;
- the old value has no line break or quote.

So rename first, then save; or rename, save, then use the action (below). If you also edited other lines of the file before saving, the before-saving comparison doesn't apply, and the after-saving one takes over once you save.

**After saving.** When a route file is saved, the plugin lists the route names it defines, in order, before and after the save. If the file still defines the same number of names, each position whose name changed is remembered as a rename (the last 32 are kept). Alt+Enter on the new name, in that file, then offers the action. A remembered rename is forgotten once you apply it.

### Group prefixes

The full name includes the prefix of the group the route is in:

```php
Route::name('admin.')->group(function () {
    Route::get('/users', UsersController::class)->name('users.list'); // was users.index
});
```

The action reads **Rename route 'admin.users.index' to 'admin.users.list' in every usage**, and updates `route('admin.users.index')`. Only the route's own `->name()` is handled: changing the group's `Route::name('admin.')` prefix doesn't offer the action.

### Where usages are found

The plugin scans the PHP files (Blade views included) under:

| Folder | Up to |
| --- | --- |
| `app/` | 5000 files |
| `routes/` | 5000 files |
| `resources/views/` | 5000 files |
| `tests/` | 5000 files |
| `database/` | 5000 files |
| `config/` | 5000 files |

and the file you are editing, as it is in the editor. In each file it changes the strings it recognises as route names, the same ones [route name completion](Route-Names.md) works in:

| Call | Example |
| --- | --- |
| `route()`, `to_route()` | `route('products.index')` |
| `->route()`, `->signedRoute()`, `->temporarySignedRoute()` | `redirect()->route('products.index')`, `URL::signedRoute('products.index')` |
| `URL::route()`, `Redirect::route()` and their signed variants | `Redirect::route('products.index')` |
| `Route::has()` | `Route::has('products.index')` (every argument) |
| `->routeIs()`, `->named()` | `request()->routeIs('products.index')` (every argument) |

A string is changed only when it equals the old name exactly; strings built with concatenation or variables are left alone. `$request->route('…')`, `request()->route('…')` and `$this->route('…')` read route parameters, not names, and are skipped.

Not covered: JavaScript and TypeScript files (Ziggy's `route('…')`), and route names in comments or plain strings that aren't route arguments.

If no file uses the old name, applying the action fails with "no usage of '&lt;old&gt;' to change", and the IDE log gets a warning.

### With the unknown route inspection

If you turned on `unknownRouteInspection`, usages of the old name are reported as unknown routes as soon as you save the new one, and the warnings go away when you apply the rename.

## Settings and permissions

No setting and no optional permission: the IDE applies the edits. See [Permissions](Permissions.md).

## Related

- [Route names](Route-Names.md)
- [Rename translation](Rename-Translation.md)
- [Unknown route names](Inspection-Unknown-Routes.md)
- [Unknown views](Inspection-Unknown-Views.md) (renamed or moved views)
- [Routes](Hub-Routes.md)

---
[← Wiki home](Home.md)
