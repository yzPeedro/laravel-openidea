use crate::php::{Kind, tokenize};
use crate::resources::Scan;
use openidea_plugin::providers::{CompletionItem, CompletionKind};
use openidea_plugin::text::position;
use openidea_plugin::{Position, Range};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Extends,
    InterfaceExtends,
    Implements,
    Trait,
}

pub struct Known {
    pub fqn: &'static str,
    pub package: Option<&'static str>,
    pub doc: &'static str,
}

const fn k(fqn: &'static str, doc: &'static str) -> Known {
    Known {
        fqn,
        package: None,
        doc,
    }
}

const fn p(fqn: &'static str, package: &'static str, doc: &'static str) -> Known {
    Known {
        fqn,
        package: Some(package),
        doc,
    }
}

pub const BASES: &[Known] = &[
    k("Illuminate\\Database\\Eloquent\\Model", "Eloquent model"),
    k(
        "Illuminate\\Foundation\\Auth\\User",
        "Authenticatable user model",
    ),
    k(
        "Illuminate\\Database\\Eloquent\\Relations\\Pivot",
        "Custom pivot model",
    ),
    k(
        "Illuminate\\Database\\Eloquent\\Relations\\MorphPivot",
        "Custom polymorphic pivot model",
    ),
    k(
        "Illuminate\\Database\\Eloquent\\Factories\\Factory",
        "Model factory",
    ),
    k("Illuminate\\Database\\Seeder", "Database seeder"),
    k("Illuminate\\Database\\Migrations\\Migration", "Migration"),
    k(
        "Illuminate\\Database\\Eloquent\\Builder",
        "Custom Eloquent builder",
    ),
    k(
        "Illuminate\\Database\\Eloquent\\Collection",
        "Custom Eloquent collection",
    ),
    k(
        "Illuminate\\Foundation\\Http\\FormRequest",
        "Validated form request",
    ),
    k(
        "Illuminate\\Http\\Resources\\Json\\JsonResource",
        "API resource",
    ),
    k(
        "Illuminate\\Http\\Resources\\Json\\ResourceCollection",
        "API resource collection",
    ),
    k("Illuminate\\Routing\\Controller", "Base controller"),
    k("Illuminate\\Support\\ServiceProvider", "Service provider"),
    k(
        "Illuminate\\Foundation\\Support\\Providers\\EventServiceProvider",
        "Event service provider",
    ),
    k(
        "Illuminate\\Foundation\\Support\\Providers\\AuthServiceProvider",
        "Auth service provider",
    ),
    k(
        "Illuminate\\Foundation\\Support\\Providers\\RouteServiceProvider",
        "Route service provider",
    ),
    k("Illuminate\\Console\\Command", "Artisan command"),
    k(
        "Illuminate\\Console\\GeneratorCommand",
        "Artisan make: command",
    ),
    k("Illuminate\\Mail\\Mailable", "Mailable"),
    k("Illuminate\\Notifications\\Notification", "Notification"),
    k("Illuminate\\View\\Component", "Blade class component"),
    k("Illuminate\\Support\\Facades\\Facade", "Facade"),
    k(
        "Illuminate\\Foundation\\Exceptions\\Handler",
        "Exception handler",
    ),
    k(
        "Illuminate\\Foundation\\Testing\\TestCase",
        "Laravel test case",
    ),
    k("Illuminate\\Support\\Collection", "Collection"),
    k("Illuminate\\Support\\Fluent", "Fluent attribute bag"),
    k("Illuminate\\Support\\Manager", "Driver manager"),
    k(
        "Illuminate\\Auth\\Access\\AuthorizationException",
        "Authorization exception",
    ),
    k(
        "Illuminate\\Validation\\ValidationException",
        "Validation exception",
    ),
    k(
        "Symfony\\Component\\HttpKernel\\Exception\\HttpException",
        "HTTP exception",
    ),
    k("Exception", "PHP exception"),
    k("RuntimeException", "PHP runtime exception"),
    k("InvalidArgumentException", "PHP invalid argument exception"),
    k("LogicException", "PHP logic exception"),
    k("DomainException", "PHP domain exception"),
    p(
        "Livewire\\Component",
        "livewire/livewire",
        "Livewire component",
    ),
    p(
        "Livewire\\Form",
        "livewire/livewire",
        "Livewire form object",
    ),
    p(
        "Spatie\\LaravelData\\Data",
        "spatie/laravel-data",
        "Data object",
    ),
    p("Laravel\\Nova\\Resource", "laravel/nova", "Nova resource"),
    p(
        "Filament\\Resources\\Resource",
        "filament/filament",
        "Filament resource",
    ),
    p(
        "Filament\\Pages\\Page",
        "filament/filament",
        "Filament page",
    ),
    p(
        "Filament\\Widgets\\Widget",
        "filament/filament",
        "Filament widget",
    ),
    p(
        "Spatie\\Permission\\Models\\Role",
        "spatie/laravel-permission",
        "Permission role model",
    ),
    p(
        "Spatie\\Permission\\Models\\Permission",
        "spatie/laravel-permission",
        "Permission model",
    ),
    p(
        "Laravel\\Sanctum\\PersonalAccessToken",
        "laravel/sanctum",
        "Sanctum token model",
    ),
    p(
        "Tests\\TestCase",
        "laravel/framework",
        "Application test case",
    ),
];

