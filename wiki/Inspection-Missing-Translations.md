# Missing translations

Reports a translation key that exists in some locales but not in others, such as `__('shop.back')` with an English text and no Portuguese one. The quick fix adds the key to the language file of each missing locale, with the existing text to translate.

## How to use it

The check runs by itself on the translation keys of PHP and Blade files: `__()`, `trans()`, `trans_choice()`, `Lang::get()`, `@lang` and the rest of the calls [translation completion](Translations.md) knows.

```php
// lang/en/shop.php:    'back' => 'Back to products', 'title' => 'Products'
// lang/pt_BR/shop.php: 'title' => 'Produtos'

__('shop.back');    // Translation 'shop.back' is missing in pt_BR
__('shop.title');   // fine: every locale has it
__('shop.nope');    // not this check: no locale has it (Alt+Enter offers to add it)
```

## What you see

- **Severity:** Warning.
- **Code:** `laravel.missing-translation`.
- **Message:** `Translation '<key>' is missing in <locales>`, the locales separated by commas.
- **Range:** the key inside its quotes.

**Quick fix.** *Add 'shop.back' to lang/pt_BR/shop.php*, one per missing locale whose file already exists. It appends `'back' => 'Back to products',` before the closing `]` of that file, with the text of the first locale that has it, ready to translate. A key that lives in JSON files (`__('Welcome back')`) is added to `lang/<locale>.json` instead, as `"Welcome back": "<text>"`. When the locale's file doesn't exist yet, there is no fix.

## Details

- **Locales** are the folders and JSON files of `lang/` and `resources/lang/` (without `vendor/`). The check needs at least two locales.
- **The text** copied by the fix is the value as written in the other locale's file, without its quotes.
- A key built with a variable or a concatenation (`__('shop.status.'.$s)`) isn't checked.
- The [Translations](Hub-Translations.md) section shows the same thing for every key, in its **Missing in** column and filter.

## Settings and permissions

| Setting | Label | Default |
| --- | --- | --- |
| `missingTranslationInspection` | Report translation keys that exist in some locales but not in others | on |

## Related

- [Translations](Translations.md)
- [Translations in the hub](Hub-Translations.md)
- [Inspections overview](Inspections.md)

---
[← Wiki home](Home.md)
