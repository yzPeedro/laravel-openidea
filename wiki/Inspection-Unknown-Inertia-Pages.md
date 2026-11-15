# Unknown Inertia pages

Reports an Inertia page that has no file, in `Inertia::render('…')`, `inertia('…')` and `Route::inertia('/uri', '…')`. Alt+Enter creates the page.

## How to use it

The check runs by itself in PHP files of projects that require `inertiajs/inertia-laravel`:

```php
return Inertia::render('Home');            // fine: resources/js/Pages/Home.vue
return Inertia::render('Reports/Daily');   // Inertia page 'Reports/Daily' not found
```

## What you see

- **Severity:** Warning.
- **Code:** `laravel.unknown-inertia-page`.
- **Message:** `Inertia page '<name>' not found`.
- **Range:** the page name inside the quotes.

**Quick fix.** Alt+Enter offers *Create Inertia page '&lt;name&gt;'*. It writes the page in the folder and with the extension most of your pages use (`resources/js/Pages/Reports/Daily.vue`); with no page yet, it picks `.vue`, `.svelte`, `.tsx` or `.jsx` from `package.json`. The file has an empty component for its framework, and it opens once created.

## Details

- **Known pages** are the `.vue`, `.tsx`, `.jsx`, `.svelte` and `.ts` files under `resources/js/Pages/`, `resources/js/pages/` and `resources/ts/Pages/`, as in [completion](Packages.md#inertia).
- **The check waits for a first page.** While none of those folders has a page, nothing is reported, so a project that keeps its pages somewhere else isn't flooded with warnings.

## Settings and permissions

| Setting | Label | Default |
| --- | --- | --- |
| `unknownInertiaInspection` | Report unknown Inertia pages | on |

See [Settings](Settings.md). No optional permission is needed.

## Related

- [Inspections overview](Inspections.md)
- [Packages](Packages.md#inertia): completion of Inertia pages
