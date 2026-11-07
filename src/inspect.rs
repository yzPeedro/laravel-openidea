use crate::index::parse_class;
use crate::php::{Kind, Token, tokenize};
use crate::project::{Lines, find, rel};
use openidea_plugin::Project;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub cells: Vec<String>,
    pub path: String,
    pub line: u32,
}

fn matching(tokens: &[Token], open: usize) -> usize {
    let mut depth = 0usize;
    for (j, t) in tokens.iter().enumerate().skip(open) {
        match t.kind {
            Kind::Open(_) => depth += 1,
            Kind::Close(_) => {
                depth -= 1;
                if depth == 0 {
                    return j;
                }
            }
            _ => {}
        }
    }
    tokens.len().saturating_sub(1)
}

fn ident(tokens: &[Token], i: usize) -> Option<&str> {
    match &tokens.get(i)?.kind {
        Kind::Ident(s) => Some(s),
        _ => None,
    }
}

fn short(name: &str) -> &str {
    name.rsplit('\\').next().unwrap_or(name)
}

fn first_string(tokens: &[Token], from: usize, to: usize) -> Option<String> {
    tokens[from..to].iter().find_map(|t| match &t.kind {
        Kind::Str { value, .. } => Some(value.clone()),
        _ => None,
    })
}

fn task_label(tokens: &[Token], kind: &str, open: usize, close: usize) -> String {
    if kind == "call" {
        return "closure".into();
    }
    if let Some(s) = first_string(tokens, open, close) {
        return s;
    }
    for j in open..close {
        if ident(tokens, j).is_some_and(|n| n.eq_ignore_ascii_case("new"))
            && let Some(class) = ident(tokens, j + 1)
        {
            return short(class).to_owned();
        }
        if tokens[j].kind == Kind::DoubleColon && ident(tokens, j + 1) == Some("class") {
            return ident(tokens, j - 1)
                .map(short)
                .unwrap_or_default()
                .to_owned();
        }
    }
    kind.to_owned()
}

pub fn schedule(text: &str) -> Vec<(String, String, usize)> {
    let tokens = tokenize(text, 0, text.len());
    let mut out = Vec::new();
    let mut i = 0;
    while i + 1 < tokens.len() {
        let Some(name) = ident(&tokens, i) else {
            i += 1;
            continue;
        };
        let kind = name.to_ascii_lowercase();
        let starts = matches!(kind.as_str(), "command" | "job" | "call" | "exec")
            && tokens[i + 1].kind == Kind::Open('(')
            && i >= 2
            && match (&tokens[i - 1].kind, &tokens[i - 2].kind) {
                (Kind::DoubleColon, Kind::Ident(c)) => short(c) == "Schedule",
                (Kind::ObjectOp, Kind::Var(v)) => v == "schedule",
                _ => false,
            };
        if !starts {
            i += 1;
            continue;
        }
        let close = matching(&tokens, i + 1);
        let label = task_label(&tokens, &kind, i + 1, close);
        let mut parts = Vec::new();
        let mut j = close + 1;
        while j + 2 < tokens.len() && tokens[j].kind == Kind::ObjectOp {
            let Some(method) = ident(&tokens, j + 1) else {
                break;
            };
            if tokens[j + 2].kind != Kind::Open('(') {
                break;
            }
            let end = matching(&tokens, j + 2);
            let arg = first_string(&tokens, j + 2, end)
                .or_else(|| {
                    tokens[j + 2..end].iter().find_map(|t| match &t.kind {
                        Kind::Num(n) => Some(n.clone()),
                        _ => None,
                    })
                })
                .map(|a| format!("({a})"))
                .unwrap_or_default();
            parts.push(format!("{method}{arg}"));
            j = end + 1;
        }
        out.push((label, parts.join(" · "), tokens[i].start));
        i = j.max(i + 1);
    }
    out
}

pub fn scheduled_tasks(project: &Project) -> Vec<Found> {
    let mut out = Vec::new();
    for file in [
        "routes/console.php",
        "app/Console/Kernel.php",
        "bootstrap/app.php",
    ] {
        let Ok(text) = project.read_file(file) else {
            continue;
        };
        let lines = Lines::new(&text);
        for (task, frequency, at) in schedule(&text) {
            out.push(Found {
                cells: vec![
                    task,
                    if frequency.is_empty() {
                        "—".into()
                    } else {
                        frequency
                    },
                    file.into(),
                ],
                path: file.into(),
                line: lines.position(&text, at).line + 1,
            });
        }
    }
    out
}

