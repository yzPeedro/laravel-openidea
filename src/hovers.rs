use crate::index::{Entry, RefKind};
use crate::project::rel;

const MAX: usize = 8;

pub fn singular(kind: RefKind) -> &'static str {
    match kind {
        RefKind::Config => "Config",
        RefKind::Route => "Route",
        RefKind::RouteAction => "Route action",
        RefKind::View => "View",
        RefKind::Env => "Env variable",
        RefKind::Translation => "Translation",
        RefKind::Disk => "Disk",
        RefKind::CacheStore => "Cache store",
        RefKind::Queue => "Queue connection",
        RefKind::LogChannel => "Log channel",
        RefKind::DbConnection => "Database connection",
        RefKind::Mailer => "Mailer",
        RefKind::Guard => "Guard",
        RefKind::PasswordBroker => "Password broker",
        RefKind::Broadcaster => "Broadcaster",
        RefKind::Middleware => "Middleware",
        RefKind::Ability => "Ability",
        RefKind::RateLimiter => "Rate limiter",
        RefKind::Table => "Table",
        RefKind::Column => "Column",
        RefKind::Model => "Model",
        RefKind::Relation => "Relation",
        RefKind::Scope => "Scope",
        RefKind::Controller => "Controller method",
        RefKind::Component => "Blade component",
        RefKind::Livewire => "Livewire component",
        RefKind::InertiaPage => "Inertia page",
        RefKind::Section => "Section",
        RefKind::Stack => "Stack",
        RefKind::Asset => "Asset",
        RefKind::Rule => "Validation rule",
        RefKind::OpenApi => "OpenAPI component",
        RefKind::TokenAbility => "Token ability",
        RefKind::Channel => "Broadcast channel",
        RefKind::Command => "Artisan command",
        RefKind::QueueName => "Queue",
        RefKind::Role => "Role",
        RefKind::Permission => "Permission",
        RefKind::SocialiteDriver => "Socialite driver",
        RefKind::Directive => "Blade directive",
        RefKind::Pivot => "Pivot attribute",
    }
}

fn code(text: &str) -> String {
    let text = text.replace(['\n', '\r'], " ");
    if text.contains('`') {
        format!("`` {text} ``")
    } else {
        format!("`{text}`")
    }
}

fn place(root: &str, entry: &Entry) -> Option<String> {
    if entry.location.path.is_empty() {
        return None;
    }
    let own = rel(root, &entry.location.path).replace('\\', "/");
    let range = entry.location.range;
    Some(if range == Default::default() {
        own
    } else {
        format!("{own}:{}", range.start.line + 1)
    })
}

fn detail(root: &str, entry: &Entry) -> Option<String> {
    let detail = entry.detail.as_deref()?.trim();
    let own = rel(root, &entry.location.path).replace('\\', "/");
    (!detail.is_empty() && detail != own && !own.ends_with(detail)).then(|| detail.to_owned())
}

pub fn markdown(
    kind: RefKind,
    key: &str,
    entries: &[Entry],
    actions: &[String],
    root: &str,
) -> Option<String> {
    if entries.is_empty() {
        return None;
    }
    let mut out = format!("**{}** {}", singular(kind), code(key));
    for action in actions.iter().take(MAX) {
        out.push_str(&format!("\n\n→ {}", code(action)));
    }
    let mut lines = Vec::new();
    for entry in entries.iter().take(MAX) {
        let line = match (detail(root, entry), place(root, entry)) {
            (Some(d), Some(p)) => format!("- {} · {}", code(&d), code(&p)),
            (Some(d), None) => format!("- {}", code(&d)),
            (None, Some(p)) => format!("- {}", code(&p)),
            (None, None) => continue,
        };
        if !lines.contains(&line) {
            lines.push(line);
        }
    }
    if !lines.is_empty() {
        out.push_str("\n\n");
        out.push_str(&lines.join("\n"));
    }
    if entries.len() > MAX {
        out.push_str(&format!("\n- … and {} more", entries.len() - MAX));
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use openidea_plugin::{Position, Range};

    fn entry(key: &str, detail: Option<&str>, path: &str, line: u32) -> Entry {
        let at = Position::new(line, 4);
        Entry::new(key, detail.map(str::to_owned), path, Range::new(at, at))
    }

    #[test]
    fn describes_a_route_with_its_uri_action_and_place() {
        let entries = [entry(
            "admin.users.index",
            Some("GET /admin/users"),
            "/project/routes/web.php",
            3,
        )];
        let md = markdown(
            RefKind::Route,
            "admin.users.index",
            &entries,
            &["UsersController@__invoke".to_owned()],
            "/project",
        )
        .unwrap();
        assert_eq!(
            md,
            "**Route** `admin.users.index`\n\n→ `UsersController@__invoke`\n\n- `GET /admin/users` · `routes/web.php:4`"
        );
    }

    #[test]
    fn lists_a_translation_in_each_locale() {
        let entries = [
            entry(
                "messages.welcome",
                Some("en: 'Welcome'"),
                "/project/lang/en/messages.php",
                1,
            ),
            entry(
                "messages.welcome",
                Some("pt_BR: 'Bem-vindo'"),
                "/project/lang/pt_BR/messages.php",
                1,
            ),
        ];
        let md = markdown(
            RefKind::Translation,
            "messages.welcome",
            &entries,
            &[],
            "/project",
        )
        .unwrap();
        assert!(
            md.contains("- `en: 'Welcome'` · `lang/en/messages.php:2`"),
            "{md}"
        );
        assert!(
            md.contains("- `pt_BR: 'Bem-vindo'` · `lang/pt_BR/messages.php:2`"),
            "{md}"
        );
    }

    #[test]
    fn leaves_out_a_detail_that_only_repeats_the_file() {
        let file = Entry::new(
            "app.providers",
            Some("config/app.php".to_owned()),
            "/project/config/app.php",
            Range::default(),
        );
        let md = markdown(RefKind::Config, "app.providers", &[file], &[], "/project").unwrap();
        assert_eq!(md, "**Config** `app.providers`\n\n- `config/app.php`");
    }

    #[test]
    fn says_nothing_about_an_unknown_key_and_escapes_backticks() {
        assert_eq!(markdown(RefKind::View, "nope", &[], &[], "/project"), None);
        assert_eq!(code("a`b"), "`` a`b ``");
    }

    #[test]
    fn caps_long_lists() {
        let entries: Vec<Entry> = (0..10)
            .map(|i| {
                entry(
                    "k",
                    Some(&format!("l{i}: x")),
                    &format!("/project/lang/l{i}/k.php"),
                    0,
                )
            })
            .collect();
        let md = markdown(RefKind::Translation, "k", &entries, &[], "/project").unwrap();
        assert!(md.ends_with("- … and 2 more"), "{md}");
    }
}