pub const INTERFACES: &[Known] = &[
    k(
        "Illuminate\\Contracts\\Queue\\ShouldQueue",
        "Run on the queue",
    ),
    k(
        "Illuminate\\Contracts\\Queue\\ShouldQueueAfterCommit",
        "Queue after the database transaction commits",
    ),
    k(
        "Illuminate\\Contracts\\Queue\\ShouldBeUnique",
        "Only one instance on the queue",
    ),
    k(
        "Illuminate\\Contracts\\Queue\\ShouldBeUniqueUntilProcessing",
        "Unique until it starts processing",
    ),
    k(
        "Illuminate\\Contracts\\Queue\\ShouldBeEncrypted",
        "Encrypt the queued payload",
    ),
    k(
        "Illuminate\\Contracts\\Broadcasting\\ShouldBroadcast",
        "Broadcast the event",
    ),
    k(
        "Illuminate\\Contracts\\Broadcasting\\ShouldBroadcastNow",
        "Broadcast synchronously",
    ),
    k(
        "Illuminate\\Contracts\\Broadcasting\\HasBroadcastChannel",
        "Model with a broadcast channel",
    ),
    k(
        "Illuminate\\Contracts\\Events\\ShouldDispatchAfterCommit",
        "Dispatch after commit",
    ),
    k(
        "Illuminate\\Contracts\\Events\\ShouldHandleEventsAfterCommit",
        "Observer runs after commit",
    ),
    k(
        "Illuminate\\Contracts\\Auth\\MustVerifyEmail",
        "Email verification",
    ),
    k(
        "Illuminate\\Contracts\\Auth\\Authenticatable",
        "Authenticatable",
    ),
    k(
        "Illuminate\\Contracts\\Auth\\CanResetPassword",
        "Password reset",
    ),
    k(
        "Illuminate\\Contracts\\Auth\\Access\\Authorizable",
        "Authorizable",
    ),
    k(
        "Illuminate\\Contracts\\Validation\\ValidationRule",
        "Validation rule",
    ),
    k(
        "Illuminate\\Contracts\\Validation\\DataAwareRule",
        "Rule that sees all the data",
    ),
    k(
        "Illuminate\\Contracts\\Validation\\ValidatorAwareRule",
        "Rule that sees the validator",
    ),
    k(
        "Illuminate\\Contracts\\Database\\Eloquent\\CastsAttributes",
        "Custom cast",
    ),
    k(
        "Illuminate\\Contracts\\Database\\Eloquent\\CastsInboundAttributes",
        "Inbound-only cast",
    ),
    k(
        "Illuminate\\Contracts\\Database\\Eloquent\\Castable",
        "Value object with its own cast",
    ),
    k(
        "Illuminate\\Contracts\\Database\\Eloquent\\SerializesCastableAttributes",
        "Cast with custom serialization",
    ),
    k("Illuminate\\Database\\Eloquent\\Scope", "Global scope"),
    k(
        "Illuminate\\Contracts\\Support\\Arrayable",
        "Can be converted to an array",
    ),
    k(
        "Illuminate\\Contracts\\Support\\Jsonable",
        "Can be converted to JSON",
    ),
    k(
        "Illuminate\\Contracts\\Support\\Htmlable",
        "Renders raw HTML in Blade",
    ),
    k(
        "Illuminate\\Contracts\\Support\\Responsable",
        "Can be returned from a route",
    ),
    k(
        "Illuminate\\Contracts\\Support\\Renderable",
        "Can be rendered",
    ),
    k(
        "Illuminate\\Contracts\\Support\\DeferrableProvider",
        "Deferred service provider",
    ),
    k(
        "Illuminate\\Contracts\\Translation\\HasLocalePreference",
        "Preferred locale for mail and notifications",
    ),
    k(
        "Illuminate\\Contracts\\Routing\\UrlRoutable",
        "Route model binding",
    ),
    k(
        "Illuminate\\Contracts\\Mail\\Attachable",
        "Mail attachment object",
    ),
    k(
        "Illuminate\\Contracts\\Console\\Isolatable",
        "Command runs one at a time",
    ),
    k(
        "Illuminate\\Contracts\\Console\\PromptsForMissingInput",
        "Prompt for missing arguments",
    ),
    k("JsonSerializable", "PHP JSON serialization"),
    k("Stringable", "PHP string conversion"),
    k("Countable", "PHP count()"),
    k("ArrayAccess", "PHP array access"),
    k("IteratorAggregate", "PHP iteration"),
    p(
        "Spatie\\MediaLibrary\\HasMedia",
        "spatie/laravel-medialibrary",
        "Model with media",
    ),
    p(
        "Filament\\Models\\Contracts\\FilamentUser",
        "filament/filament",
        "Filament panel access",
    ),
    p(
        "Filament\\Models\\Contracts\\HasAvatar",
        "filament/filament",
        "Filament avatar",
    ),
    p(
        "Laravel\\Sanctum\\Contracts\\HasApiTokens",
        "laravel/sanctum",
        "Sanctum tokens",
    ),
    p(
        "Spatie\\Sitemap\\Contracts\\Sitemapable",
        "spatie/laravel-sitemap",
        "Sitemap entry",
    ),
    p("Spatie\\Feed\\Feedable", "spatie/laravel-feed", "Feed item"),
];

