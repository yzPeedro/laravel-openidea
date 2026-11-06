mod assets;
mod auth;
mod blade;
mod classes;
mod config;
mod database;
mod env;
pub mod folio;
mod lang;
mod middleware;
mod models;
pub mod openapi;
mod packages;
pub mod routes;
pub mod rules;
mod views;

use crate::project::rel;
use openidea_plugin::providers::CompletionKind;
use openidea_plugin::{Location, LogLevel, Project, Range, host};
use std::collections::BTreeMap;
use std::rc::Rc;
use std::time::Instant;

pub use classes::{Class, parse_class};
pub use models::{is_model, pivots, plural, table_for};
pub use routes::parse as parse_routes;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RefKind {
    Config,
    Route,
    RouteAction,
    View,
    Env,
    Translation,
    Disk,
    CacheStore,
    Queue,
    LogChannel,
    DbConnection,
    Mailer,
    Guard,
    PasswordBroker,
    Broadcaster,
    Middleware,
    Ability,
    RateLimiter,
    Table,
    Column,
    Model,
    Relation,
    Scope,
    Controller,
    Component,
    Livewire,
    InertiaPage,
    Section,
    Stack,
    Asset,
    Rule,
    OpenApi,
    TokenAbility,
    Channel,
    Command,
    QueueName,
    Role,
    Permission,
    SocialiteDriver,
    Directive,
}

impl RefKind {
    pub const ALL: [RefKind; 40] = [
        Self::Config,
        Self::Route,
        Self::RouteAction,
        Self::View,
        Self::Env,
        Self::Translation,
        Self::Disk,
        Self::CacheStore,
        Self::Queue,
        Self::LogChannel,
        Self::DbConnection,
        Self::Mailer,
        Self::Guard,
        Self::PasswordBroker,
        Self::Broadcaster,
        Self::Middleware,
        Self::Ability,
        Self::RateLimiter,
        Self::Table,
        Self::Column,
        Self::Model,
        Self::Relation,
        Self::Scope,
        Self::Controller,
        Self::Component,
        Self::Livewire,
        Self::InertiaPage,
        Self::Section,
        Self::Stack,
        Self::Asset,
        Self::Rule,
        Self::OpenApi,
        Self::TokenAbility,
        Self::Channel,
        Self::Command,
        Self::QueueName,
        Self::Role,
        Self::Permission,
        Self::SocialiteDriver,
        Self::Directive,
    ];

    pub fn completion_kind(self) -> CompletionKind {
        match self {
            Self::Config => CompletionKind::Property,
            Self::Route | Self::RouteAction => CompletionKind::Reference,
            Self::View | Self::InertiaPage | Self::Asset => CompletionKind::File,
            Self::Env => CompletionKind::Constant,
            Self::Translation => CompletionKind::Text,
            Self::Disk
            | Self::CacheStore
            | Self::Queue
            | Self::LogChannel
            | Self::DbConnection
            | Self::Mailer
            | Self::Guard
            | Self::PasswordBroker
            | Self::Broadcaster => CompletionKind::Value,
            Self::Middleware | Self::RateLimiter => CompletionKind::Interface,
            Self::Ability => CompletionKind::Event,
            Self::Table => CompletionKind::Struct,
            Self::Column => CompletionKind::Field,
            Self::Model | Self::Component | Self::Livewire => CompletionKind::Class,
            Self::Relation | Self::Scope | Self::Controller => CompletionKind::Reference,
            Self::Section | Self::Stack => CompletionKind::Module,
            Self::Rule => CompletionKind::Keyword,
            Self::OpenApi => CompletionKind::Reference,
            Self::TokenAbility | Self::Role | Self::Permission => CompletionKind::Event,
            Self::Channel | Self::QueueName | Self::SocialiteDriver => CompletionKind::Value,
            Self::Command => CompletionKind::Value,
            Self::Directive => CompletionKind::Keyword,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Config => "config keys",
            Self::Route => "routes",
            Self::RouteAction => "route actions",
            Self::View => "views",
            Self::Env => "env variables",
            Self::Translation => "translations",
            Self::Disk => "disks",
            Self::CacheStore => "cache stores",
            Self::Queue => "queue connections",
            Self::LogChannel => "log channels",
            Self::DbConnection => "database connections",
            Self::Mailer => "mailers",
            Self::Guard => "guards",
            Self::PasswordBroker => "password brokers",
            Self::Broadcaster => "broadcasters",
            Self::Middleware => "middleware",
            Self::Ability => "abilities",
            Self::RateLimiter => "rate limiters",
            Self::Table => "tables",
            Self::Column => "columns",
            Self::Model => "models",
            Self::Relation => "relations",
            Self::Scope => "scopes",
            Self::Controller => "controller methods",
            Self::Component => "Blade components",
            Self::Livewire => "Livewire components",
            Self::InertiaPage => "Inertia pages",
            Self::Section => "sections",
            Self::Stack => "stacks",
            Self::Asset => "assets",
            Self::Rule => "validation rules",
            Self::OpenApi => "OpenAPI components",
            Self::TokenAbility => "token abilities",
            Self::Channel => "broadcast channels",
            Self::Command => "Artisan commands",
            Self::QueueName => "queues",
            Self::Role => "roles",
            Self::Permission => "permissions",
            Self::SocialiteDriver => "Socialite drivers",
            Self::Directive => "Blade directives",
        }
    }

