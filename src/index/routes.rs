use super::Entry;
use crate::php::{CallKind, Kind, Token, callee_before, tokenize};
use crate::project::{Lines, find};
use openidea_plugin::{Location, Project};

const VERBS: [&str; 9] = [
    "get", "post", "put", "patch", "delete", "options", "any", "match", "view",
];
const RESOURCE: [&str; 7] = [
    "index", "create", "store", "show", "edit", "update", "destroy",
];
const API_RESOURCE: [&str; 5] = ["index", "store", "show", "update", "destroy"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Route {
    pub name: Option<String>,
    pub detail: Option<String>,
    pub start: usize,
    pub end: usize,
    pub uri: Option<(usize, usize)>,
    pub action: Option<(String, String)>,
}

pub fn index(project: &Project, root: &str) -> Vec<Entry> {
    let mut out = Vec::new();
    for path in find(project, root, "routes/**/*.php", 1000) {
        let Ok(text) = project.read_file(&path) else {
            continue;
        };
        let lines = Lines::new(&text);
        out.extend(parse(&text).into_iter().filter_map(|r| {
            Some(Entry {
                key: r.name?,
                detail: r.detail,
                location: Location {
                    path: path.clone(),
                    range: lines.range(&text, r.start, r.end),
                },
            })
        }));
    }
    out.sort_by(|a, b| a.key.cmp(&b.key));
    out
}

pub fn actions(project: &Project, root: &str) -> Vec<Entry> {
    let mut out = Vec::new();
    for path in find(project, root, "routes/**/*.php", 1000) {
        let Ok(text) = project.read_file(&path) else {
            continue;
        };
        let lines = Lines::new(&text);
        for r in parse(&text) {
            let Some((class, method)) = r.action else {
                continue;
            };
            let detail = match (&r.name, &r.detail) {
                (Some(n), Some(d)) => format!("{d} · {n}"),
                (None, Some(d)) => d.clone(),
                (Some(n), None) => n.clone(),
                (None, None) => String::new(),
            };
            out.push(Entry::new(
                format!("{class}@{method}"),
                Some(detail),
                &path,
                lines.range(&text, r.start, r.end),
            ));
        }
    }
    out
}

#[derive(Default)]
struct Chain {
    name: Option<(String, usize, usize)>,
    verb: Option<String>,
    uri: Option<(String, usize, usize)>,
    resource: Option<(bool, String, usize, usize)>,
    only: Vec<String>,
    except: Vec<String>,
    class: Option<String>,
    method: Option<String>,
    prefix: Option<String>,
}

struct Frame {
    method: Option<String>,
    arg: usize,
    bracket: char,
    group: Option<String>,
    uri: Option<String>,
}

fn str_at(tokens: &[Token], i: usize) -> Option<&str> {
    match tokens.get(i).map(|t| &t.kind) {
        Some(Kind::Str { value, .. }) => Some(value),
        _ => None,
    }
}

fn join_uri(prefix: &str, uri: &str) -> String {
    let joined = format!(
        "{}/{}",
        prefix.trim_end_matches('/'),
        uri.trim_start_matches('/')
    );
    let joined = joined.trim_end_matches('/');
    if joined.is_empty() {
        "/".into()
    } else if joined.starts_with('/') {
        joined.to_owned()
    } else {
        format!("/{joined}")
    }
}

fn emit(done: Chain, prefix: &str, uri_prefix: &str, out: &mut Vec<Route>) {
    if let Some((api, name, start, end)) = done.resource {
        let actions = if api {
            &API_RESOURCE[..]
        } else {
            &RESOURCE[..]
        };
        for action in actions {
            let wanted = (done.only.is_empty() || done.only.iter().any(|o| o == action))
                && !done.except.iter().any(|x| x == action);
            if wanted {
                out.push(Route {
                    name: Some(format!("{prefix}{name}.{action}")),
                    detail: Some(format!("resource {}", join_uri(uri_prefix, &name))),
                    start,
                    end,
                    uri: None,
                    action: done.class.clone().map(|c| (c, (*action).to_owned())),
                });
            }
        }
    } else {
        let detail = match (&done.verb, &done.uri) {
            (Some(v), Some((u, ..))) => Some(format!(
                "{} {}",
                v.to_ascii_uppercase(),
                join_uri(uri_prefix, u)
            )),
            (Some(v), None) => Some(v.to_ascii_uppercase()),
            _ => None,
        };
        let action = done.verb.as_ref().and_then(|_| {
            let class = done.class.clone()?;
            Some((
                class,
                done.method.clone().unwrap_or_else(|| "__invoke".into()),
            ))
        });
        let (name, start, end) = match (done.name, &done.uri) {
            (Some((n, s, e)), _) => (Some(format!("{prefix}{n}")), s, e),
            (None, Some((_, s, e))) if action.is_some() => (None, *s, *e),
            _ => return,
        };
        let uri = done.uri.as_ref().map(|(_, s, e)| (*s, *e));
        out.push(Route {
            name,
            detail,
            start,
            end,
            uri,
            action,
        });
    }
}

pub fn parse(text: &str) -> Vec<Route> {
    let tokens = tokenize(text, 0, text.len());
    let mut frames: Vec<Frame> = Vec::new();
    let mut chains: Vec<Chain> = vec![Chain::default()];
    let mut out = Vec::new();
    let prefix = |frames: &[Frame]| {
        frames
            .iter()
            .filter_map(|f| f.group.as_deref())
            .collect::<String>()
    };
    let uri_prefix = |frames: &[Frame]| {
        frames
            .iter()
            .filter_map(|f| f.uri.as_deref())
            .fold(String::new(), |acc, p| join_uri(&acc, p))
    };
    for (i, t) in tokens.iter().enumerate() {
        let chain = chains.last_mut().expect("root chain");
        match &t.kind {
            Kind::Open('{') => {
                frames.push(Frame {
                    method: None,
                    arg: 0,
                    bracket: '{',
                    group: None,
                    uri: None,
                });
                chains.push(Chain::default());
            }
            Kind::Close('}') => {
                frames.pop();
                if chains.len() > 1 {
                    chains.pop();
                }
            }
            Kind::Open(c) => {
                let method = if *c == '(' {
                    callee_before(&tokens, i)
                        .filter(|c| c.kind != CallKind::Function)
                        .map(|c| c.name)
                } else {
                    None
                };
                let is_group = method.as_deref() == Some("group");
                let group =
                    is_group.then(|| chain.name.take().map(|(n, ..)| n).unwrap_or_default());
                let uri = if is_group { chain.prefix.take() } else { None };
                frames.push(Frame {
                    method,
                    arg: 0,
                    bracket: *c,
                    group,
                    uri,
                });
            }
            Kind::Close(_) => {
                if frames.last().is_some_and(|f| f.group.is_some()) {
                    emit(
                        std::mem::take(chain),
                        &prefix(&frames),
                        &uri_prefix(&frames),
                        &mut out,
                    );
                }
                frames.pop();
            }
            Kind::Comma => {
                if let Some(f) = frames.last_mut() {
                    f.arg += 1;
                }
            }
            Kind::Semicolon => emit(
                std::mem::take(chain),
                &prefix(&frames),
                &uri_prefix(&frames),
                &mut out,
            ),
            Kind::Ident(class)
                if class == "class"
                    && i >= 2
                    && tokens[i - 1].kind == Kind::DoubleColon
                    && let Kind::Ident(name) = &tokens[i - 2].kind =>
            {
                let n = frames.len();
                let frame = match n {
                    0 => None,
                    _ if frames[n - 1].bracket == '(' => Some(&frames[n - 1]),
                    _ if n >= 2 && frames[n - 1].bracket == '[' => Some(&frames[n - 2]),
                    _ => None,
                };
                let routed = frame.is_some_and(|f| {
                    f.arg == 1
                        && f.method.as_deref().is_some_and(|m| {
                            VERBS.contains(&m) || m == "resource" || m == "apiresource"
                        })
                        || f.arg == 0 && f.method.as_deref() == Some("controller")
                });
                if routed {
                    chain.class = Some(name.rsplit('\\').next().unwrap_or(name).to_owned());
                }
            }
            Kind::Str { value, .. } => {
                if str_at(&tokens, i).is_some()
                    && tokens
                        .get(i + 1)
                        .is_some_and(|n| n.kind == Kind::DoubleArrow)
                {
                    if (value == "as" || value == "prefix")
                        && let Some(v) = str_at(&tokens, i + 2)
                    {
                        let n = frames.len();
                        let in_group =
                            n >= 2 && frames[n - 1].bracket == '[' && frames[n - 2].group.is_some();
                        if in_group && value == "as" {
                            frames[n - 2].group.get_or_insert_default().push_str(v);
                        } else if in_group {
                            frames[n - 2].uri = Some(v.to_owned());
                        } else if value == "as" {
                            chain.name =
                                Some((v.to_owned(), tokens[i + 2].start, tokens[i + 2].end));
                        }
                    }
                    continue;
                }
                let n = frames.len();
                let (frame, in_array) = match n {
                    0 => continue,
                    _ if frames[n - 1].bracket == '(' => (&frames[n - 1], false),
                    _ if n >= 2 && frames[n - 1].bracket == '[' && frames[n - 2].bracket == '(' => {
                        (&frames[n - 2], true)
                    }
                    _ => continue,
                };
                let Some(method) = frame.method.as_deref() else {
                    continue;
                };
                match (method, frame.arg, in_array) {
                    ("name", 0, false) => {
                        if frames.len() == 1
                            || frames[..n - 1]
                                .iter()
                                .all(|f| f.bracket != '(' || f.group.is_some())
                        {
                            chain.name = Some((value.clone(), t.start, t.end));
                        }
                    }
                    ("resource", 0, false) => {
                        chain.resource = Some((false, value.clone(), t.start, t.end))
                    }
                    ("apiresource", 0, false) => {
                        chain.resource = Some((true, value.clone(), t.start, t.end))
                    }
                    ("only", _, _) => chain.only.push(value.clone()),
                    ("except", _, _) => chain.except.push(value.clone()),
                    ("match", 1, false) => chain.uri = Some((value.clone(), t.start, t.end)),
                    ("prefix", 0, false) => chain.prefix = Some(value.clone()),
                    (verb, 1, true) if VERBS.contains(&verb) && chain.class.is_some() => {
                        chain.method = Some(value.clone())
                    }
                    ("match", 0, true) => {
                        let verb = chain
                            .verb
                            .take()
                            .map_or(value.clone(), |v| format!("{v}|{value}"));
                        chain.verb = Some(verb);
                    }
                    (verb, 0, false) if VERBS.contains(&verb) && verb != "match" => {
                        chain.verb = Some(verb.to_owned());
                        chain.uri = Some((value.clone(), t.start, t.end));
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(text: &str) -> Vec<(String, Option<String>)> {
        parse(text)
            .into_iter()
            .filter_map(|r| Some((r.name?, r.detail)))
            .collect()
    }

    #[test]
    fn names_routes_with_group_prefixes() {
        let text = r#"<?php
use Illuminate\Support\Facades\Route;

Route::get('/', function () {
    return view('welcome');
})->name('home');

Route::middleware('auth')->name('admin.')->prefix('admin')->group(function () {
    Route::post('/users', [UserController::class, 'store'])->name('users.store');
    Route::name('reports.')->group(function () {
        Route::get('/daily', DailyController::class)->name('daily');
    });
});

Route::group(['as' => 'api.', 'prefix' => 'api'], function () {
    Route::match(['get', 'post'], '/ping', PingController::class)->name('ping');
});

Route::view('/about', 'about')->name('about');
"#;
        assert_eq!(
            names(text),
            [
                ("home".into(), Some("GET /".into())),
                ("admin.users.store".into(), Some("POST /admin/users".into())),
                (
                    "admin.reports.daily".into(),
                    Some("GET /admin/daily".into())
                ),
                ("api.ping".into(), Some("GET|POST /api/ping".into())),
                ("about".into(), Some("VIEW /about".into())),
            ]
        );
    }

    #[test]
    fn records_controller_actions() {
        let text = "<?php\nRoute::prefix('v1')->group(function () {\n    Route::get('/users', [UserController::class, 'index']);\n    Route::post('/ping', PingController::class)->name('ping');\n});\nRoute::resource('photos', PhotoController::class)->only(['show']);\n";
        let got: Vec<_> = parse(text)
            .into_iter()
            .map(|r| (r.action, r.name, r.detail))
            .collect();
        assert_eq!(
            got,
            [
                (
                    Some(("UserController".into(), "index".into())),
                    None,
                    Some("GET /v1/users".into())
                ),
                (
                    Some(("PingController".into(), "__invoke".into())),
                    Some("ping".into()),
                    Some("POST /v1/ping".into())
                ),
                (
                    Some(("PhotoController".into(), "show".into())),
                    Some("photos.show".into()),
                    Some("resource /photos".into())
                ),
            ]
        );
    }

    #[test]
    fn expands_resources() {
        let text = "<?php\nRoute::resource('photos', PhotoController::class)->only(['index', 'show']);\nRoute::name('v1.')->group(fn () => Route::apiResource('posts', PostController::class)->except('destroy'));\n";
        let got: Vec<_> = names(text).into_iter().map(|(n, _)| n).collect();
        assert_eq!(
            got,
            [
                "photos.index",
                "photos.show",
                "v1.posts.index",
                "v1.posts.store",
                "v1.posts.show",
                "v1.posts.update"
            ]
        );
    }
}