pub const TRAITS: &[Known] = &[
    k(
        "Illuminate\\Database\\Eloquent\\Factories\\HasFactory",
        "Model factories",
    ),
    k(
        "Illuminate\\Database\\Eloquent\\SoftDeletes",
        "Soft deletes",
    ),
    k(
        "Illuminate\\Database\\Eloquent\\Concerns\\HasUuids",
        "UUID primary keys",
    ),
    k(
        "Illuminate\\Database\\Eloquent\\Concerns\\HasUlids",
        "ULID primary keys",
    ),
    k(
        "Illuminate\\Database\\Eloquent\\Prunable",
        "Prune models one by one",
    ),
    k(
        "Illuminate\\Database\\Eloquent\\MassPrunable",
        "Prune models in bulk",
    ),
    k(
        "Illuminate\\Database\\Eloquent\\BroadcastsEvents",
        "Broadcast model events",
    ),
    k(
        "Illuminate\\Notifications\\Notifiable",
        "Receive notifications",
    ),
    k(
        "Illuminate\\Foundation\\Bus\\Dispatchable",
        "Job ::dispatch()",
    ),
    k(
        "Illuminate\\Foundation\\Queue\\Queueable",
        "Queueable job (Dispatchable, InteractsWithQueue, Queueable, SerializesModels)",
    ),
    k("Illuminate\\Bus\\Queueable", "Queue, connection and delay"),
    k(
        "Illuminate\\Queue\\InteractsWithQueue",
        "release(), delete(), attempts()",
    ),
    k(
        "Illuminate\\Queue\\SerializesModels",
        "Serialize models by key",
    ),
    k("Illuminate\\Bus\\Batchable", "Job batches"),
    k(
        "Illuminate\\Foundation\\Events\\Dispatchable",
        "Event ::dispatch()",
    ),
    k(
        "Illuminate\\Broadcasting\\InteractsWithSockets",
        "dontBroadcastToCurrentUser()",
    ),
    k(
        "Illuminate\\Broadcasting\\InteractsWithBroadcasting",
        "broadcastVia()",
    ),
    k(
        "Illuminate\\Foundation\\Auth\\Access\\AuthorizesRequests",
        "$this->authorize()",
    ),
    k(
        "Illuminate\\Foundation\\Validation\\ValidatesRequests",
        "$this->validate()",
    ),
    k(
        "Illuminate\\Auth\\Access\\HandlesAuthorization",
        "allow() and deny() in policies",
    ),
    k(
        "Illuminate\\Auth\\Authenticatable",
        "Authenticatable implementation",
    ),
    k(
        "Illuminate\\Auth\\Passwords\\CanResetPassword",
        "Password reset implementation",
    ),
    k(
        "Illuminate\\Auth\\MustVerifyEmail",
        "Email verification implementation",
    ),
    k(
        "Illuminate\\Foundation\\Auth\\Access\\Authorizable",
        "can() and cannot()",
    ),
    k("Illuminate\\Support\\Traits\\Macroable", "::macro()"),
    k(
        "Illuminate\\Support\\Traits\\Conditionable",
        "when() and unless()",
    ),
    k("Illuminate\\Support\\Traits\\Tappable", "tap()"),
    k(
        "Illuminate\\Support\\Traits\\ForwardsCalls",
        "Forward calls to another object",
    ),
    k("Illuminate\\Support\\Traits\\Localizable", "withLocale()"),
    k(
        "Illuminate\\Console\\ConfirmableTrait",
        "confirmToProceed() in production",
    ),
    k(
        "Illuminate\\Console\\Prohibitable",
        "Prohibit destructive commands",
    ),
    k(
        "Illuminate\\Foundation\\Testing\\RefreshDatabase",
        "Reset the database between tests",
    ),
    k(
        "Illuminate\\Foundation\\Testing\\LazilyRefreshDatabase",
        "Reset the database when a test touches it",
    ),
    k(
        "Illuminate\\Foundation\\Testing\\DatabaseTransactions",
        "Wrap each test in a transaction",
    ),
    k(
        "Illuminate\\Foundation\\Testing\\DatabaseMigrations",
        "Migrate before each test",
    ),
    k(
        "Illuminate\\Foundation\\Testing\\DatabaseTruncation",
        "Truncate tables between tests",
    ),
    k("Illuminate\\Foundation\\Testing\\WithFaker", "$this->faker"),
    k(
        "Illuminate\\Foundation\\Testing\\WithoutMiddleware",
        "Disable middleware in tests",
    ),
    p(
        "Laravel\\Sanctum\\HasApiTokens",
        "laravel/sanctum",
        "Sanctum API tokens",
    ),
    p(
        "Laravel\\Passport\\HasApiTokens",
        "laravel/passport",
        "Passport API tokens",
    ),
    p(
        "Laravel\\Fortify\\TwoFactorAuthenticatable",
        "laravel/fortify",
        "Two-factor authentication",
    ),
    p(
        "Laravel\\Jetstream\\HasProfilePhoto",
        "laravel/jetstream",
        "Profile photos",
    ),
    p("Laravel\\Jetstream\\HasTeams", "laravel/jetstream", "Teams"),
    p(
        "Laravel\\Scout\\Searchable",
        "laravel/scout",
        "Full-text search",
    ),
    p(
        "Laravel\\Cashier\\Billable",
        "laravel/cashier",
        "Stripe billing",
    ),
    p(
        "Laravel\\Pennant\\Concerns\\HasFeatures",
        "laravel/pennant",
        "Feature flags",
    ),
    p(
        "Spatie\\Permission\\Traits\\HasRoles",
        "spatie/laravel-permission",
        "Roles and permissions",
    ),
    p(
        "Spatie\\MediaLibrary\\InteractsWithMedia",
        "spatie/laravel-medialibrary",
        "Media collections",
    ),
    p(
        "Spatie\\Activitylog\\Traits\\LogsActivity",
        "spatie/laravel-activitylog",
        "Activity log",
    ),
    p(
        "Spatie\\Sluggable\\HasSlug",
        "spatie/laravel-sluggable",
        "Slugs",
    ),
    p(
        "Spatie\\Translatable\\HasTranslations",
        "spatie/laravel-translatable",
        "Translatable attributes",
    ),
    p("Spatie\\Tags\\HasTags", "spatie/laravel-tags", "Tags"),
    p(
        "Livewire\\WithPagination",
        "livewire/livewire",
        "Livewire pagination",
    ),
    p(
        "Livewire\\WithFileUploads",
        "livewire/livewire",
        "Livewire uploads",
    ),
    p(
        "Filament\\Forms\\Concerns\\InteractsWithForms",
        "filament/filament",
        "Filament forms",
    ),
    p(
        "OwenIt\\Auditing\\Auditable",
        "owen-it/laravel-auditing",
        "Auditing",
    ),
];