    fn config_prefix(self) -> Option<&'static str> {
        Some(match self {
            Self::Disk => "filesystems.disks",
            Self::CacheStore => "cache.stores",
            Self::Queue => "queue.connections",
            Self::LogChannel => "logging.channels",
            Self::DbConnection => "database.connections",
            Self::Mailer => "mail.mailers",
            Self::Guard => "auth.guards",
            Self::PasswordBroker => "auth.passwords",
            Self::Broadcaster => "broadcasting.connections",
            _ => return None,
        })
    }

    pub fn affected_by(rel_path: &str) -> Vec<RefKind> {
        let p = rel_path.replace('\\', "/");
        let has = |prefixes: &[&str]| prefixes.iter().any(|x| p.starts_with(x));
        Self::ALL
            .into_iter()
            .filter(|kind| match kind {
                Self::Config => has(&["config/"]),
                k if k.config_prefix().is_some() => has(&["config/"]),
                Self::Route | Self::RouteAction => has(&[
                    "routes/",
                    "bootstrap/app.php",
                    "bootstrap/cache/",
                    "app/Providers/",
                    "resources/views/",
                ]),
                Self::View | Self::Section | Self::Stack => has(&["resources/views/"]),
                Self::Env => has(&[".env"]),
                Self::Translation => has(&["lang/", "resources/lang/"]),
                Self::Middleware => has(&["bootstrap/app.php", "app/Http/Kernel.php"]),
                Self::Ability | Self::RateLimiter => has(&["app/"]),
                Self::Table | Self::Column => has(&["database/migrations/", "app/Models/"]),
                Self::Model | Self::Relation | Self::Scope => has(&["app/Models/", "app/"]),
                Self::Controller => has(&["app/"]),
                Self::Component => has(&["resources/views/components/", "app/View/Components/"]),
                Self::Livewire => has(&[
                    "app/Livewire/",
                    "app/Http/Livewire/",
                    "resources/views/livewire/",
                ]),
                Self::InertiaPage => has(&["resources/js/"]),
                Self::Asset => has(&["resources/", "public/", "vite.config"]),
                Self::OpenApi | Self::TokenAbility | Self::QueueName => {
                    has(&["app/", "docs/", "routes/", "config/"])
                }
                Self::Channel => has(&["routes/"]),
                Self::Command => has(&["app/", "routes/console.php"]),
                Self::Role | Self::Permission => has(&["database/", "app/"]),
                Self::SocialiteDriver => has(&["config/"]),
                Self::Directive => has(&["app/"]),
                _ => false,
            })
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub key: String,
    pub detail: Option<String>,
    pub location: Location,
}

impl Entry {
    pub fn new(key: impl Into<String>, detail: Option<String>, path: &str, range: Range) -> Self {
        Self {
            key: key.into(),
            detail,
            location: Location {
                path: path.to_owned(),
                range,
            },
        }
    }
}

