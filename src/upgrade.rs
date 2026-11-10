use crate::checks::{Edit, Finding, Fix};
use crate::project::{find, read_listed, rel};
use crate::sections::{Row, Table, facet};
use openidea_plugin::Project;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum When {
    Always,
    Package(&'static str),
    PathEnds(&'static str),
}

#[derive(Clone, Copy, Debug)]
pub struct Rule {
    pub from: u32,
    pub title: &'static str,
    pub impact: &'static str,
    pub pattern: &'static str,
    pub replacement: Option<&'static str>,
    pub autofix: bool,
    pub url: &'static str,
    pub when: When,
}

const fn rule(
    from: u32,
    title: &'static str,
    impact: &'static str,
    pattern: &'static str,
    replacement: Option<&'static str>,
    autofix: bool,
    url: &'static str,
) -> Rule {
    Rule {
        from,
        title,
        impact,
        pattern,
        replacement,
        autofix,
        url,
        when: When::Always,
    }
}

const fn only(mut r: Rule, when: When) -> Rule {
    r.when = when;
    r
}

const G7: &str = "https://laravel.com/docs/7.x/upgrade";
const G8: &str = "https://laravel.com/docs/8.x/upgrade";
const G9: &str = "https://laravel.com/docs/9.x/upgrade";
const G10: &str = "https://laravel.com/docs/10.x/upgrade";
const G11: &str = "https://laravel.com/docs/11.x/upgrade";
const G12: &str = "https://laravel.com/docs/12.x/upgrade";
const G13: &str = "https://laravel.com/docs/13.x/upgrade";

pub const RULES: &[Rule] = &[
    rule(
        6,
        "The component Method",
        "Medium",
        "Blade::component(",
        Some("Blade::aliasComponent("),
        true,
        G7,
    ),
    rule(
        6,
        "The addHidden / addVisible Methods",
        "Low",
        "->addHidden(",
        Some("->makeHidden("),
        false,
        G7,
    ),
    rule(
        6,
        "The addHidden / addVisible Methods",
        "Low",
        "->addVisible(",
        Some("->makeVisible("),
        false,
        G7,
    ),
    rule(
        6,
        "The Illuminate\\Http\\Resources\\Json\\Resource Class",
        "Low",
        "Illuminate\\Http\\Resources\\Json\\Resource;",
        Some("Illuminate\\Http\\Resources\\Json\\JsonResource;"),
        false,
        G7,
    ),
    rule(
        6,
        "The TestResponse Class",
        "Low",
        "Illuminate\\Foundation\\Testing\\TestResponse",
        Some("Illuminate\\Testing\\TestResponse"),
        true,
        G7,
    ),
    rule(
        6,
        "The Assert Class",
        "Low",
        "Illuminate\\Foundation\\Testing\\Assert",
        Some("Illuminate\\Testing\\Assert"),
        true,
        G7,
    ),
    rule(
        6,
        "Swift Mailer Bindings",
        "Low",
        "'swift.mailer'",
        Some("app('mailer')->getSwiftMailer()"),
        false,
        G7,
    ),
    rule(
        6,
        "Swift Mailer Bindings",
        "Low",
        "'swift.transport'",
        Some("app('mailer')->getSwiftMailer()->getTransport()"),
        false,
        G7,
    ),
    rule(
        6,
        "Symfony 5 Related Upgrades",
        "High",
        "public function report(Exception $exception)",
        Some("public function report(Throwable $exception)"),
        false,
        G7,
    ),
    rule(
        6,
        "Symfony 5 Related Upgrades",
        "High",
        "public function render($request, Exception $exception)",
        Some("public function render($request, Throwable $exception)"),
        false,
        G7,
    ),
    rule(
        7,
        "The retryAfter Method",
        "High",
        "function retryAfter(",
        Some("function backoff("),
        true,
        G8,
    ),
    rule(
        7,
        "The retryAfter Method",
        "High",
        "public $retryAfter",
        Some("public $backoff"),
        true,
        G8,
    ),
    rule(
        7,
        "The timeoutAt Property",
        "High",
        "public $timeoutAt",
        Some("public $retryUntil"),
        false,
        G8,
    ),
    rule(
        7,
        "The allOnQueue() / allOnConnection() Methods",
        "High",
        "->allOnQueue(",
        Some("->onQueue("),
        false,
        G8,
    ),
    rule(
        7,
        "The allOnQueue() / allOnConnection() Methods",
        "High",
        "->allOnConnection(",
        Some("->onConnection("),
        false,
        G8,
    ),
    rule(7, "The elixir Helper", "Low", "elixir(", None, false, G8),
    rule(
        7,
        "The sendNow Method",
        "Low",
        "Mail::sendNow(",
        Some("Mail::send("),
        true,
        G8,
    ),
    rule(
        7,
        "The decodeResponseJson Method",
        "Low",
        "->decodeResponseJson('",
        Some("->json('"),
        false,
        G8,
    ),
    rule(
        7,
        "The Castable Interface",
        "Low",
        "function castUsing()",
        Some("function castUsing(array $arguments)"),
        true,
        G8,
    ),
    rule(
        8,
        "The reduceWithKeys Method",
        "—",
        "->reduceWithKeys(",
        Some("->reduce("),
        true,
        G9,
    ),
    rule(
        8,
        "The reduceMany Method",
        "—",
        "->reduceMany(",
        Some("->reduceSpread("),
        true,
        G9,
    ),
    rule(
        8,
        "Schema Builder registerCustomDoctrineType Method",
        "Low",
        "registerCustomDoctrineType(",
        Some("DB::registerDoctrineType("),
        false,
        G9,
    ),
    rule(
        8,
        "The assertDeleted Method",
        "Medium",
        "->assertDeleted(",
        Some("->assertModelMissing("),
        false,
        G9,
    ),
    rule(
        8,
        "Renamed \"Swift\" Methods",
        "High",
        "->withSwiftMessage(",
        Some("->withSymfonyMessage("),
        false,
        G9,
    ),
    rule(
        8,
        "Renamed \"Swift\" Methods",
        "High",
        "->getSwiftMessage(",
        Some("->getSymfonyMessage("),
        false,
        G9,
    ),
    rule(
        8,
        "Renamed \"Swift\" Methods",
        "High",
        "->getSwiftMailer(",
        Some("->getSymfonyTransport("),
        false,
        G9,
    ),
    rule(
        8,
        "Renamed \"Swift\" Methods",
        "High",
        "->setSwiftMailer(",
        Some("->setSymfonyTransport("),
        false,
        G9,
    ),
    rule(
        8,
        "Trusted Proxies",
        "Low",
        "Fideloper\\Proxy\\TrustProxies",
        Some("Illuminate\\Http\\Middleware\\TrustProxies"),
        true,
        G9,
    ),
    rule(
        8,
        "Trusted Proxies",
        "Low",
        "Request::HEADER_X_FORWARDED_ALL",
        Some(
            "Request::HEADER_X_FORWARDED_FOR | Request::HEADER_X_FORWARDED_HOST | Request::HEADER_X_FORWARDED_PORT | Request::HEADER_X_FORWARDED_PROTO | Request::HEADER_X_FORWARDED_AWS_ELB",
        ),
        false,
        G9,
    ),
    rule(
        8,
        "The opis/closure Library",
        "Low",
        "Illuminate\\Queue\\SerializableClosure",
        None,
        false,
        G9,
    ),
    only(
        rule(
            8,
            "Exception Handler ignore Method",
            "Low",
            "protected function ignore(",
            Some("public function ignore("),
            true,
            G9,
        ),
        When::PathEnds("Exceptions/Handler.php"),
    ),
    rule(
        9,
        "Model \"Dates\" Property",
        "Medium",
        "protected $dates",
        Some("protected $casts"),
        false,
        G10,
    ),
    rule(
        9,
        "The Bus::dispatchNow Method",
        "Low",
        "Bus::dispatchNow(",
        Some("Bus::dispatchSync("),
        true,
        G10,
    ),
    rule(
        9,
        "The Bus::dispatchNow Method",
        "Low",
        "dispatch_now(",
        Some("dispatch_sync("),
        true,
        G10,
    ),
    rule(
        9,
        "The Redirect::home Method",
        "Very Low",
        "Redirect::home(",
        Some("Redirect::route('home')"),
        false,
        G10,
    ),
    rule(
        9,
        "Service Mocking",
        "Medium",
        "->expectsEvents(",
        Some("Event::fake"),
        false,
        G10,
    ),
    rule(
        9,
        "Service Mocking",
        "Medium",
        "->expectsJobs(",
        Some("Bus::fake"),
        false,
        G10,
    ),
    rule(
        9,
        "Service Mocking",
        "Medium",
        "->expectsNotifications(",
        Some("Notification::fake"),
        false,
        G10,
    ),
    rule(
        9,
        "Database Expressions",
        "Medium",
        "(string) DB::raw(",
        Some("->getValue(DB::connection()->getQueryGrammar())"),
        false,
        G10,
    ),
    rule(
        10,
        "Floating-Point Types",
        "High",
        "->unsignedDecimal(",
        Some("->decimal(...)->unsigned()"),
        false,
        G11,
    ),
    rule(
        10,
        "Floating-Point Types",
        "High",
        "->unsignedDouble(",
        Some("->double(...)->unsigned()"),
        false,
        G11,
    ),
    rule(
        10,
        "Floating-Point Types",
        "High",
        "->unsignedFloat(",
        Some("->float(...)->unsigned()"),
        false,
        G11,
    ),
    rule(
        10,
        "Spatial Types",
        "Low",
        "->isGeometry(",
        None,
        false,
        G11,
    ),
    rule(
        10,
        "Doctrine DBAL Removal",
        "Low",
        "getDoctrineSchemaManager(",
        None,
        false,
        G11,
    ),
    rule(
        10,
        "Doctrine DBAL Removal",
        "Low",
        "getDoctrineColumn(",
        None,
        false,
        G11,
    ),
    rule(
        10,
        "Doctrine DBAL Removal",
        "Low",
        "getDoctrineConnection(",
        None,
        false,
        G11,
    ),
    rule(
        10,
        "Doctrine DBAL Removal",
        "Low",
        "registerDoctrineType(",
        None,
        false,
        G11,
    ),
    rule(
        10,
        "Doctrine DBAL Removal",
        "Low",
        "isDoctrineAvailable(",
        None,
        false,
        G11,
    ),
    rule(
        10,
        "Doctrine DBAL Removal",
        "Low",
        "useNativeSchemaOperationsIfPossible(",
        None,
        false,
        G11,
    ),
    rule(
        10,
        "Doctrine DBAL Removal",
        "Low",
        "usingNativeSchemaOperations(",
        None,
        false,
        G11,
    ),
    rule(
        10,
        "Deprecated Schema Methods",
        "Very Low",
        "Schema::getAllTables(",
        Some("Schema::getTables("),
        false,
        G11,
    ),
    rule(
        10,
        "Deprecated Schema Methods",
        "Very Low",
        "Schema::getAllViews(",
        Some("Schema::getViews("),
        false,
        G11,
    ),
    rule(
        10,
        "Deprecated Schema Methods",
        "Very Low",
        "Schema::getAllTypes(",
        Some("Schema::getTypes("),
        false,
        G11,
    ),
    rule(
        10,
        "Per-Second Rate Limiting",
        "Medium",
        "->decayMinutes",
        Some("->decaySeconds"),
        false,
        G11,
    ),
    rule(
        11,
        "Models and UUIDv7",
        "Medium",
        "HasVersion7Uuids",
        Some("HasUuids"),
        true,
        G12,
    ),
    rule(
        11,
        "Database Constructor Signature Changes",
        "Very Low",
        "->withTablePrefix(",
        None,
        false,
        G12,
    ),
    rule(
        12,
        "Request Forgery Protection",
        "High",
        "Illuminate\\Foundation\\Http\\Middleware\\VerifyCsrfToken",
        Some("Illuminate\\Foundation\\Http\\Middleware\\PreventRequestForgery"),
        false,
        G13,
    ),
    rule(
        12,
        "Request Forgery Protection",
        "High",
        "Illuminate\\Foundation\\Http\\Middleware\\ValidateCsrfToken",
        Some("Illuminate\\Foundation\\Http\\Middleware\\PreventRequestForgery"),
        false,
        G13,
    ),
    rule(
        12,
        "JobAttempted Event Exception Payload",
        "Low",
        "->exceptionOccurred",
        Some("->exception"),
        false,
        G13,
    ),
    rule(
        12,
        "Pagination Bootstrap View Names",
        "Low",
        "pagination::default",
        Some("pagination::bootstrap-3"),
        true,
        G13,
    ),
    rule(
        12,
        "Pagination Bootstrap View Names",
        "Low",
        "pagination::simple-default",
        Some("pagination::simple-bootstrap-3"),
        true,
        G13,
    ),
    only(
        rule(
            12,
            "Symfony PHP 8.5 Polyfill and Global Function Conflicts",
            "Low",
            "array_first(",
            Some("Arr::first("),
            false,
            G13,
        ),
        When::Package("laravel/helpers"),
    ),
    only(
        rule(
            12,
            "Symfony PHP 8.5 Polyfill and Global Function Conflicts",
            "Low",
            "array_last(",
            None,
            false,
            G13,
        ),
        When::Package("laravel/helpers"),
    ),
];

const GLOBS: [&str; 7] = [
    "app/**/*.php",
    "routes/**/*.php",
    "config/**/*.php",
    "database/**/*.php",
    "tests/**/*.php",
    "bootstrap/app.php",
    "resources/views/**/*.php",
];

fn word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

pub fn blank_comments(text: &str) -> String {
    let b = text.as_bytes();
    let mut out = b.to_vec();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'\'' | b'"' => {
                let q = b[i];
                i += 1;
                while i < b.len() && b[i] != q {
                    i += if b[i] == b'\\' { 2 } else { 1 };
                }
                i += 1;
            }
            b'/' if b.get(i + 1) == Some(&b'/') => {
                while i < b.len() && b[i] != b'\n' {
                    out[i] = b' ';
                    i += 1;
                }
            }
            b'#' if b.get(i + 1) != Some(&b'[') => {
                while i < b.len() && b[i] != b'\n' {
                    out[i] = b' ';
                    i += 1;
                }
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                while i < b.len() && !(b[i] == b'*' && b.get(i + 1) == Some(&b'/')) {
                    if b[i] != b'\n' {
                        out[i] = b' ';
                    }
                    i += 1;
                }
                if i < b.len() {
                    out[i] = b' ';
                    out[i + 1] = b' ';
                    i += 2;
                }
            }
            _ => i += 1,
        }
    }
    String::from_utf8(out).unwrap_or_else(|_| text.to_owned())
}

