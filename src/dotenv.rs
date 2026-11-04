use crate::index::RefKind;

pub enum Context {
    Key {
        start: usize,
        typed: String,
    },
    Value {
        key: String,
        start: usize,
        typed: String,
    },
}

pub enum Values {
    Kind(RefKind),
    Static(&'static [(&'static str, &'static str)]),
    Locales,
    Boolean,
}

const LOG_LEVELS: &[(&str, &str)] = &[
    ("debug", ""),
    ("info", ""),
    ("notice", ""),
    ("warning", ""),
    ("error", ""),
    ("critical", ""),
    ("alert", ""),
    ("emergency", ""),
];
const SESSION_DRIVERS: &[(&str, &str)] = &[
    ("file", "storage/framework/sessions"),
    ("cookie", "encrypted cookies"),
    ("database", "sessions table"),
    ("redis", ""),
    ("memcached", ""),
    ("dynamodb", ""),
    ("apc", ""),
    ("array", "not persisted"),
];
const ENVIRONMENTS: &[(&str, &str)] = &[
    ("local", ""),
    ("production", ""),
    ("staging", ""),
    ("testing", ""),
];
const MAINTENANCE: &[(&str, &str)] = &[
    ("file", "single server"),
    ("cache", "shared through the cache store"),
];
const SAME_SITE: &[(&str, &str)] = &[
    ("lax", ""),
    ("strict", ""),
    ("none", "requires secure cookies"),
    ("null", ""),
];
const MAIL_SCHEME: &[(&str, &str)] = &[("smtp", ""), ("smtps", "")];
const REDIS_CLIENT: &[(&str, &str)] = &[
    ("phpredis", "PHP extension"),
    ("predis", "composer package"),
];
const HASH_DRIVER: &[(&str, &str)] = &[("bcrypt", ""), ("argon", ""), ("argon2id", "")];
const BOOLEAN_SUFFIXES: [&str; 9] = [
    "_DEBUG",
    "_ENCRYPT",
    "_ENABLED",
    "_SECURE_COOKIE",
    "_HTTP_ONLY",
    "_PARTITIONED_COOKIE",
    "_EXPIRE_ON_CLOSE",
    "_VERIFY",
    "_ACTIVE",
];

pub const LARAVEL_KEYS: &[(&str, &str)] = &[
    ("APP_NAME", "app.name"),
    ("APP_ENV", "app.env"),
    ("APP_KEY", "app.key"),
    ("APP_DEBUG", "app.debug"),
    ("APP_URL", "app.url"),
    ("APP_TIMEZONE", "app.timezone"),
    ("APP_LOCALE", "app.locale"),
    ("APP_FALLBACK_LOCALE", "app.fallback_locale"),
    ("APP_FAKER_LOCALE", "app.faker_locale"),
    ("APP_MAINTENANCE_DRIVER", "app.maintenance.driver"),
    ("APP_MAINTENANCE_STORE", "app.maintenance.store"),
    ("APP_PREVIOUS_KEYS", "app.previous_keys"),
    ("BCRYPT_ROUNDS", "hashing.bcrypt.rounds"),
    ("LOG_CHANNEL", "logging.default"),
    ("LOG_STACK", "logging.channels.stack"),
    ("LOG_DEPRECATIONS_CHANNEL", "logging.deprecations"),
    ("LOG_LEVEL", "logging level"),
    ("DB_CONNECTION", "database.default"),
    ("DB_HOST", ""),
    ("DB_PORT", ""),
    ("DB_DATABASE", ""),
    ("DB_USERNAME", ""),
    ("DB_PASSWORD", ""),
    ("SESSION_DRIVER", "session.driver"),
    ("SESSION_LIFETIME", "minutes"),
    ("SESSION_ENCRYPT", "session.encrypt"),
    ("SESSION_PATH", "session.path"),
    ("SESSION_DOMAIN", "session.domain"),
    ("BROADCAST_CONNECTION", "broadcasting.default"),
    ("FILESYSTEM_DISK", "filesystems.default"),
    ("QUEUE_CONNECTION", "queue.default"),
    ("CACHE_STORE", "cache.default"),
    ("CACHE_PREFIX", "cache.prefix"),
    ("MEMCACHED_HOST", ""),
    ("REDIS_CLIENT", "database.redis.client"),
    ("REDIS_HOST", ""),
    ("REDIS_PASSWORD", ""),
    ("REDIS_PORT", ""),
    ("MAIL_MAILER", "mail.default"),
    ("MAIL_SCHEME", ""),
    ("MAIL_HOST", ""),
    ("MAIL_PORT", ""),
    ("MAIL_USERNAME", ""),
    ("MAIL_PASSWORD", ""),
    ("MAIL_FROM_ADDRESS", "mail.from.address"),
    ("MAIL_FROM_NAME", "mail.from.name"),
    ("AWS_ACCESS_KEY_ID", ""),
    ("AWS_SECRET_ACCESS_KEY", ""),
    ("AWS_DEFAULT_REGION", ""),
    ("AWS_BUCKET", ""),
    ("AWS_USE_PATH_STYLE_ENDPOINT", ""),
    ("VITE_APP_NAME", "exposed to Vite"),
];

pub fn context(text: &str, cursor: usize) -> Option<Context> {
    let line_start = text[..cursor].rfind('\n').map_or(0, |i| i + 1);
    let before = &text[line_start..cursor];
    let trimmed = before.trim_start();
    if trimmed.starts_with('#') {
        return None;
    }
    let indent = before.len() - trimmed.len();
    let body = trimmed.strip_prefix("export ").unwrap_or(trimmed);
    let skip = trimmed.len() - body.len();
    match body.find('=') {
        Some(eq) => {
            let value = &body[eq + 1..];
            let quote = usize::from(value.starts_with(['"', '\'']));
            let start = line_start + indent + skip + eq + 1 + quote;
            Some(Context::Value {
                key: body[..eq].trim().to_owned(),
                start,
                typed: text[start..cursor].to_owned(),
            })
        }
        None if body.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') => Some(Context::Key {
            start: line_start + indent + skip,
            typed: body.to_owned(),
        }),
        None => None,
    }
}

