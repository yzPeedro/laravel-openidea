# Unknown Livewire components

Reports a Livewire component that the project doesn't have, in `<livewire:…>` tags and `@livewire('…')`. Alt+Enter creates the component class and its view.

## How to use it

The check runs by itself in PHP and Blade files of projects with Livewire 3 or older installed:

```blade
<livewire:cart-summary />     {{-- fine: app/Livewire/CartSummary.php --}}
<livewire:user-table />       {{-- Livewire component 'user-table' not found --}}
@livewire('admin.stats')      {{-- Livewire component 'admin.stats' not found --}}
```

## What you see

- **Severity:** Warning.
- **Code:** `laravel.unknown-livewire-component`.
- **Message:** `Livewire component '<name>' not found`.
- **Range:** the component name.

**Quick fix.** Alt+Enter offers *Create Livewire component '&lt;name&gt;' (app/Livewire/…)*. It writes the class (`admin.user-table` → `app/Livewire/Admin/UserTable.php`, extending `Livewire\Component` with a `render()` that returns its view) and the view (`resources/views/livewire/admin/user-table.blade.php`) when it doesn't exist yet, then opens the class. One undo reverts both.

## Details

- **Known components** are the classes under `app/Livewire/` and `app/Http/Livewire/` and the views under `resources/views/livewire/`, as in [completion](Packages.md#livewire).
- **Livewire 4 isn't checked yet.** Its single-file and multi-file components live elsewhere, so the inspection only runs when `vendor/composer/installed.json` says Livewire 3 or older is installed.
- **Skipped:** `<livewire:styles>`, `<livewire:scripts>` and namespaced names (`package::component`).
- In Blade the warnings are refreshed when the file is opened or saved, and right after a component is created.

## Settings and permissions

| Setting | Label | Default |
| --- | --- | --- |
| `unknownLivewireInspection` | Report unknown Livewire components (Livewire 3 and older) | on |

See [Settings](Settings.md). No optional permission is needed.

## Related

- [Inspections overview](Inspections.md)
- [Packages](Packages.md#livewire): completion of Livewire components