pub fn applies(rule: &Rule, rel_path: &str, composer: &str) -> bool {
    match rule.when {
        When::Always => true,
        When::Package(p) => composer.contains(&format!("\"{p}\"")),
        When::PathEnds(s) => rel_path.ends_with(s),
    }
}

pub fn matches(text: &str, pattern: &str) -> Vec<usize> {
    let starts_word = pattern.chars().next().is_some_and(word);
    let ends_word = pattern.chars().last().is_some_and(word);
    text.match_indices(pattern)
        .map(|(at, _)| at)
        .filter(|&at| {
            !starts_word
                || !text[..at]
                    .chars()
                    .last()
                    .is_some_and(|c| word(c) || c == '$')
        })
        .filter(|&at| !ends_word || !text[at + pattern.len()..].chars().next().is_some_and(word))
        .collect()
}

fn shown(code: &str) -> String {
    match code.strip_suffix('(') {
        Some(call) => format!("{call}()"),
        None => code.to_owned(),
    }
}

pub fn major(version: &str) -> Option<u32> {
    version
        .trim_start_matches(['v', '^', '~'])
        .split('.')
        .next()?
        .parse()
        .ok()
}

pub fn findings(
    text: &str,
    path: &str,
    rel_path: &str,
    composer: &str,
    installed: u32,
) -> Vec<Finding> {
    let clean = blank_comments(text);
    let mut out = Vec::new();
    for r in RULES
        .iter()
        .filter(|r| r.from == installed && applies(r, rel_path, composer))
    {
        for at in matches(&clean, r.pattern) {
            let end = at + r.pattern.len();
            let advice = match r.replacement {
                Some(rep) => format!("use {}", shown(rep)),
                None => format!("{} was removed", shown(r.pattern)),
            };
            let fixes = match (r.autofix, r.replacement) {
                (true, Some(rep)) => vec![Fix {
                    title: format!("Replace with {}", shown(rep)),
                    edits: vec![Edit {
                        path: path.to_owned(),
                        start: at,
                        end,
                        text: rep.to_owned(),
                    }],
                }],
                _ => Vec::new(),
            };
            out.push(Finding {
                start: at,
                end,
                message: format!(
                    "Breaks on Laravel {}: {}; {advice} (upgrade guide: {})",
                    r.from + 1,
                    r.title,
                    r.url
                ),
                code: "laravel.upgrade",
                fixes,
                info: true,
            });
        }
    }
    out
}

