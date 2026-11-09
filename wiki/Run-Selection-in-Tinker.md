# Run Selection in Tinker

Select some PHP in the editor, right-click and pick *Laravel: Run Selection in Tinker*: a terminal opens `artisan tinker` with the file's imports, shows the value of each statement of the selection, and stays open at the Tinker prompt with the selection's variables defined, so you can keep exploring.

## How to use it

1. In a PHP file of a Laravel project, select the code to run, such as:

   ```php
   $count = Post::count();
   Post::first();
   ```

2. Right-click and pick **Laravel: Run Selection in Tinker**, or run it from the action search. It only shows when the editor has a selection.
3. A terminal tab titled **artisan tinker (selection)** opens, prints each value and waits at the prompt. `$count` is defined there.
4. Type `exit` (or close the tab) when you're done.

## What you see

The plugin writes the selection to `storage/openidea/selection.php`, turned into a script that dumps each statement:

```php
<?php

use App\Models\Post;

dump($count = Post::count());
dump(Post::first());
```

and starts Tinker with it:

```sh
php artisan tinker storage/openidea/selection.php
```

Tinker runs the file, so you see one dump per statement, then hands you its prompt in the same session.

## Details

### How the script is built

1. **Imports.** Every `use …;` statement of the file you're in that names a class (contains a `\`) is copied to the top. Then each of your models used in the selection as `Post::`, `new Post` or `Post $` is imported (`use App\Models\Post;`) when the file doesn't import it already. Models come from the [model index](Model-Members.md), with the namespace written in each model's file.
2. **Statements.** A leading `<?php` is dropped, and the selection is split into statements at each `;` outside brackets, braces and strings (comments are skipped). A missing final `;` is added.
3. **Dumps.** Each statement is wrapped in `dump(…)`, so assignments still assign (`dump($count = Post::count())`). A statement is left as it is when it:
   - starts with a control or declaration keyword: `if`, `else`, `elseif`, `foreach`, `for`, `while`, `do`, `switch`, `try`, `catch`, `finally`, `function`, `class`, `interface`, `trait`, `enum`, `namespace`, `use`, `return`, `echo`, `print`, `throw`, `unset`, `global`, `static`, `const`, `abstract`, `final`, `readonly` or `declare`;
   - ends with `}` (a block, a closure assignment);
   - already calls `dump(` or `dd(`;
   - is a comment.

### Where it runs

The script is written by `php/selection.php`, which creates `storage/openidea` with its `.gitignore` (`*`) when needed, and overwrites `selection.php` on every run. Then the terminal runs Tinker with your [runner](Settings.md#artisanrunner):

| Runner | Terminal command |
| --- | --- |
| `php` | `php artisan tinker storage/openidea/selection.php` |
| `docker-run` | `docker compose run --rm --no-deps --entrypoint php <service> artisan tinker storage/openidea/selection.php` |
| `docker` | `docker compose exec <service> php artisan tinker storage/openidea/selection.php` |

The path is relative to the project root, so it works inside the container too. The terminal is interactive and stays open while Tinker runs.

### Messages

| Message | Why |
| --- | --- |
| Select the code first, or place the cursor in a query. | Nothing is selected |
| this file is not in a Laravel project | The file's project has no `artisan` and `laravel/framework` |
| Allow "process:php" for Laravel OpenIDEA in Settings › Plugins to run Tinker. | Writing the script or opening the terminal was refused |
| Couldn't prepare the selection for Tinker: … | `selection.php` couldn't be written, with PHP's error |

### Compared with the hub's Tinker

The hub's [Tinker](Hub-Tinker.md) section runs code in tabs and shows Tinker-style results in the hub, with echo output, `dump()` calls, SQL queries and timing, and remembers your tabs. *Run Selection in Tinker* uses the real `artisan tinker` in a terminal, which is handy when you want to keep a session open with the selection's variables.

## Settings and permissions

- **`editor.read`** to read the selection and the file's imports.
- **`process:php`** (or **`process:docker`**) to write the script and open the terminal.
- `artisanRunner` and `dockerService` choose how Tinker runs.

See [Permissions](Permissions.md) and [Settings](Settings.md).

## Related

- [Tinker](Hub-Tinker.md)
- [Copy Query as MySQL](Copy-Query-as-MySQL.md)
- [Commands and shortcuts](Commands.md) (*Laravel: Open Tinker*)
- [SQL to Laravel](Hub-SQL-to-Laravel.md)

---
[← Wiki home](Home.md)