#[derive(Default)]
pub struct Cache {
    built: BTreeMap<(String, RefKind), Rc<Vec<Entry>>>,
    pub locale: String,
}

impl Cache {
    pub fn get(&mut self, kind: RefKind, project: &Project, root: &str) -> Rc<Vec<Entry>> {
        let key = (root.to_owned(), kind);
        if let Some(entries) = self.built.get(&key) {
            return entries.clone();
        }
        let started = Instant::now();
        let entries = Rc::new(match kind.config_prefix() {
            Some(prefix) => {
                let mut entries = derived(&self.get(RefKind::Config, project, root), prefix);
                if kind == RefKind::Guard
                    && project
                        .read_file("composer.json")
                        .is_ok_and(|c| c.contains("laravel/sanctum"))
                    && !entries.iter().any(|e| e.key == "sanctum")
                {
                    entries.push(Entry::new(
                        "sanctum",
                        Some("Laravel Sanctum".into()),
                        "",
                        Range::default(),
                    ));
                }
                entries
            }
            None if kind == RefKind::QueueName => {
                let config = self.get(RefKind::Config, project, root);
                packages::queues(project, root, &config)
            }
            None if kind == RefKind::SocialiteDriver => {
                packages::socialite(&self.get(RefKind::Config, project, root))
            }
            None => build(kind, project, root, &self.locale),
        });
        host::log(
            LogLevel::Info,
            &format!(
                "indexed {} {} in {} ms",
                entries.len(),
                kind.label(),
                started.elapsed().as_millis()
            ),
        );
        self.built.insert(key, entries.clone());
        entries
    }

    pub fn warm(&mut self, project: &Project, root: &str) {
        for kind in RefKind::ALL {
            self.get(kind, project, root);
        }
    }

    pub fn invalidate(&mut self, root: &str, path: &str) -> bool {
        let kinds = RefKind::affected_by(rel(root, path));
        for kind in &kinds {
            self.built.remove(&(root.to_owned(), *kind));
        }
        !kinds.is_empty()
    }

    pub fn clear(&mut self) {
        self.built.clear();
    }
}

fn derived(config: &[Entry], prefix: &str) -> Vec<Entry> {
    config
        .iter()
        .filter_map(|e| {
            let name = e.key.strip_prefix(prefix)?.strip_prefix('.')?;
            (!name.contains('.')).then(|| Entry {
                key: name.to_owned(),
                detail: Some(e.key.clone()),
                location: e.location.clone(),
            })
        })
        .collect()
}

fn build(kind: RefKind, project: &Project, root: &str, locale: &str) -> Vec<Entry> {
    match kind {
        RefKind::Config => config::index(project, root),
        RefKind::Route => routes::index(project, root),
        RefKind::RouteAction => routes::actions(project, root),
        RefKind::View => views::index(project, root),
        RefKind::Env => env::index(project, root),
        RefKind::Translation => lang::index(project, root, locale),
        RefKind::Middleware => middleware::index(project, root),
        RefKind::Ability => auth::abilities(project, root),
        RefKind::RateLimiter => auth::rate_limiters(project, root),
        RefKind::Table => database::tables(project, root),
        RefKind::Column => database::columns(project, root),
        RefKind::Model => models::models(project, root),
        RefKind::Relation => models::relations(project, root),
        RefKind::Scope => models::scopes(project, root),
        RefKind::Controller => classes::controller_methods(project, root),
        RefKind::Component => blade::components(project, root),
        RefKind::Livewire => blade::livewire(project, root),
        RefKind::InertiaPage => assets::inertia_pages(project, root),
        RefKind::Section => blade::sections(project, root),
        RefKind::Stack => blade::stacks(project, root),
        RefKind::Asset => assets::assets(project, root),
        RefKind::Rule => rules::entries(),
        RefKind::OpenApi => openapi::index(project, root),
        RefKind::TokenAbility => packages::token_abilities(project, root),
        RefKind::Channel => packages::channels(project, root),
        RefKind::Command => packages::commands(project, root),
        RefKind::Role => packages::roles(project, root),
        RefKind::Permission => packages::permissions(project, root),
        RefKind::Directive => blade::custom_directives(project, root),
        _ => Vec::new(),
    }
}