pub fn values_for(key: &str) -> Option<Values> {
    Some(match key {
        "DB_CONNECTION" => Values::Kind(RefKind::DbConnection),
        "CACHE_STORE" | "CACHE_DRIVER" | "APP_MAINTENANCE_STORE" | "SESSION_STORE" => {
            Values::Kind(RefKind::CacheStore)
        }
        "QUEUE_CONNECTION" | "QUEUE_FAILED_DRIVER" => Values::Kind(RefKind::Queue),
        "LOG_CHANNEL" | "LOG_STACK" | "LOG_DEPRECATIONS_CHANNEL" => {
            Values::Kind(RefKind::LogChannel)
        }
        "MAIL_MAILER" => Values::Kind(RefKind::Mailer),
        "FILESYSTEM_DISK" | "FILESYSTEM_CLOUD" => Values::Kind(RefKind::Disk),
        "BROADCAST_CONNECTION" | "BROADCAST_DRIVER" => Values::Kind(RefKind::Broadcaster),
        "AUTH_GUARD" => Values::Kind(RefKind::Guard),
        "AUTH_PASSWORD_BROKER" => Values::Kind(RefKind::PasswordBroker),
        "SESSION_DRIVER" => Values::Static(SESSION_DRIVERS),
        "SESSION_SAME_SITE" => Values::Static(SAME_SITE),
        "LOG_LEVEL" => Values::Static(LOG_LEVELS),
        "APP_ENV" => Values::Static(ENVIRONMENTS),
        "APP_MAINTENANCE_DRIVER" => Values::Static(MAINTENANCE),
        "MAIL_SCHEME" => Values::Static(MAIL_SCHEME),
        "REDIS_CLIENT" => Values::Static(REDIS_CLIENT),
        "HASH_DRIVER" => Values::Static(HASH_DRIVER),
        k if k.ends_with("_LOCALE") => Values::Locales,
        k if BOOLEAN_SUFFIXES.iter().any(|s| k.ends_with(s)) => Values::Boolean,
        _ => return None,
    })
}

pub fn env_key(detail: &str) -> Option<&str> {
    let rest = detail.strip_prefix("env('")?;
    Some(&rest[..rest.find('\'')?])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_key_and_value_positions() {
        let text = "APP_ENV=local\nDB_CONNECTION=my\nexport LOG_\n# CACHE_";
        let Some(Context::Value { key, start, typed }) =
            context(text, text.find("my").unwrap() + 2)
        else {
            panic!()
        };
        assert_eq!(
            (key.as_str(), typed.as_str(), &text[start..start + 2]),
            ("DB_CONNECTION", "my", "my")
        );
        let Some(Context::Key { typed, .. }) = context(text, text.find("LOG_").unwrap() + 4) else {
            panic!()
        };
        assert_eq!(typed, "LOG_");
        assert!(context(text, text.len()).is_none());
        assert!(matches!(values_for("APP_DEBUG"), Some(Values::Boolean)));
        assert!(matches!(
            values_for("APP_FAKER_LOCALE"),
            Some(Values::Locales)
        ));
        assert_eq!(env_key("env('APP_NAME')"), Some("APP_NAME"));
    }
}
