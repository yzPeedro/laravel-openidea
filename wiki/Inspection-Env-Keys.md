# `.env` keys

Compares `.env` with `.env.example` and reports the keys that are in one file but not the other. A variable you add to your `.env` but forget in `.env.example` leaves the next person who clones the project without it; a key in `.env.example` that your `.env` lacks may be a setting you never configured. The quick fix copies the key across.

## How to use it

Open or save `.env` or `.env.example` in a Laravel project that has both files. Both files are checked together, each against the other:

```dotenv
# .env
APP_NAME=Shop
STRIPE_SECRET=your-secret
```

```dotenv
# .env.example
APP_NAME=Laravel
MAIL_HOST=127.0.0.1
```

With these two files, `STRIPE_SECRET` is reported in `.env` and `MAIL_HOST` in `.env.example`.

## What you see

| File | Severity | Message |
| --- | --- | --- |
| `.env` | Warning | `<KEY> is missing from .env.example, so new checkouts won't know about it` |
| `.env.example` | Info | `<KEY> is in .env.example but not in .env` |

- **Code:** `laravel.env-keys`.
- **Range:** the key name at the start of the line.

**Quick fix.** Alt+Enter on the key offers *Add &lt;KEY&gt; to &lt;other file&gt;*, which appends a line at the end of the other file (adding a line break first if the file doesn't end with one):

| From | Line added | Why |
| --- | --- | --- |
| `.env` | `KEY=` to `.env.example`, **without the value** | your real values (secrets, keys, passwords) never end up in the committed example |
| `.env.example` | `KEY=<value>` to `.env`, with the example's value | the example's value is a sensible starting point |

## Details

- **Which files.** Exactly `.env` and `.env.example` in the same folder. `.env.testing`, `.env.production` and other variants are not compared. If the other file doesn't exist, nothing is reported.
- **How lines are read.** Leading spaces and an `export ` prefix are ignored. Lines starting with `#` are comments. A line counts when it has an `=` and the name before it (trimmed) is made only of letters, digits and `_`. Only names are compared; values are never compared or shown in a message.
- **When it runs.** When either file is opened or saved, and when either changes on disk, including from a terminal or another program: OpenIDEA doesn't report changes to git-ignored files, so the plugin compares the files' size and modification time on its next event. Both files' warnings are updated each time, so fixing a key in one file clears it in the other. The `.env` files are not checked while you type.
- **Turning it off** clears the warnings of both files the next time either is checked.
- The [AI agents' MCP server](AI-Agents.md) offers the same comparison as the `check_env_keys` tool, by name only.

## Settings and permissions

| Setting | Label | Default |
| --- | --- | --- |
| `envKeysInspection` | Report keys that are in .env but not in .env.example, and the other way around | on |

See [Settings](Settings.md). No optional permission is needed. The values in `.env` stay on your machine; see [Privacy and security](Privacy-and-Security.md).

## Related

- [Inspections overview](Inspections.md)
- [Environment variables](Environment-Variables.md): completion of keys and values in `env()` and in `.env`
- [`env()` outside config](Inspection-Env-Outside-Config.md)
- [Environment in the hub](Hub-Environment.md)
- [AI agents and MCP](AI-Agents.md)

---
[← Wiki home](Home.md)
