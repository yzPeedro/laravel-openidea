<p align="center">
  <img src="docs/media/banner.webp" alt="Laravel OpenIDEA" width="100%">
</p>

<p align="center">
  <a href="https://github.com/yzPeedro/laravel-openidea/actions/workflows/ci.yml"><img src="https://github.com/yzPeedro/laravel-openidea/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://github.com/yzPeedro/laravel-openidea/releases/latest"><img src="https://img.shields.io/github/v/release/yzPeedro/laravel-openidea?color=ff2d20&label=release" alt="Latest release"></a>
  <img src="https://img.shields.io/badge/OpenIDEA-0.1.40%2B-4f46e5" alt="OpenIDEA 0.1.40 and newer">
  <img src="https://img.shields.io/badge/Laravel-6%2B-ff2d20?logo=laravel&logoColor=white" alt="Laravel 6 and newer">
  <img src="https://img.shields.io/badge/built%20with-Rust%20%2B%20WebAssembly-orange?logo=rust" alt="Rust and WebAssembly">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-green" alt="MIT license"></a>
</p>

<p align="center">
  <b>The complete Laravel toolkit for <a href="https://github.com/l0ng-ai/openidea">OpenIDEA</a>.</b><br>
  Completion and navigation for every Laravel string, Eloquent that follows your chains, inspections with quick fixes,<br>
  and a hub with Artisan, Tinker, every resource of your app, live logs, a live audit and an MCP server for AI agents.
</p>

<p align="center">
  <a href="https://github.com/yzPeedro/laravel-openidea/wiki"><b>📖 Read the wiki</b></a> — every feature, setting and hub section, with screenshots
</p>

https://github.com/user-attachments/assets/a653530b-f3d4-44e3-921e-bef9119cd659

<p align="center"><sub>An 87-second tour of 0.14: Live Audit, SQL to Laravel and back, Copy Query as MySQL, Form Request extraction, inspections with quick fixes, Eloquent chains, casts, the Cache browser, Tinker, and composer.json and phpunit.xml completion. <a href="docs/media/tour.mp4">Download the MP4</a>.</sub></p>

## Highlights

| | |
| --- | --- |
| ⚡ **[Completion everywhere](https://github.com/yzPeedro/laravel-openidea/wiki/Route-Names)** | Routes with verb and URI, views, config, env, translations, validation rules, middleware, gates, disks, stores, channels, Blade components and directives, and more. |
| 🧠 **[Eloquent that follows the chain](https://github.com/yzPeedro/laravel-openidea/wiki/Eloquent-Chains)** | Columns, relations, scopes, casts and enum members, from `$order->` to `->first()->`, with typed closures and factory states. |
| 🧭 **[Navigation and gutter usages](https://github.com/yzPeedro/laravel-openidea/wiki/Gutter-Usages)** | Ctrl+click opens what a string means; a Laravel icon lists every place a class or scope is used. |
| 🩺 **[Inspections with quick fixes](https://github.com/yzPeedro/laravel-openidea/wiki/Inspections)** | Unknown views, rules and columns, N+1 queries, `env()` outside config, `$fillable`, enum values, duplicate keys, mass assignment and raw SQL, missing translations, what breaks on the next Laravel and more. |
| 🧩 **[A hub for your whole app](https://github.com/yzPeedro/laravel-openidea/wiki/Hub)** | 47 sections, from routes and models to queues, schedule and translations, searchable as one, in a tool window or editor tabs. |
| 🐘 **[Tinker](https://github.com/yzPeedro/laravel-openidea/wiki/Hub-Tinker) and [SQL to Laravel](https://github.com/yzPeedro/laravel-openidea/wiki/Hub-SQL-to-Laravel)** | PHP in tabs with Tinker-style output; SQL to Eloquent as you type, and Laravel back to MySQL. |
| 🔍 **[Live Audit](https://github.com/yzPeedro/laravel-openidea/wiki/Hub-Live-Audit)** | Requests, queries with the line that ran them, events, jobs, cache, mail and more, with N+1 and slow query insights. |
| 🤖 **[AI agents](https://github.com/yzPeedro/laravel-openidea/wiki/AI-Agents)** | An MCP server with your app's runtime data behind an access level you choose, next to Laravel Boost. |

## Installation

**Requirements:** [OpenIDEA](https://github.com/l0ng-ai/openidea/releases/latest) 0.1.40 or newer and a Laravel project (Laravel 6 to 13). PHP or Docker Compose is needed for the runtime features: Artisan, Tinker, Cache and Live Audit.

1. Download `laravel-openidea-<version>.zip` from the [latest release](https://github.com/yzPeedro/laravel-openidea/releases/latest), or install **Laravel OpenIDEA** from **Settings › Plugins › Available**.
2. For the zip, open **Settings › Plugins › ⚙ › Install Plugin from Package File…** and pick it.
3. Allow `process:php` or `process:docker` (and `process:php-win` on Windows) in the plugin's permissions.
4. Open a Laravel project and click the Laravel icon at the bottom of the left tool bar.

The [installation page](https://github.com/yzPeedro/laravel-openidea/wiki/Installation) covers each step with screenshots, the [permissions](https://github.com/yzPeedro/laravel-openidea/wiki/Permissions) and the [settings](https://github.com/yzPeedro/laravel-openidea/wiki/Settings).

## Documentation

| | |
| --- | --- |
| **Getting started** | [Installation](https://github.com/yzPeedro/laravel-openidea/wiki/Installation) · [Permissions](https://github.com/yzPeedro/laravel-openidea/wiki/Permissions) · [Settings](https://github.com/yzPeedro/laravel-openidea/wiki/Settings) · [Commands](https://github.com/yzPeedro/laravel-openidea/wiki/Commands) · [Privacy and security](https://github.com/yzPeedro/laravel-openidea/wiki/Privacy-and-Security) · [How it works](https://github.com/yzPeedro/laravel-openidea/wiki/How-It-Works) · [Troubleshooting](https://github.com/yzPeedro/laravel-openidea/wiki/Troubleshooting) |
| **Editor** | [Completion and navigation](https://github.com/yzPeedro/laravel-openidea/wiki#editor-completion-and-navigation) · [Inspections](https://github.com/yzPeedro/laravel-openidea/wiki/Inspections) · [Refactorings and actions](https://github.com/yzPeedro/laravel-openidea/wiki#refactorings-and-actions) |
| **Hub** | [The hub](https://github.com/yzPeedro/laravel-openidea/wiki/Hub) · [All 47 sections](https://github.com/yzPeedro/laravel-openidea/wiki#the-hub) |
| **AI agents** | [MCP server and access levels](https://github.com/yzPeedro/laravel-openidea/wiki/AI-Agents) |

## Contributing

Issues and pull requests are welcome. Start with [CONTRIBUTING.md](CONTRIBUTING.md) and the [development page](https://github.com/yzPeedro/laravel-openidea/wiki/Development), and see the [roadmap](ROADMAP.md) for what's next. Please follow the [code of conduct](CODE_OF_CONDUCT.md), and report security issues as described in [SECURITY.md](SECURITY.md).

## License

[MIT](LICENSE) © Pedro Pessoa. File icons come from the [Material Icon Theme](https://github.com/material-extensions/vscode-material-icon-theme) (MIT, see `icons/LICENSE-material-icon-theme`).

Laravel is a trademark of Taylor Otwell. This project is not affiliated with Laravel, JetBrains or the OpenIDEA authors.
