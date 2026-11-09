use crate::index::RefKind;
use crate::index::openapi::{FORMATS, IN, TYPES, is_openapi};
use crate::php::{CallKind, StringArg};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Want {
    Kind(RefKind),
    Columns {
        model: Option<String>,
        table: Option<String>,
    },
    Relations {
        model: String,
        path: Vec<String>,
    },
    RelationColumns {
        model: String,
        path: Vec<String>,
    },
    Methods(String),
    OpenApiRef,
    OpenApiSection(&'static str),
    Static(&'static [(&'static str, &'static str)]),
    Container,
    CastTypes,
    EnumValues {
        model: String,
        call_start: usize,
        index: usize,
    },
    SchemaProperties(usize),
    RoutePaths,
    PathParams(usize),
    Paths(&'static str),
    DiskPaths(usize),
    RequestFields,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Segment {
    pub want: Want,
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Debug, Default)]
pub struct Scope {
    pub class: Option<String>,
    pub model: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Ctx {
    Simple(RefKind),
    Middleware,
    Rules,
    Casts,
    Columns(Option<String>),
    Relations(String),
    Methods(String),
    OpenApiRef,
    Section(&'static str),
    Static(&'static [(&'static str, &'static str)]),
    Container,
    EnumValues(String),
    SchemaProperties,
    RoutePaths,
    PathParams,
    Paths(&'static str),
    DiskPaths,
    RequestFields,
}

pub const CONTAINER: [(&str, &str); 38] = [
    ("app", "Illuminate\\Foundation\\Application"),
    ("auth", "Illuminate\\Auth\\AuthManager"),
    ("auth.driver", "Illuminate\\Contracts\\Auth\\Guard"),
    (
        "auth.password",
        "Illuminate\\Auth\\Passwords\\PasswordBrokerManager",
    ),
    (
        "auth.password.broker",
        "Illuminate\\Auth\\Passwords\\PasswordBroker",
    ),
    (
        "blade.compiler",
        "Illuminate\\View\\Compilers\\BladeCompiler",
    ),
    ("cache", "Illuminate\\Cache\\CacheManager"),
    ("cache.store", "Illuminate\\Cache\\Repository"),
    (
        "cache.psr6",
        "Symfony\\Component\\Cache\\Adapter\\Psr16Adapter",
    ),
    ("config", "Illuminate\\Config\\Repository"),
    ("cookie", "Illuminate\\Cookie\\CookieJar"),
    ("db", "Illuminate\\Database\\DatabaseManager"),
    ("db.connection", "Illuminate\\Database\\Connection"),
    ("db.schema", "Illuminate\\Database\\Schema\\Builder"),
    ("encrypter", "Illuminate\\Encryption\\Encrypter"),
    ("events", "Illuminate\\Events\\Dispatcher"),
    ("files", "Illuminate\\Filesystem\\Filesystem"),
    ("filesystem", "Illuminate\\Filesystem\\FilesystemManager"),
    (
        "filesystem.disk",
        "Illuminate\\Contracts\\Filesystem\\Filesystem",
    ),
    (
        "filesystem.cloud",
        "Illuminate\\Contracts\\Filesystem\\Cloud",
    ),
    ("hash", "Illuminate\\Hashing\\HashManager"),
    ("hash.driver", "Illuminate\\Contracts\\Hashing\\Hasher"),
    ("translator", "Illuminate\\Translation\\Translator"),
    ("log", "Illuminate\\Log\\LogManager"),
    ("mail.manager", "Illuminate\\Mail\\MailManager"),
    ("mailer", "Illuminate\\Mail\\Mailer"),
    ("queue", "Illuminate\\Queue\\QueueManager"),
    ("queue.connection", "Illuminate\\Contracts\\Queue\\Queue"),
    (
        "queue.failer",
        "Illuminate\\Queue\\Failed\\FailedJobProviderInterface",
    ),
    ("redirect", "Illuminate\\Routing\\Redirector"),
    ("redis", "Illuminate\\Redis\\RedisManager"),
    (
        "redis.connection",
        "Illuminate\\Redis\\Connections\\Connection",
    ),
    ("request", "Illuminate\\Http\\Request"),
    ("router", "Illuminate\\Routing\\Router"),
    ("session", "Illuminate\\Session\\SessionManager"),
    ("session.store", "Illuminate\\Session\\Store"),
    ("url", "Illuminate\\Routing\\UrlGenerator"),
    ("validator", "Illuminate\\Validation\\Factory"),
];

pub fn path_base(name: &str) -> Option<&'static str> {
    Some(match name {
        "base_path" | "basepath" => "",
        "app_path" | "apppath" => "app/",
        "config_path" | "configpath" => "config/",
        "database_path" | "databasepath" => "database/",
        "lang_path" | "langpath" => "lang/",
        "public_path" | "publicpath" => "public/",
        "resource_path" | "resourcepath" => "resources/",
        "storage_path" | "storagepath" => "storage/",
        "bootstrappath" => "bootstrap/",
        _ => return None,
    })
}

const STORAGE_PATHS: [&str; 31] = [
    "get",
    "put",
    "exists",
    "missing",
    "delete",
    "url",
    "temporaryurl",
    "path",
    "download",
    "size",
    "lastmodified",
    "mimetype",
    "files",
    "allfiles",
    "directories",
    "alldirectories",
    "makedirectory",
    "deletedirectory",
    "copy",
    "move",
    "prepend",
    "append",
    "readstream",
    "writestream",
    "json",
    "setvisibility",
    "getvisibility",
    "putfile",
    "putfileas",
    "response",
    "checksum",
];

const REQUEST_INPUTS: [&str; 26] = [
    "input",
    "get",
    "post",
    "query",
    "string",
    "str",
    "integer",
    "float",
    "boolean",
    "date",
    "enum",
    "collect",
    "array",
    "has",
    "hasany",
    "filled",
    "anyfilled",
    "missing",
    "exists",
    "isnotfilled",
    "only",
    "except",
    "file",
    "hasfile",
    "validated",
    "safe",
];

fn storage_position(name: &str, arg: &StringArg) -> Option<(Ctx, usize, bool)> {
    if !STORAGE_PATHS.contains(&name) {
        return None;
    }
    let position = if matches!(name, "copy" | "move") && arg.arg < 2 {
        arg.arg
    } else {
        0
    };
    Some((Ctx::DiskPaths, position, name == "delete"))
}

fn is_request(class: &str) -> bool {
    matches!(
        class,
        "$request" | "request()" | "request" | "$this->request"
    )
}

fn is_app(class: &str) -> bool {
    matches!(class, "app()" | "$app" | "app" | "$this->app")
}

const SCHEMA_CALLS: [&str; 6] = [
    "schema",
    "jsoncontent",
    "xmlcontent",
    "items",
    "property",
    "additionalproperties",
];
const OPERATIONS: [&str; 9] = [
    "get", "post", "put", "patch", "delete", "options", "head", "trace", "pathitem",
];

fn openapi_ctx(arg: &StringArg) -> Option<(Ctx, usize, bool)> {
    let name = arg.callee.name.as_str();
    let ctx = match arg.param.as_deref()? {
        "ref" => Ctx::OpenApiRef,
        "tags" => Ctx::Section("tags"),
        "security" => Ctx::Section("securitySchemes"),
        "in" => Ctx::Static(IN),
        "type" => Ctx::Static(TYPES),
        "format" => Ctx::Static(FORMATS),
        "required" if SCHEMA_CALLS.contains(&name) => Ctx::SchemaProperties,
        "path" if OPERATIONS.contains(&name) => Ctx::RoutePaths,
        "name" if name == "parameter" => Ctx::PathParams,
        _ => return None,
    };
    Some((ctx, arg.arg, true))
}

const COLUMN_METHODS: [&str; 42] = [
    "where",
    "orwhere",
    "wherenot",
    "orwherenot",
    "wherein",
    "orwherein",
    "wherenotin",
    "orwherenotin",
    "wherenull",
    "orwherenull",
    "wherenotnull",
    "orwherenotnull",
    "wherebetween",
    "orwherebetween",
    "wherenotbetween",
    "wheredate",
    "wheremonth",
    "whereday",
    "whereyear",
    "wheretime",
    "wherecolumn",
    "wherelike",
    "orwherelike",
    "firstwhere",
    "orderby",
    "orderbydesc",
    "latest",
    "oldest",
    "groupby",
    "having",
    "pluck",
    "value",
    "select",
    "addselect",
    "increment",
    "decrement",
    "sum",
    "avg",
    "average",
    "max",
    "min",
    "wherejsoncontains",
];

const COLUMN_KEY_METHODS: [&str; 10] = [
    "create",
    "update",
    "fill",
    "forcefill",
    "firstorcreate",
    "firstornew",
    "updateorcreate",
    "where",
    "make",
    "forcecreate",
];

const RELATION_METHODS: [&str; 22] = [
    "with",
    "without",
    "withonly",
    "load",
    "loadmissing",
    "loadcount",
    "has",
    "orhas",
    "doesnthave",
    "ordoesnthave",
    "wherehas",
    "orwherehas",
    "wheredoesnthave",
    "orwheredoesnthave",
    "withcount",
    "withexists",
    "withsum",
    "withavg",
    "withmax",
    "withmin",
    "whererelation",
    "withwherehas",
];

const VERBS: [&str; 8] = [
    "get", "post", "put", "patch", "delete", "options", "any", "match",
];

fn model_head(head: Option<&str>) -> Option<String> {
    let head = head?;
    head.chars()
        .next()
        .is_some_and(char::is_uppercase)
        .then(|| head.to_owned())
}

fn ctx_of(arg: &StringArg, scope: &Scope) -> Option<(Ctx, usize, bool)> {
    let c = &arg.callee;
    let name = c.name.as_str();
    let class = c.class.as_deref().unwrap_or("");
    use Ctx::Simple as S;
    if is_openapi(c) {
        return openapi_ctx(arg);
    }
    Some(match c.kind {
        CallKind::Function => match name {
            "config" => (S(RefKind::Config), 0, false),
            "route" | "to_route" => (S(RefKind::Route), 0, false),
            "view" => (S(RefKind::View), 0, false),
            "env" => (S(RefKind::Env), 0, false),
            "__" | "trans" | "trans_choice" => (S(RefKind::Translation), 0, false),
            "auth" => (S(RefKind::Guard), 0, false),
            "asset" | "secure_asset" | "mix" => (S(RefKind::Asset), 0, false),
            "inertia" => (S(RefKind::InertiaPage), 0, false),
            "logs" => (S(RefKind::LogChannel), 0, false),
            "url" | "redirect" => (Ctx::RoutePaths, 0, false),
            "app" | "resolve" => (Ctx::Container, 0, false),
            "old" => (Ctx::RequestFields, 0, false),
            "request" => (Ctx::RequestFields, 0, true),
            p if p.ends_with("_path") && path_base(p).is_some() => {
                (Ctx::Paths(path_base(p)?), 0, false)
            }
            "action" => (Ctx::Methods(arg.class_ref.clone()?), 0, true),
            "validator" => (Ctx::Rules, 1, true),
            "privatechannel" | "channel" | "presencechannel" | "encryptedprivatechannel" => {
                (S(RefKind::Channel), 0, false)
            }
            _ => return None,
        },
        CallKind::Static => match (class, name) {
            (
                "config",
                "get" | "set" | "has" | "string" | "integer" | "float" | "boolean" | "array"
                | "collection" | "prepend" | "push",
            ) => (S(RefKind::Config), 0, false),
            ("route", "has") => (S(RefKind::Route), 0, true),
            ("route", "view") => (S(RefKind::View), 1, false),
            ("route", "inertia") => (S(RefKind::InertiaPage), 1, false),
            ("route", "middleware" | "withoutmiddleware") => (Ctx::Middleware, 0, true),
            ("route", v) if VERBS.contains(&v) => (
                Ctx::Methods(arg.class_ref.clone()?),
                if v == "match" { 2 } else { 1 },
                true,
            ),
            ("url" | "redirect", "route" | "signedroute" | "temporarysignedroute") => {
                (S(RefKind::Route), 0, false)
            }
            ("view", "make" | "exists") => (S(RefKind::View), 0, false),
            ("view", "first") => (S(RefKind::View), 0, true),
            ("env", "get") => (S(RefKind::Env), 0, false),
            ("lang", "get" | "has" | "choice") => (S(RefKind::Translation), 0, false),
            ("storage", "disk" | "fake" | "persistentfake" | "build") => {
                (S(RefKind::Disk), 0, false)
            }
            ("storage", m) if STORAGE_PATHS.contains(&m) => return storage_position(m, arg),
            ("app", m) if path_base(m).is_some() => (Ctx::Paths(path_base(m)?), 0, false),
            ("app", "make" | "makewith" | "bound" | "has" | "get") => (Ctx::Container, 0, false),
            ("url" | "redirect", "to") => (Ctx::RoutePaths, 0, false),
            ("request", m) if REQUEST_INPUTS.contains(&m) => (Ctx::RequestFields, 0, true),
            ("cache", "store" | "driver" | "memo") => (S(RefKind::CacheStore), 0, false),
            ("queue", "connection") => (S(RefKind::Queue), 0, false),
            ("log", "channel") => (S(RefKind::LogChannel), 0, false),
            ("log", "stack") => (S(RefKind::LogChannel), 0, true),
            ("db", "connection") | ("schema", "connection") => (S(RefKind::DbConnection), 0, false),
            ("db", "table") => (S(RefKind::Table), 0, false),
            (
                "schema",
                "create" | "table" | "drop" | "dropifexists" | "hastable" | "hascolumn"
                | "hascolumns" | "getcolumns" | "rename",
            ) => (S(RefKind::Table), 0, false),
            ("mail", "mailer") => (S(RefKind::Mailer), 0, false),
            ("auth", "guard" | "shoulduse") => (S(RefKind::Guard), 0, false),
            ("password", "broker") => (S(RefKind::PasswordBroker), 0, false),
            ("broadcast", "connection" | "driver") => (S(RefKind::Broadcaster), 0, false),
            (
                "gate",
                "allows" | "denies" | "check" | "any" | "none" | "authorize" | "inspect" | "has"
                | "raw",
            ) => (S(RefKind::Ability), 0, true),
            ("inertia", "render") => (S(RefKind::InertiaPage), 0, false),
            ("vite", "asset" | "content") => (S(RefKind::Asset), 0, false),
            ("validator", "make" | "validate") => (Ctx::Rules, 1, true),
            ("artisan", "call" | "queue" | "callsilent" | "callsilently")
            | ("schedule", "command") => (S(RefKind::Command), 0, false),
            ("socialite", "driver" | "with" | "buildprovider") => {
                (S(RefKind::SocialiteDriver), 0, false)
            }
            ("role", "findbyname" | "findorcreate") => (S(RefKind::Role), 0, false),
            ("permission", "findbyname" | "findorcreate") => (S(RefKind::Permission), 0, false),
            _ => return model_ctx(arg, name),
        },
        CallKind::Method => match name {
            m if arg.head.as_deref() == Some("Storage") && STORAGE_PATHS.contains(&m) => {
                return storage_position(m, arg);
            }
            m if path_base(m).is_some() && !m.contains('_') => {
                (Ctx::Paths(path_base(m)?), 0, false)
            }
            "path" if is_app(class) => (Ctx::Paths("app/"), 0, false),
            "make" | "makewith" | "bound" if is_app(class) => (Ctx::Container, 0, false),
            "to" if matches!(class, "url()" | "redirect()") => (Ctx::RoutePaths, 0, false),
            m if is_request(class) && REQUEST_INPUTS.contains(&m) => (Ctx::RequestFields, 0, true),
            "route" if matches!(class, "$request" | "request()" | "$this") => return None,
            "route" | "signedroute" | "temporarysignedroute" => (S(RefKind::Route), 0, false),
            "routeis" | "named" => (S(RefKind::Route), 0, true),
            "view" | "markdown" | "text" | "layout" => (S(RefKind::View), 0, false),
            "middleware" | "withoutmiddleware" => (Ctx::Middleware, 0, true),
            "can" | "cannot" | "cant" | "authorize" | "authorizeforuser" => (
                S(RefKind::Ability),
                if name == "authorizeforuser" { 1 } else { 0 },
                true,
            ),
            "guard" => (S(RefKind::Guard), 0, false),
            "tokencan" | "tokencant" => (S(RefKind::TokenAbility), 0, false),
            "createtoken" => (S(RefKind::TokenAbility), 1, true),
            "onqueue" => (S(RefKind::QueueName), 0, false),
            "command" if arg.head.as_deref() == Some("$schedule") => {
                (S(RefKind::Command), 0, false)
            }
            "artisan" | "call" | "callsilent" | "callsilently"
                if arg.head.as_deref() == Some("$this") =>
            {
                (S(RefKind::Command), 0, false)
            }
            "hasrole" | "hasanyrole" | "hasallroles" | "hasexactroles" | "assignrole"
            | "removerole" | "syncroles" | "role" => (S(RefKind::Role), arg.arg, true),
            "haspermissionto"
            | "hasanypermission"
            | "hasallpermissions"
            | "givepermissionto"
            | "revokepermissionto"
            | "syncpermissions"
            | "checkpermissionto"
            | "hasdirectpermission"
            | "permission" => (S(RefKind::Permission), arg.arg, true),
            "onconnection" => (S(RefKind::Queue), 0, false),
            v if VERBS.contains(&v) && arg.head.as_deref() == Some("Route") => (
                Ctx::Methods(arg.class_ref.clone()?),
                if v == "match" { 2 } else { 1 },
                true,
            ),
            "validate" if arg.arg == 0 => (Ctx::Rules, 0, true),
            "validate" | "validatewithbag" => (Ctx::Rules, 1, true),
            _ => return model_ctx(arg, name),
        },
        CallKind::Directive => match name {
            "include" | "includeif" | "extends" | "each" | "component" => {
                (S(RefKind::View), 0, false)
            }
            "includewhen" | "includeunless" => (S(RefKind::View), 1, false),
            "includefirst" => (S(RefKind::View), 0, true),
            "lang" | "choice" => (S(RefKind::Translation), 0, false),
            "section" | "yield" | "hassection" | "sectionmissing" => {
                (S(RefKind::Section), 0, false)
            }
            "push" | "prepend" | "stack" | "pushonce" | "prependonce" => {
                (S(RefKind::Stack), 0, false)
            }
            "pushif" => (S(RefKind::Stack), 1, false),
            "can" | "cannot" | "canany" | "elsecan" | "elsecannot" => {
                (S(RefKind::Ability), 0, true)
            }
            "livewire" => (S(RefKind::Livewire), 0, false),
            "vite" => (S(RefKind::Asset), 0, true),
            "auth" | "guest" => (S(RefKind::Guard), 0, false),
            "role" | "hasrole" | "hasanyrole" | "hasallroles" | "unlessrole" | "hasexactroles" => {
                (S(RefKind::Role), 0, true)
            }
            "haspermission" | "hasanypermission" | "hasallpermissions" => {
                (S(RefKind::Permission), 0, true)
            }
            _ => return None,
        },
        CallKind::Property => match name {
            "casts" if scope.model && !arg.is_key && arg.after_arrow => (Ctx::Casts, arg.arg, true),
            "fillable" | "guarded" | "hidden" | "visible" | "dates" | "casts" | "appends"
                if scope.model =>
            {
                (Ctx::Columns(scope.class.clone()), arg.arg, true)
            }
            "with" | "withcount" | "touches" if scope.model => {
                (Ctx::Relations(scope.class.clone()?), arg.arg, true)
            }
            "rules" => (Ctx::Rules, arg.arg, true),
            "middleware" => (Ctx::Middleware, arg.arg, true),
            _ => return None,
        },
        CallKind::Attribute => match name {
            "fillable" | "guarded" | "hidden" | "visible" | "appends" if scope.model => {
                (Ctx::Columns(scope.class.clone()), 0, true)
            }
            "with" | "touches" | "withcount" if scope.model => {
                (Ctx::Relations(scope.class.clone()?), 0, true)
            }
            "table" => (S(RefKind::Table), 0, false),
            "connection" => (S(RefKind::DbConnection), 0, false),
            _ => return None,
        },
        CallKind::Return => match name {
            "rules" => (Ctx::Rules, arg.arg, true),
            "casts" if scope.model && !arg.is_key && arg.after_arrow => (Ctx::Casts, arg.arg, true),
            "casts" if scope.model => (Ctx::Columns(scope.class.clone()), arg.arg, true),
            "middleware" => (Ctx::Middleware, arg.arg, true),
            _ => return None,
        },
    })
}

const ENUM_VALUE_METHODS: [&str; 9] = [
    "where",
    "orwhere",
    "wherenot",
    "orwherenot",
    "firstwhere",
    "wherein",
    "orwherein",
    "wherenotin",
    "orwherenotin",
];

fn model_ctx(arg: &StringArg, name: &str) -> Option<(Ctx, usize, bool)> {
    let model = model_head(arg.head.as_deref())?;
    if ENUM_VALUE_METHODS.contains(&name) && (1..=2).contains(&arg.arg) && !arg.is_key {
        return Some((Ctx::EnumValues(model), arg.arg, name.ends_with("in")));
    }
    if RELATION_METHODS.contains(&name) {
        return Some((Ctx::Relations(model), 0, true));
    }
    if (arg.is_key || arg.in_array && !arg.after_arrow) && COLUMN_KEY_METHODS.contains(&name) {
        return Some((Ctx::Columns(Some(model)), arg.arg, true));
    }
    if COLUMN_METHODS.contains(&name) {
        let any_arg = matches!(name, "select" | "addselect" | "groupby" | "pluck");
        return Some((
            Ctx::Columns(Some(model)),
            if any_arg && arg.arg <= 1 { arg.arg } else { 0 },
            any_arg || name == "wherein",
        ));
    }
    None
}

fn bounds(value: &str, at: usize, separators: &[char]) -> (usize, usize) {
    let at = at.min(value.len());
    let start = value[..at].rfind(separators).map_or(0, |i| i + 1);
    let end = value[at..].find(separators).map_or(value.len(), |i| at + i);
    (start, end)
}

fn rules_segment(value: &str, at: usize) -> Option<Segment> {
    let (s, e) = bounds(value, at, &['|']);
    let rule = &value[s..e];
    let colon = rule.find(':').map(|c| s + c);
    match colon {
        Some(c) if at > c => {
            let name = &value[s..c];
            let (ps, pe) = bounds(value, at, &['|', ':', ',']);
            let index = value[c + 1..ps].matches(',').count();
            let want = match (name, index) {
                ("exists" | "unique", 0) => Want::Kind(RefKind::Table),
                ("exists" | "unique", 1) => {
                    let table = value[c + 1..]
                        .split([',', '|'])
                        .next()?
                        .rsplit('.')
                        .next()?
                        .to_owned();
                    Want::Columns {
                        model: None,
                        table: Some(table),
                    }
                }
                _ => return None,
            };
            Some(Segment {
                want,
                start: ps,
                end: pe,
            })
        }
        _ => Some(Segment {
            want: Want::Kind(RefKind::Rule),
            start: s,
            end: colon.unwrap_or(e),
        }),
    }
}

pub const CAST_TYPES: [(&str, &str); 39] = [
    ("array", "JSON column ⇄ PHP array"),
    ("boolean", "true / false"),
    ("bool", "true / false"),
    ("collection", "JSON column ⇄ Collection"),
    ("date", "Carbon at midnight · date:Y-m-d"),
    ("datetime", "Carbon · datetime:Y-m-d H:i"),
    (
        "custom_datetime",
        "Carbon with a format · custom_datetime:Y-m-d",
    ),
    ("immutable_date", "CarbonImmutable at midnight"),
    (
        "immutable_datetime",
        "CarbonImmutable · immutable_datetime:Y-m-d H:i",
    ),
    ("immutable_custom_datetime", "CarbonImmutable with a format"),
    ("timestamp", "Unix timestamp as int"),
    ("decimal", "string with fixed decimals · decimal:2"),
    ("double", "float"),
    ("float", "float"),
    ("real", "float"),
    ("integer", "int"),
    ("int", "int"),
    ("string", "string"),
    ("object", "JSON column ⇄ stdClass"),
    ("json", "JSON column ⇄ PHP array"),
    ("json:unicode", "JSON without escaping unicode"),
    ("hashed", "hashed on set (passwords)"),
    ("encrypted", "encrypted string · encrypted:array"),
    ("encrypted:array", "encrypted JSON ⇄ array"),
    ("encrypted:collection", "encrypted JSON ⇄ Collection"),
    ("encrypted:json", "encrypted JSON ⇄ array"),
    ("encrypted:object", "encrypted JSON ⇄ stdClass"),
    (
        "Illuminate\\Database\\Eloquent\\Casts\\AsArrayObject",
        "AsArrayObject · JSON ⇄ ArrayObject",
    ),
    (
        "Illuminate\\Database\\Eloquent\\Casts\\AsCollection",
        "AsCollection · JSON ⇄ Collection",
    ),
    (
        "Illuminate\\Database\\Eloquent\\Casts\\AsEncryptedArrayObject",
        "AsEncryptedArrayObject",
    ),
    (
        "Illuminate\\Database\\Eloquent\\Casts\\AsEncryptedCollection",
        "AsEncryptedCollection",
    ),
    (
        "Illuminate\\Database\\Eloquent\\Casts\\AsEnumArrayObject",
        "AsEnumArrayObject · :App\\Enums\\…",
    ),
    (
        "Illuminate\\Database\\Eloquent\\Casts\\AsEnumCollection",
        "AsEnumCollection · :App\\Enums\\…",
    ),
    (
        "Illuminate\\Database\\Eloquent\\Casts\\AsStringable",
        "AsStringable · Stringable",
    ),
    (
        "Illuminate\\Database\\Eloquent\\Casts\\AsUri",
        "AsUri · Illuminate\\Support\\Uri",
    ),
    (
        "Illuminate\\Database\\Eloquent\\Casts\\AsFluent",
        "AsFluent · JSON ⇄ Fluent",
    ),
    (
        "Illuminate\\Database\\Eloquent\\Casts\\AsHtmlString",
        "AsHtmlString · HtmlString",
    ),
    (
        "Illuminate\\Database\\Eloquent\\Casts\\AsBinary",
        "AsBinary · binary UUID / ULID",
    ),
    (
        "Illuminate\\Database\\Eloquent\\Casts\\AsVector",
        "AsVector · vector column ⇄ array",
    ),
];

const CAST_JSON: [(&str, &str); 1] = [("unicode", "keep unicode characters unescaped")];

const CAST_DECIMALS: [(&str, &str); 5] = [
    ("0", "no decimal places"),
    ("1", "1 decimal place"),
    ("2", "2 decimal places (money)"),
    ("3", "3 decimal places"),
    ("4", "4 decimal places"),
];

const CAST_FORMATS: [(&str, &str); 9] = [
    ("Y-m-d", "2026-11-07"),
    ("Y-m-d H:i:s", "2026-11-07 14:30:00"),
    ("Y-m-d H:i", "2026-11-07 14:30"),
    ("d/m/Y", "07/11/2026"),
    ("d/m/Y H:i", "07/11/2026 14:30"),
    ("m/d/Y", "11/07/2026"),
    ("H:i", "14:30"),
    ("c", "ISO 8601 · 2026-11-07T14:30:00+00:00"),
    ("U", "Unix timestamp"),
];

const CAST_ENCRYPTED: [(&str, &str); 4] = [
    ("array", "encrypted JSON ⇄ array"),
    ("collection", "encrypted JSON ⇄ Collection"),
    ("json", "encrypted JSON ⇄ array"),
    ("object", "encrypted JSON ⇄ stdClass"),
];

const CAST_BINARY: [(&str, &str); 2] = [("uuid", "16-byte UUID"), ("ulid", "16-byte ULID")];

fn casts_segment(value: &str, at: usize) -> Option<Segment> {
    let at = at.min(value.len());
    match value.find(':') {
        Some(c) if at > c => {
            let name = value[..c].rsplit('\\').next().unwrap_or(&value[..c]);
            let values: &'static [(&'static str, &'static str)] = match name {
                "decimal" => &CAST_DECIMALS,
                "date"
                | "datetime"
                | "custom_datetime"
                | "immutable_date"
                | "immutable_datetime"
                | "immutable_custom_datetime" => &CAST_FORMATS,
                "encrypted" => &CAST_ENCRYPTED,
                "json" => &CAST_JSON,
                "AsBinary" => &CAST_BINARY,
                _ => return None,
            };
            Some(Segment {
                want: Want::Static(values),
                start: c + 1,
                end: value.len(),
            })
        }
        _ => Some(Segment {
            want: Want::CastTypes,
            start: 0,
            end: value.len(),
        }),
    }
}

fn middleware_segment(value: &str, at: usize) -> Option<Segment> {
    let colon = value.find(':');
    match colon {
        Some(c) if at > c => {
            let name = &value[..c];
            let (ps, pe) = bounds(value, at, &[':', ',']);
            let index = value[c + 1..ps].matches(',').count();
            let kind = match (name, index) {
                ("auth" | "auth.basic" | "auth.session" | "guest", _) => RefKind::Guard,
                ("can", 0) => RefKind::Ability,
                ("throttle", 0) if !value[ps..pe].starts_with(|ch: char| ch.is_ascii_digit()) => {
                    RefKind::RateLimiter
                }
                ("abilities" | "ability", _) => RefKind::TokenAbility,
                ("role" | "role_or_permission", _) => RefKind::Role,
                ("permission", _) => RefKind::Permission,
                _ => return None,
            };
            Some(Segment {
                want: Want::Kind(kind),
                start: ps,
                end: pe,
            })
        }
        _ => Some(Segment {
            want: Want::Kind(RefKind::Middleware),
            start: 0,
            end: colon.unwrap_or(value.len()),
        }),
    }
}

fn relation_segment(model: String, value: &str, at: usize) -> Option<Segment> {
    let colon = value.find(':');
    if let Some(c) = colon
        && at > c
    {
        let path: Vec<String> = value[..c].split('.').map(str::to_owned).collect();
        let (s, e) = bounds(value, at, &[':', ',']);
        return Some(Segment {
            want: Want::RelationColumns { model, path },
            start: s,
            end: e,
        });
    }
    let limit = colon.unwrap_or(value.len());
    let (s, e) = bounds(&value[..limit], at.min(limit), &['.']);
    let path = if s == 0 {
        Vec::new()
    } else {
        value[..s - 1].split('.').map(str::to_owned).collect()
    };
    Some(Segment {
        want: Want::Relations { model, path },
        start: s,
        end: e,
    })
}

pub fn segment(arg: &StringArg, at: usize, scope: &Scope) -> Option<Segment> {
    let (ctx, position, arrays) = ctx_of(arg, scope)?;
    if arg.arg != position || arg.in_array && !arrays {
        return None;
    }
    let value = arg.value.as_str();
    let whole = |want| {
        Some(Segment {
            want,
            start: 0,
            end: value.len(),
        })
    };
    match ctx {
        Ctx::Simple(kind) if !arg.is_key => whole(Want::Kind(kind)),
        Ctx::Middleware if !arg.is_key => middleware_segment(value, at),
        Ctx::Rules
            if !arg.is_key
                && (arg.in_array
                    || matches!(arg.callee.kind, CallKind::Return | CallKind::Property)) =>
        {
            rules_segment(value, at)
        }
        Ctx::Casts if !arg.is_key => casts_segment(value, at),
        Ctx::Columns(model) if !value.contains(['.', ' ', '(']) => {
            whole(Want::Columns { model, table: None })
        }
        Ctx::Relations(model) => relation_segment(model, value, at),
        Ctx::Methods(class) if !arg.is_key => whole(Want::Methods(class)),
        Ctx::OpenApiRef if !arg.is_key => whole(Want::OpenApiRef),
        Ctx::Section(section) if arg.is_key == (section == "securitySchemes") => {
            whole(Want::OpenApiSection(section))
        }
        Ctx::Static(values) if !arg.is_key => whole(Want::Static(values)),
        Ctx::Container if !arg.is_key => whole(Want::Container),
        Ctx::EnumValues(model) => whole(Want::EnumValues {
            model,
            call_start: arg.call_start,
            index: arg.arg,
        }),
        Ctx::SchemaProperties if !arg.is_key && arg.in_array => {
            whole(Want::SchemaProperties(arg.call_start))
        }
        Ctx::RoutePaths if !arg.is_key => whole(Want::RoutePaths),
        Ctx::PathParams if !arg.is_key => whole(Want::PathParams(arg.start)),
        Ctx::Paths(base) if !arg.is_key => whole(Want::Paths(base)),
        Ctx::DiskPaths if !arg.is_key => whole(Want::DiskPaths(arg.call_start)),
        Ctx::RequestFields if !arg.is_key => whole(Want::RequestFields),
        _ => None,
    }
}

pub fn is_rule_key(arg: &StringArg) -> bool {
    arg.is_key && matches!(ctx_of(arg, &Scope::default()), Some((Ctx::Rules, _, _)))
}

pub fn checkable(kind: RefKind, value: &str) -> bool {
    !value.is_empty()
        && !value.contains(['$', '{', '*'])
        && match kind {
            RefKind::View => !value.contains("::"),
            _ => true,
        }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::php::args_in;

    fn wants(text: &str, blade: bool, scope: &Scope) -> Vec<(String, Option<Want>)> {
        args_in(text, blade)
            .into_iter()
            .map(|a| {
                let at = a.value.len();
                (a.value.clone(), segment(&a, at, scope).map(|s| s.want))
            })
            .collect()
    }

    fn k(kind: RefKind) -> Option<Want> {
        Some(Want::Kind(kind))
    }

    #[test]
    fn container_and_request_properties_of_this_are_receivers() {
        let text = "<?php\n$this->app->make('cache');\n$this->app->path('Models');\n$this->request->input('total');\n$this->other->make('x');\n";
        assert_eq!(
            wants(text, false, &Scope::default()),
            [
                ("cache".into(), Some(Want::Container)),
                ("Models".into(), Some(Want::Paths("app/"))),
                ("total".into(), Some(Want::RequestFields)),
                ("x".into(), None),
            ]
        );
    }

    #[test]
    fn suggests_paths_disks_container_ids_uris_and_request_fields() {
        let text = "<?php\npublic_path('css/app.css');\nstorage_path('logs');\nbase_path('');\napp()->storagePath('app');\nApp::langPath('en');\nStorage::put('a.txt', 'x');\nStorage::disk('public')->exists('img');\nStorage::copy('a', 'b');\nlogs('daily');\napp('cache');\nresolve('files');\nurl('/orders');\nredirect('/home');\nredirect()->to('/back');\nold('email');\nrequest('name');\n$request->input('total');\n$request->only(['a', 'b']);\n$collection->get('x');\n";
        let got = wants(text, false, &Scope::default());
        let disk = |s: &str| Some(Want::DiskPaths(text.find(s).unwrap()));
        assert_eq!(
            got,
            [
                ("css/app.css".into(), Some(Want::Paths("public/"))),
                ("logs".into(), Some(Want::Paths("storage/"))),
                ("".into(), Some(Want::Paths(""))),
                ("app".into(), Some(Want::Paths("storage/"))),
                ("en".into(), Some(Want::Paths("lang/"))),
                ("a.txt".into(), disk("('a.txt'")),
                ("x".into(), None),
                ("public".into(), k(RefKind::Disk)),
                ("img".into(), disk("('img'")),
                ("a".into(), disk("('a', 'b')")),
                ("b".into(), disk("('a', 'b')")),
                ("daily".into(), k(RefKind::LogChannel)),
                ("cache".into(), Some(Want::Container)),
                ("files".into(), Some(Want::Container)),
                ("/orders".into(), Some(Want::RoutePaths)),
                ("/home".into(), Some(Want::RoutePaths)),
                ("/back".into(), Some(Want::RoutePaths)),
                ("email".into(), Some(Want::RequestFields)),
                ("name".into(), Some(Want::RequestFields)),
                ("total".into(), Some(Want::RequestFields)),
                ("a".into(), Some(Want::RequestFields)),
                ("b".into(), Some(Want::RequestFields)),
                ("x".into(), None),
            ]
        );
    }

    #[test]
    fn recognises_laravel_helpers_facades_and_directives() {
        let text = "<?php\nConfig::get('app.name');\nreturn redirect()->route('home');\nRoute::view('/a', 'about');\nView::first(['a', 'b']);\n__('auth.failed');\n$request->routeIs('admin.*');\nview('x', ['k' => 'v']);\nfoo('bar');\n$id = $request->route('userId');\nStorage::disk('s3');\nLog::channel('slack');\n";
        let got = wants(text, false, &Scope::default());
        assert_eq!(
            got,
            [
                ("app.name".into(), k(RefKind::Config)),
                ("home".into(), k(RefKind::Route)),
                ("/a".into(), None),
                ("about".into(), k(RefKind::View)),
                ("a".into(), k(RefKind::View)),
                ("b".into(), k(RefKind::View)),
                ("auth.failed".into(), k(RefKind::Translation)),
                ("admin.*".into(), k(RefKind::Route)),
                ("x".into(), k(RefKind::View)),
                ("k".into(), None),
                ("v".into(), None),
                ("bar".into(), None),
                ("userId".into(), None),
                ("s3".into(), k(RefKind::Disk)),
                ("slack".into(), k(RefKind::LogChannel)),
            ]
        );
        let blade = "@extends('layouts.app')\n@includeWhen($ok, 'partials.alert')\n@lang('messages.hi')\n@section('title')\n@can('update')\n";
        assert_eq!(
            wants(blade, true, &Scope::default()),
            [
                ("layouts.app".into(), k(RefKind::View)),
                ("partials.alert".into(), k(RefKind::View)),
                ("messages.hi".into(), k(RefKind::Translation)),
                ("title".into(), k(RefKind::Section)),
                ("update".into(), k(RefKind::Ability)),
            ]
        );
    }

    #[test]
    fn splits_middleware_and_rules() {
        let text = "<?php\nRoute::middleware(['auth:sanctum', 'throttle:uploads'])->get('/x', [UploadController::class, 'store']);\n";
        let got = wants(text, false, &Scope::default());
        assert_eq!(got[0].1, k(RefKind::Guard));
        assert_eq!(got[1].1, k(RefKind::RateLimiter));
        assert_eq!(got[3].1, Some(Want::Methods("UploadController".into())));
        let arg = &args_in("<?php\nRoute::middleware('auth:api');", false)[0];
        assert_eq!(
            segment(arg, 2, &Scope::default()).map(|s| (s.want, s.start, s.end)),
            Some((Want::Kind(RefKind::Middleware), 0, 4))
        );
        let rules = "<?php\nclass R { public function rules(): array { return ['email' => 'required|exists:users,email', 'tags' => ['array', 'max:3']]; } }";
        let args = args_in(rules, false);
        let email = &args[1];
        assert_eq!(
            segment(email, 3, &Scope::default()).unwrap().want,
            Want::Kind(RefKind::Rule)
        );
        let at = email.value.find("users").unwrap() + 2;
        assert_eq!(
            segment(email, at, &Scope::default()).unwrap().want,
            Want::Kind(RefKind::Table)
        );
        let at = email.value.len();
        assert_eq!(
            segment(email, at, &Scope::default()).unwrap().want,
            Want::Columns {
                model: None,
                table: Some("users".into())
            }
        );
        assert_eq!(
            segment(&args[3], 1, &Scope::default()).unwrap().want,
            Want::Kind(RefKind::Rule)
        );
    }

    #[test]
    fn understands_openapi_attributes() {
        let text = "<?php\nuse OpenApi\\Attributes as OA;\n#[OA\\Post(path: '/users', tags: ['Users'], security: [['bearerAuth' => []]], parameters: [new OA\\Parameter(name: 'id', in: 'path', schema: new OA\\Schema(type: 'integer', format: 'int64'))], responses: [new OA\\Response(response: 404, content: new OA\\JsonContent(ref: '#/components/schemas/X'))])]\n#[OA\\Schema(schema: 'S', required: ['name'], properties: [new OA\\Property(property: 'name')])]\n";
        let got: Vec<_> = wants(text, false, &Scope::default())
            .into_iter()
            .map(|(v, w)| (v, w.map(|w| format!("{w:?}"))))
            .collect();
        let find = |v: &str| {
            got.iter()
                .find(|(x, _)| x == v)
                .and_then(|(_, w)| w.clone())
                .unwrap_or_default()
        };
        assert_eq!(find("/users"), "RoutePaths");
        assert_eq!(find("Users"), "OpenApiSection(\"tags\")");
        assert_eq!(find("bearerAuth"), "OpenApiSection(\"securitySchemes\")");
        assert!(find("id").starts_with("PathParams"));
        assert!(find("path").starts_with("Static"));
        assert!(find("integer").starts_with("Static"));
        assert_eq!(find("#/components/schemas/X"), "OpenApiRef");
        assert!(find("name").starts_with("SchemaProperties"));
    }

    #[test]
    fn understands_package_helpers() {
        let text = "<?php\n$user->hasRole('admin');\n$user->tokenCan('posts:read');\nArtisan::call('cache:clear');\nSocialite::driver('github');\nnew PrivateChannel('chat.1');\nRoute::middleware('role:admin');\n";
        let got = wants(text, false, &Scope::default());
        assert_eq!(got[0].1, k(RefKind::Role));
        assert_eq!(got[1].1, k(RefKind::TokenAbility));
        assert_eq!(got[2].1, k(RefKind::Command));
        assert_eq!(got[3].1, k(RefKind::SocialiteDriver));
        assert_eq!(got[4].1, k(RefKind::Channel));
        assert_eq!(got[5].1, k(RefKind::Role));
    }

    #[test]
    fn understands_eloquent_chains() {
        let text = "<?php\nUser::query()->where('email', $e)->with(['posts.comments', 'team:id,name'])->orderBy('created_at')->get();\nPost::create(['title' => 't']);\n";
        let got = wants(text, false, &Scope::default());
        let user = || "User".to_owned();
        assert_eq!(
            got[0].1,
            Some(Want::Columns {
                model: Some(user()),
                table: None
            })
        );
        assert_eq!(
            got[1].1,
            Some(Want::Relations {
                model: user(),
                path: vec!["posts".into()]
            })
        );
        assert_eq!(
            got[2].1,
            Some(Want::RelationColumns {
                model: user(),
                path: vec!["team".into()]
            })
        );
        assert_eq!(
            got[3].1,
            Some(Want::Columns {
                model: Some(user()),
                table: None
            })
        );
        assert_eq!(
            got[4].1,
            Some(Want::Columns {
                model: Some("Post".into()),
                table: None
            })
        );
        let model = "<?php\nclass Post extends Model { protected $fillable = ['title', 'body']; protected $with = ['author']; }";
        let scope = Scope {
            class: Some("Post".into()),
            model: true,
        };
        let got = wants(model, false, &scope);
        assert_eq!(
            got[0].1,
            Some(Want::Columns {
                model: Some("Post".into()),
                table: None
            })
        );
        assert_eq!(
            got[2].1,
            Some(Want::Relations {
                model: "Post".into(),
                path: vec![]
            })
        );
    }
}