pub const COLUMNS: [&str; 5] = ["Upgrade", "Change", "Where", "Fix", "Impact"];

pub fn section(project: &Project, root: &str, installed: Option<u32>) -> Table {
    let composer = project.read_file("composer.json").unwrap_or_default();
    let mut rows = Vec::new();
    if let Some(installed) = installed {
        let rules: Vec<&Rule> = RULES.iter().filter(|r| r.from >= installed).collect();
        let mut paths: Vec<String> = GLOBS
            .iter()
            .flat_map(|g| find(project, root, g, 20_000))
            .collect();
        paths.sort();
        paths.dedup();
        for path in paths {
            let rel_path = rel(root, &path).replace('\\', "/");
            let Ok(text) = read_listed(project, &path) else {
                continue;
            };
            let clean = blank_comments(&text);
            for r in rules.iter().filter(|r| applies(r, &rel_path, &composer)) {
                for at in matches(&clean, r.pattern) {
                    let line = u32::try_from(clean[..at].matches('\n').count() + 1).unwrap_or(1);
                    rows.push(Row::at(
                        vec![
                            format!("{} → {}", r.from, r.from + 1),
                            r.title.into(),
                            format!("{rel_path}:{line}"),
                            match r.replacement {
                                Some(rep) => format!("{} → {}", shown(r.pattern), shown(rep)),
                                None => format!("{} was removed", shown(r.pattern)),
                            },
                            r.impact.into(),
                        ],
                        &rel_path,
                        line,
                    ));
                }
            }
        }
    }
    let from = |r: &Row| -> u32 {
        r.cells[0]
            .split_whitespace()
            .next()
            .and_then(|n| n.parse().ok())
            .unwrap_or(0)
    };
    rows.sort_by(|a, b| {
        from(a).cmp(&from(b)).then_with(|| {
            let at = |r: &Row| r.open.as_ref().map(|t| (t.path.clone(), t.line));
            at(a).cmp(&at(b))
        })
    });
    Table {
        columns: COLUMNS.to_vec(),
        rows,
        facets: vec![facet("Upgrade"), facet("Impact"), facet("Change")],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_whole_words_outside_comments() {
        let text = "<?php\n// dispatch_now($job);\nmy_dispatch_now($a);\ndispatch_now($job);\n/* Bus::dispatchNow($x) */\nBus::dispatchNow($job);\nprotected $dates = [];\nprotected $datesFormat = 'x';\n$s = 'dispatch_now(';\n";
        let clean = blank_comments(text);
        assert_eq!(clean.len(), text.len());
        assert_eq!(matches(&clean, "dispatch_now(").len(), 2);
        assert_eq!(matches(&clean, "Bus::dispatchNow(").len(), 1);
        assert_eq!(matches(&clean, "protected $dates").len(), 1);
    }

    #[test]
    fn reports_what_breaks_on_the_next_major_with_safe_fixes() {
        let text = "<?php\nclass Order extends Model\n{\n    protected $dates = ['paid_at'];\n    public function pay() { Bus::dispatchNow(new Charge($this)); }\n    public function old() { return Redirect::home(); }\n    public function later() { $t->unsignedDecimal('x'); }\n}\n";
        let found = findings(
            text,
            "/p/app/Models/Order.php",
            "app/Models/Order.php",
            "{}",
            9,
        );
        let shown: Vec<String> = found
            .iter()
            .map(|f| format!("{}:{}", &text[f.start..f.end], f.fixes.len()))
            .collect();
        assert_eq!(
            shown,
            [
                "protected $dates:0",
                "Bus::dispatchNow(:1",
                "Redirect::home(:0"
            ]
        );
        assert!(found.iter().all(|f| f.info && f.code == "laravel.upgrade"));
        assert!(found[1].message.starts_with(
            "Breaks on Laravel 10: The Bus::dispatchNow Method; use Bus::dispatchSync()"
        ));
        assert_eq!(found[1].fixes[0].edits[0].text, "Bus::dispatchSync(");
        assert!(
            findings(text, "/p/a.php", "app/a.php", "{}", 10)
                .iter()
                .all(|f| text[f.start..f.end].starts_with("->unsignedDecimal"))
        );
    }

    #[test]
    fn conditional_rules_need_their_file_or_package() {
        let handler = "<?php\nprotected function ignore(string $class) {}\n";
        assert_eq!(
            findings(handler, "/p/x.php", "app/Exceptions/Handler.php", "{}", 8).len(),
            1
        );
        assert!(findings(handler, "/p/x.php", "app/Services/Thing.php", "{}", 8).is_empty());
        let helpers = "<?php\narray_first($items);\n";
        assert!(findings(helpers, "/p/x.php", "app/x.php", "{}", 12).is_empty());
        assert_eq!(
            findings(
                helpers,
                "/p/x.php",
                "app/x.php",
                "{\"require\": {\"laravel/helpers\": \"^1.0\"}}",
                12
            )
            .len(),
            1
        );
        assert_eq!(major("v13.40.0"), Some(13));
        assert_eq!(RULES.len(), 63);
    }
}