const PREFERRED: &[(&str, Slot, &[&str])] = &[
    ("/Models/", Slot::Extends, &["Model", "User", "Pivot"]),
    (
        "/Models/",
        Slot::Trait,
        &[
            "HasFactory",
            "SoftDeletes",
            "Notifiable",
            "HasUuids",
            "HasUlids",
            "HasApiTokens",
            "HasRoles",
        ],
    ),
    (
        "/Models/",
        Slot::Implements,
        &[
            "MustVerifyEmail",
            "HasLocalePreference",
            "HasMedia",
            "FilamentUser",
        ],
    ),
    ("/Http/Controllers/", Slot::Extends, &["Controller"]),
    (
        "/Http/Controllers/",
        Slot::Trait,
        &["AuthorizesRequests", "ValidatesRequests"],
    ),
    ("/Http/Requests/", Slot::Extends, &["FormRequest"]),
    (
        "/Http/Resources/",
        Slot::Extends,
        &["JsonResource", "ResourceCollection"],
    ),
    (
        "/Jobs/",
        Slot::Implements,
        &["ShouldQueue", "ShouldBeUnique", "ShouldBeEncrypted"],
    ),
    (
        "/Jobs/",
        Slot::Trait,
        &[
            "Queueable",
            "Dispatchable",
            "InteractsWithQueue",
            "SerializesModels",
            "Batchable",
        ],
    ),
    (
        "/Listeners/",
        Slot::Implements,
        &["ShouldQueue", "ShouldQueueAfterCommit"],
    ),
    (
        "/Listeners/",
        Slot::Trait,
        &["InteractsWithQueue", "Queueable"],
    ),
    (
        "/Events/",
        Slot::Implements,
        &[
            "ShouldBroadcast",
            "ShouldBroadcastNow",
            "ShouldDispatchAfterCommit",
        ],
    ),
    (
        "/Events/",
        Slot::Trait,
        &["Dispatchable", "InteractsWithSockets", "SerializesModels"],
    ),
    ("/Mail/", Slot::Extends, &["Mailable"]),
    ("/Mail/", Slot::Implements, &["ShouldQueue"]),
    ("/Mail/", Slot::Trait, &["Queueable", "SerializesModels"]),
    ("/Notifications/", Slot::Extends, &["Notification"]),
    ("/Notifications/", Slot::Implements, &["ShouldQueue"]),
    ("/Notifications/", Slot::Trait, &["Queueable"]),
    (
        "/Rules/",
        Slot::Implements,
        &["ValidationRule", "DataAwareRule", "ValidatorAwareRule"],
    ),
    (
        "/Casts/",
        Slot::Implements,
        &["CastsAttributes", "CastsInboundAttributes"],
    ),
    ("/Scopes/", Slot::Implements, &["Scope"]),
    (
        "/Observers/",
        Slot::Implements,
        &["ShouldHandleEventsAfterCommit"],
    ),
    ("/Policies/", Slot::Trait, &["HandlesAuthorization"]),
    ("/Providers/", Slot::Extends, &["ServiceProvider"]),
    ("/Providers/", Slot::Implements, &["DeferrableProvider"]),
    ("/Console/", Slot::Extends, &["Command"]),
    (
        "/Console/",
        Slot::Implements,
        &["Isolatable", "PromptsForMissingInput"],
    ),
    ("/View/Components/", Slot::Extends, &["Component"]),
    ("/Livewire/", Slot::Extends, &["Component", "Form"]),
    (
        "/Exceptions/",
        Slot::Extends,
        &["Exception", "RuntimeException", "HttpException"],
    ),
    ("database/factories/", Slot::Extends, &["Factory"]),
    ("database/seeders/", Slot::Extends, &["Seeder"]),
    ("tests/", Slot::Extends, &["TestCase"]),
    (
        "tests/",
        Slot::Trait,
        &[
            "RefreshDatabase",
            "LazilyRefreshDatabase",
            "WithFaker",
            "DatabaseTransactions",
        ],
    ),
];

