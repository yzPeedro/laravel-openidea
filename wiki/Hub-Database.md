# Database

Every table your migrations create, with its columns, the migration that creates it and the year of that migration, plus buttons for `migrate`, `migrate:status`, `db:show` and seeding. A click opens the `Schema::create` call.

<p align="center"><img src="images/hub-database.webp" alt="The Database section listing tables such as cache, categories, failed_jobs, jobs and orders with their column counts, fields, migration and year" width="100%"></p>
<p align="center"><sub>Tables and columns read from the migrations</sub></p>

## How to use it

Click **Database** under **Data** in the sidebar, the **Tables** card on Overview, or open `storage/openidea/hub/database.laravel-hub`. Type a column name in **Filter…** (`user_id`) to see every table that has it, or pick a **Year** to see what a period of migrations created.

## What you see

### Columns

| Column | Content |
| --- | --- |
| Table | The table name |
| Columns | How many columns were found |
| Fields | The columns, comma separated |
| Migration | The file name of the migration that creates the table |
| Year | The first four characters of that file name when they are digits (`2026`), else `—`. Laravel's own stub migrations (`0001_01_01_…`) read `0001` |

### Facets

**Year**.

### Actions

| Button | Command | Mode |
| --- | --- | --- |
| **migrate:status** | `migrate:status` | Output |
| **db:show** | `db:show` | Output |
| **Migrate** | `migrate` | Terminal |
| **Rollback** | `migrate:rollback` | Terminal |
| **Fresh + seed** | `migrate:fresh --seed` | Terminal |
| **Seed** | `db:seed` | Terminal |

The terminal ones run where Artisan can ask its questions, such as the confirmation it asks before migrating in production. **Fresh + seed** drops every table before migrating and seeding.

## Details

### Where the tables come from

The plugin reads every PHP file under `database/migrations/` (up to 10,000), in file name order, without running them:

- A **table** is the first argument of `Schema::create('…')`. When two migrations create the same table, the first one wins. Tables are sorted by name.
- **Columns** come from every migration, `Schema::create` and `Schema::table` alike, in file order: each `$table->type('name')` call with one of the Blueprint column types (`string`, `integer`, `foreignId`, `foreignIdFor`, `json`, `enum`, `uuid`, `ulid`, `timestamp`, `decimal`, `boolean`, `text` and the rest of the 62 types the plugin knows), then the columns implied by `id()` (`id`), `timestamps()`, `timestampsTz()` and `nullableTimestamps()` (`created_at`, `updated_at`), `softDeletes()` and `softDeletesTz()` (`deleted_at`) and `rememberToken()` (`remember_token`). `morphs('commentable')` and its nullable, UUID and ULID variants give `commentable_id` and `commentable_type`.
- A column is listed once per table even when several migrations add it. Dropped or renamed columns aren't removed: the list is everything the migrations add.

Within each migration the named columns come first and the implied ones (`id`, timestamps) after them, which is why `id` often appears after the other fields.

### What it doesn't read

The live database isn't queried: tables created outside migrations, by raw SQL or by a package's own migrations under `vendor/` aren't listed, and pending migrations are listed as if they had run. **migrate:status** and **db:show** show the database's own view.

### Opening and freshness

A click opens the migration at the table name in `Schema::create`. The tables and columns are rebuilt after a change under `database/migrations/` or `app/Models/`. They are the same columns the plugin completes in `where('…')` and checks with the unknown column inspection.

With no migration the section reads `Nothing found in this project yet.`

## Settings and permissions

The table needs only `project.read`. The buttons run with the project's runner and need `process:php` or `process:docker`; with a Compose runner they run in the container, against the database it sees. See [Settings](Settings.md) and [Permissions](Permissions.md).

## Related

- [Models](Hub-Models.md)
- [Eloquent columns and relations](Eloquent-Columns-and-Relations.md)
- [Unknown columns](Inspection-Unknown-Columns.md)
- [Factories & Seeders](Hub-Factories.md)
- [AI agents and MCP](AI-Agents.md) (`database_schema` reads the live connection)
- [The hub](Hub.md)

---
[← Wiki home](Home.md)
