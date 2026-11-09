# Go to Related Class or View

*Laravel: Go to Related Class or View* jumps between a Livewire component or a Blade class component and its view, both ways: from `app/Livewire/Admin/UserTable.php` to `resources/views/livewire/admin/user-table.blade.php` and back.

## How to use it

1. Open a Livewire component class, a Blade component class, or the view of either.
2. Run **Laravel: Go to Related Class or View** from the action search (type `Laravel:`), or from a key you bound in **Settings › Keymap** ([Commands and shortcuts](Commands.md#bind-keys)).
3. The related file opens at its first line, and the IDE reports "Opened &lt;path&gt;".

## What you see

| Open file | Opens |
| --- | --- |
| `app/Livewire/Admin/UserTable.php` | `resources/views/livewire/admin/user-table.blade.php` |
| `app/Http/Livewire/Admin/UserTable.php` (Livewire 2) | `resources/views/livewire/admin/user-table.blade.php` |
| `app/View/Components/Forms/TextInput.php` | `resources/views/components/forms/text-input.blade.php` |
| `resources/views/livewire/admin/user-table.blade.php` | `app/Livewire/Admin/UserTable.php`, or `app/Http/Livewire/Admin/UserTable.php` |
| `resources/views/livewire/orders/index.blade.php` | `app/Livewire/Orders/Index.php`, or `app/Http/Livewire/Orders/Index.php`, or `app/Livewire/Orders.php` |
| `resources/views/components/forms/text-input.blade.php` | `app/View/Components/Forms/TextInput.php` |

## Details

- **Names.** From a class, each folder and the file name are turned into kebab case: every upper-case letter after the first starts a new word (`UserTable` → `user-table`, `Admin` → `admin`). From a view, each part is turned into studly case at `-` and `_` (`user-table` → `UserTable`, `text_input` → `TextInput`).
- **Order.** The plugin builds the list of candidates for the open file and opens the first one that exists. For a Livewire view it tries `app/Livewire/…` first, then `app/Http/Livewire/…`; for a view named `index.blade.php` it also tries the class named after its folder.
- **Missing file.** When none of the candidates exists, the command fails with "&lt;first candidate&gt; does not exist yet", such as an anonymous Blade component without a class, or a Volt component.
- **Other files.** In any other file (a model, a controller, a regular view), it fails with "no related file: open a Livewire or Blade component class or its view".
- **No file open.** "open a file first". Outside a Laravel project: "this file is not in a Laravel project".
- **Paths** are compared from the project root, with `\` and `/` treated alike, so it works the same on Windows.

## Settings and permissions

No setting and no optional permission. The command reads the current editor through the command's context and opens the file through the IDE. See [Permissions](Permissions.md).

## Related

- [Blade](Blade.md)
- [Components](Hub-Components.md)
- [Views](Views.md)
- [Packages](Packages.md) (Livewire)
- [Commands and shortcuts](Commands.md)

---
[← Wiki home](Home.md)