fn word_start(text: &str, cursor: usize) -> usize {
    let b = text.as_bytes();
    let mut i = cursor;
    while i > 0 && (b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_' || b[i - 1] == b'\\') {
        i -= 1;
    }
    i
}

fn depth(text: &str, end: usize) -> i32 {
    let mut depth = 0;
    for t in tokenize(text, 0, end) {
        match t.kind {
            Kind::Open('{') => depth += 1,
            Kind::Close('}') => depth -= 1,
            _ => {}
        }
    }
    depth
}

pub fn slot_at(text: &str, cursor: usize) -> Option<(Slot, usize)> {
    if cursor > text.len() || !text.is_char_boundary(cursor) {
        return None;
    }
    let start = word_start(text, cursor);
    let window = text[..start].rfind(['{', '}', ';']).map_or(0, |i| i + 1);
    let tokens = tokenize(text, window, start);
    let last = tokens.last()?;
    let word = |t: &crate::php::Token| match &t.kind {
        Kind::Ident(s) => Some(s.to_ascii_lowercase()),
        _ => None,
    };
    let declares_interface = || tokens.iter().filter_map(word).any(|w| w == "interface");
    match word(last).as_deref() {
        Some("extends") if tokens.len() >= 3 => {
            let slot = if declares_interface() {
                Slot::InterfaceExtends
            } else {
                Slot::Extends
            };
            return Some((slot, start));
        }
        Some("implements") if tokens.len() >= 3 => return Some((Slot::Implements, start)),
        Some("use")
            if tokens.len() == 1
                && text[..start].ends_with(char::is_whitespace)
                && depth(text, start) >= 1 =>
        {
            return Some((Slot::Trait, start));
        }
        _ => {}
    }
    if last.kind != Kind::Comma {
        return None;
    }
    let mut i = tokens.len();
    let mut keyword = None;
    while i > 0 {
        i -= 1;
        match &tokens[i].kind {
            Kind::Comma | Kind::Ident(_) => {
                if let Some(w) = word(&tokens[i])
                    && matches!(w.as_str(), "implements" | "extends" | "use")
                {
                    keyword = Some((w, i));
                    break;
                }
            }
            _ => return None,
        }
    }
    match keyword {
        Some((w, _)) if w == "implements" => Some((Slot::Implements, start)),
        Some((w, _)) if w == "extends" && declares_interface() => {
            Some((Slot::InterfaceExtends, start))
        }
        Some((w, 0)) if w == "use" && depth(text, start) >= 1 => Some((Slot::Trait, start)),
        _ => None,
    }
}

