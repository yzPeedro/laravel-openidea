# Rename translation

Change a translation key in a `lang/` file, press Alt+Enter, and *Rename translation* updates every `__('…')`, `trans('…')`, `@lang('…')` and other usage of the old key across your PHP and Blade files, and renames the same key in the other locales' files. Like [Rename route](Rename-Route.md), it works before and after saving.

## How to use it

1. Open a translation file, for example `lang/en/shop.php`, and change a key:

   ```php
   return [
       'title' => 'Products',      // was 'heading'
       'empty' => 'No products yet.',
   ];
   ```

2. With the cursor inside the new key, press **Alt+Enter**.
3. Pick **Rename translation 'shop.heading' to 'shop.title' in every locale and usage**.

Then:

- `__('shop.heading')`, `trans('shop.heading')`, `@lang('shop.heading')`… become `shop.title` everywhere;
- `'heading'` becomes `'title'` in `lang/pt_BR/shop.php`, `lang/es/shop.php` and every other locale's `shop.php` that has it.

JSON translations work the same way. In `lang/en.json`, change `"Hello"` to `"Hi there"` and the action renames `__('Hello')` in your code and the `"Hello"` key in `lang/pt.json` and the other JSON files.

The action is offered **after** you change the key, not before: the plugin works out the old key from what you changed.

## What you see

One refactoring in the Alt+Enter list:

> Rename translation '&lt;old&gt;' to '&lt;new&gt;' in every locale and usage

Keys are shown in full, with the file name as prefix for PHP files (`shop.heading`, `auth.password.reset`) and as written for JSON files (`Hello`). Applying it edits every file at once through the IDE, so a single undo reverts it.

## Details

### Translation files

| File | Key of `'heading'` / `"Hello"` |
| --- | --- |
| `lang/<locale>/<file>.php` | `<file>.heading`; nested arrays add their keys, so `'password' => ['reset' => …]` in `auth.php` is `auth.password.reset` |
| `lang/<locale>.json` | `Hello`, the key as written |
| `resources/lang/…` | The same, for projects from before Laravel 9 |

The cursor must be on an array key in a PHP file, or on a key (a string followed by `:`) in a JSON file. Values are not keys: editing `'Products'` offers nothing.

### How the old key is found

**Before saving.** When a translation file is opened, the plugin remembers its text as it was on disk. The action is offered when the only difference between the editor and that copy is inside the key under the cursor (the rest of the file must still match the disk), and the old key has no line break or quote.

**After saving.** When a translation file is saved, the plugin lists its keys in order before and after the save. If the file still has the same number of keys, each key that changed is remembered as a rename (the last 32 are kept), and Alt+Enter on the new key offers the action. A remembered rename is forgotten once applied.

### Where usages are found

The PHP files (Blade views included) under `app/`, `routes/`, `resources/views/`, `tests/`, `database/` and `config/` (up to 5000 files per folder), and the file you are editing as it is in the editor. In each file the plugin changes the strings it recognises as translation keys, the same ones [translation completion](Translations.md) works in:

| Call | Example |
| --- | --- |
| `__()`, `trans()`, `trans_choice()` | `__('shop.heading')` |
| `Lang::get()`, `Lang::has()`, `Lang::choice()` | `Lang::has('shop.heading')` |
| `@lang`, `@choice` in Blade | `@lang('shop.heading')` |

A string is changed only when it equals the old key exactly. Keys built with concatenation or variables are left alone, and JavaScript, TypeScript, Vue and Svelte files (`$t('…')`, `t('…')`) are not scanned.

### Other locales

Every translation file under `lang/` and `resources/lang/` (up to 2000 per pattern) except the one you edited is checked:

- in a PHP file with the **same file name** (`shop.php` in every locale), the key at the same path is renamed. Only the last segment changes: `auth.password.reset` → `auth.password.renew` renames `'reset'` inside the `'password'` array;
- in a JSON file, a key equal to the old key is renamed.

A locale that doesn't have the key is left as it is. The values are never changed.

If neither a usage nor another locale has the old key, applying the action fails with "no usage of '&lt;old&gt;' to change", and the IDE log gets a warning.

## Settings and permissions

No setting and no optional permission: the IDE applies the edits. The `locale` setting doesn't matter here: every locale is renamed. See [Permissions](Permissions.md).

## Related

- [Translations](Translations.md)
- [Rename route](Rename-Route.md)
- [Translations section of the hub](Hub-Translations.md)
- [Settings](Settings.md#locale)

---
[← Wiki home](Home.md)