fn property(tokens: &[Token], name: &str) -> Option<String> {
    tokens
        .windows(3)
        .find_map(|w| match (&w[0].kind, &w[1].kind, &w[2].kind) {
            (Kind::Var(v), Kind::Assign, Kind::Str { value, .. }) if v == name => {
                Some(value.clone())
            }
            (Kind::Var(v), Kind::Assign, Kind::Num(n)) if v == name => Some(n.clone()),
            _ => None,
        })
}

fn classes(
    project: &Project,
    root: &str,
    glob: &str,
    keep: impl Fn(&str) -> bool,
) -> Vec<(String, String, crate::index::Class)> {
    let mut paths = find(project, root, glob, 20_000);
    paths.sort();
    paths
        .into_iter()
        .filter_map(|p| {
            let text = crate::project::read_listed(project, &p).ok()?;
            if !keep(&text) {
                return None;
            }
            let class = parse_class(&text)?;
            Some((rel(root, &p).to_owned(), text, class))
        })
        .collect()
}

fn class_line(text: &str, class: &crate::index::Class) -> u32 {
    Lines::new(text).position(text, class.start).line + 1
}

pub fn jobs(project: &Project, root: &str) -> Vec<Found> {
    classes(project, root, "app/**/*.php", |t| t.contains("ShouldQueue"))
        .into_iter()
        .filter(|(_, text, _)| text.contains("implements") && text.contains("ShouldQueue"))
        .map(|(path, text, class)| {
            let tokens = tokenize(&text, 0, text.len());
            let get = |n: &str| property(&tokens, n).unwrap_or_else(|| "—".into());
            let kind = if text.contains("extends Mailable") {
                "mail"
            } else if text.contains("extends Notification") {
                "notification"
            } else if path.contains("/Listeners/") {
                "listener"
            } else {
                "job"
            };
            Found {
                cells: vec![
                    class.name.clone(),
                    kind.into(),
                    get("queue"),
                    get("tries"),
                    get("timeout"),
                    path.clone(),
                ],
                line: class_line(&text, &class),
                path,
            }
        })
        .collect()
}

pub fn messages(project: &Project, root: &str) -> Vec<Found> {
    classes(project, root, "app/**/*.php", |t| {
        t.contains("extends Mailable") || t.contains("extends Notification")
    })
    .into_iter()
    .map(|(path, text, class)| {
        let kind = if text.contains("extends Mailable") {
            "mailable"
        } else {
            "notification"
        };
        let channels = if kind == "notification" {
            let via = text.find("function via").map(|i| &text[i..]).and_then(|t| {
                t.find('[')
                    .zip(t.find(']'))
                    .map(|(a, b)| t[a + 1..b].to_owned())
            });
            via.unwrap_or_default()
                .replace(['\'', '"'], "")
                .replace(' ', "")
        } else {
            String::new()
        };
        Found {
            cells: vec![class.name.clone(), kind.into(), channels, path.clone()],
            line: class_line(&text, &class),
            path,
        }
    })
    .collect()
}

pub fn tests(project: &Project, root: &str) -> Vec<Found> {
    let mut paths = find(project, root, "tests/**/*.php", 20_000);
    paths.sort();
    paths
        .into_iter()
        .filter_map(|p| {
            let text = crate::project::read_listed(project, &p).ok()?;
            let count = text.matches("public function test").count()
                + text.matches("#[Test]").count()
                + text.matches("\ntest(").count()
                + text.matches("\nit(").count();
            if count == 0 {
                return None;
            }
            let path = rel(root, &p).to_owned();
            let suite = path.split('/').nth(1).unwrap_or_default().to_owned();
            Some(Found {
                cells: vec![path.clone(), suite, count.to_string()],
                path,
                line: 1,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_scheduled_tasks() {
        let text = "<?php\nSchedule::command('telemetry:aggregate')->dailyAt('01:10')->withoutOverlapping()->onOneServer();\nSchedule::job(new PruneJob)->hourly();\n$schedule->call(fn () => 1)->everyFiveMinutes();\nSchedule::command(PurgeCommand::class)->weekly();\n";
        let tasks: Vec<_> = schedule(text).into_iter().map(|(t, f, _)| (t, f)).collect();
        assert_eq!(
            tasks,
            [
                (
                    "telemetry:aggregate".to_owned(),
                    "dailyAt(01:10) · withoutOverlapping · onOneServer".to_owned()
                ),
                ("PruneJob".to_owned(), "hourly".to_owned()),
                ("closure".to_owned(), "everyFiveMinutes".to_owned()),
                ("PurgeCommand".to_owned(), "weekly".to_owned()),
            ]
        );
    }
}