fn short(fqn: &str) -> &str {
    fqn.rsplit('\\').next().unwrap_or(fqn)
}

fn imported(text: &str, fqn: &str) -> bool {
    let short_name = short(fqn);
    text.lines().any(|l| {
        let l = l.trim();
        l.starts_with("use ")
            && (l
                .trim_end_matches(';')
                .ends_with(&format!("\\{short_name}"))
                || l.trim_end_matches(';')
                    .trim_start_matches("use ")
                    .trim_start_matches('\\')
                    == fqn
                || l.contains(&format!(" as {short_name};")))
    })
}

pub struct ProjectSymbol {
    pub fqn: String,
    pub keyword: String,
    pub is_abstract: bool,
}

pub fn project_symbols(scan: &Scan) -> Vec<ProjectSymbol> {
    scan.symbols
        .iter()
        .filter(|s| !s.namespace.is_empty())
        .map(|s| ProjectSymbol {
            fqn: format!("{}\\{}", s.namespace, s.name),
            keyword: s.keyword.clone(),
            is_abstract: s.is_abstract,
        })
        .collect()
}

pub fn items(
    slot: Slot,
    text: &str,
    path: &str,
    start: usize,
    at: Position,
    composer: &str,
    project: &[ProjectSymbol],
) -> Vec<CompletionItem> {
    let cursor = openidea_plugin::text::offset(text, at)
        .unwrap_or(start)
        .clamp(start, text.len());
    let typed = &text[start..cursor];
    let typed_short = typed
        .trim_start_matches('\\')
        .rsplit('\\')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let list = match slot {
        Slot::Extends => BASES,
        Slot::Implements | Slot::InterfaceExtends => INTERFACES,
        Slot::Trait => TRAITS,
    };
    let preferred: Vec<&str> = PREFERRED
        .iter()
        .filter(|(dir, s, _)| *s == slot && path.contains(dir))
        .flat_map(|(_, _, names)| names.iter().copied())
        .collect();
    let replace = Range::new(position(text, start), at);
    let mut out = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    let mut push = |fqn: &str, doc: &str, origin: &str, kind: CompletionKind| {
        let name = short(fqn);
        if !typed_short.is_empty() && !name.to_ascii_lowercase().contains(&typed_short) {
            return;
        }
        if !seen.insert(fqn.to_owned()) {
            return;
        }
        let rank = preferred.iter().position(|p| *p == name).unwrap_or(99);
        let insert = if imported(text, fqn) {
            name.to_owned()
        } else {
            format!("\\{fqn}")
        };
        let mut item = CompletionItem::new(name, kind);
        item.insert_text = insert;
        item.replace = Some(replace);
        item.detail = Some(format!("{fqn} · {origin}"));
        item.documentation = Some(doc.to_owned());
        item.sort_text = Some(format!(
            "{rank:02}{}{name}",
            if origin == "Laravel" { 0 } else { 1 }
        ));
        out.push(item);
    };
    for known in list {
        if known
            .package
            .is_some_and(|p| p != "laravel/framework" && !composer.contains(&format!("\"{p}\"")))
        {
            continue;
        }
        if known.fqn == "Tests\\TestCase" && !path.contains("tests/") {
            continue;
        }
        let kind = match slot {
            Slot::Extends => CompletionKind::Class,
            Slot::Trait => CompletionKind::Module,
            _ => CompletionKind::Interface,
        };
        push(
            known.fqn,
            known.doc,
            if known.package.is_some() {
                "package"
            } else {
                "Laravel"
            },
            kind,
        );
    }
    for s in project {
        let fits = match slot {
            Slot::Extends => s.keyword == "class",
            Slot::Implements | Slot::InterfaceExtends => s.keyword == "interface",
            Slot::Trait => s.keyword == "trait",
        };
        if !fits {
            continue;
        }
        let doc = if s.is_abstract {
            "Abstract class of this project"
        } else {
            "Defined in this project"
        };
        let kind = match slot {
            Slot::Extends => CompletionKind::Class,
            Slot::Trait => CompletionKind::Module,
            _ => CompletionKind::Interface,
        };
        push(&s.fqn, doc, "project", kind);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(src: &str) -> Option<Slot> {
        let cursor = src.find('|').unwrap();
        let text = src.replace('|', "");
        slot_at(&text, cursor).map(|(s, _)| s)
    }

    #[test]
    fn finds_the_slot() {
        assert_eq!(slot("<?php\nclass User extends Mo|"), Some(Slot::Extends));
        assert_eq!(slot("<?php\nclass User extends |"), Some(Slot::Extends));
        assert_eq!(
            slot("<?php\nclass User extends Model implements Must|"),
            Some(Slot::Implements)
        );
        assert_eq!(
            slot("<?php\nclass Job implements ShouldQueue, Should|"),
            Some(Slot::Implements)
        );
        assert_eq!(
            slot("<?php\ninterface Repo extends Coun|"),
            Some(Slot::InterfaceExtends)
        );
        assert_eq!(
            slot("<?php\nclass User extends Model\n{\n    use Has|"),
            Some(Slot::Trait)
        );
        assert_eq!(
            slot("<?php\nclass User extends Model\n{\n    use HasFactory, Soft|"),
            Some(Slot::Trait)
        );
        assert_eq!(
            slot("<?php\nclass User extends Model\n{\n    use HasFactory;\n    use |"),
            Some(Slot::Trait)
        );
        assert_eq!(slot("<?php\nnamespace App;\nuse Illu|"), None);
        assert_eq!(
            slot("<?php\nclass A {\n public function x() { $f = function () use (|"),
            None
        );
        assert_eq!(
            slot("<?php\nclass A {\n public function x() { foo(1, |"),
            None
        );
    }

    #[test]
    fn suggests_laravel_and_project_types() {
        let text = "<?php\nnamespace App\\Models;\nuse Illuminate\\Database\\Eloquent\\SoftDeletes;\nclass User extends Model\n{\n    use ";
        let at = position(text, text.len());
        let project = vec![ProjectSymbol {
            fqn: "App\\Concerns\\HasAvatar".into(),
            keyword: "trait".into(),
            is_abstract: false,
        }];
        let items = items(
            Slot::Trait,
            text,
            "app/Models/User.php",
            text.len(),
            at,
            "\"laravel/sanctum\"",
            &project,
        );
        let first = items.iter().min_by_key(|i| i.sort_text.clone()).unwrap();
        assert_eq!(first.label, "HasFactory");
        assert_eq!(
            first.insert_text,
            "\\Illuminate\\Database\\Eloquent\\Factories\\HasFactory"
        );
        let soft = items.iter().find(|i| i.label == "SoftDeletes").unwrap();
        assert_eq!(soft.insert_text, "SoftDeletes");
        assert!(
            items
                .iter()
                .any(|i| i.label == "HasApiTokens"
                    && i.insert_text.starts_with("\\Laravel\\Sanctum"))
        );
        assert!(!items.iter().any(|i| i.insert_text.contains("Passport")));
        assert!(items.iter().any(|i| i.label == "HasAvatar"));
    }
}
