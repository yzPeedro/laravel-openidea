use crate::php::{Kind, string_args, tokenize};
use crate::project::find;
use openidea_plugin::Project;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Argument {
    pub name: String,
    pub required: bool,
    pub array: bool,
    pub default: Option<String>,
    pub description: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Opt {
    pub name: String,
    pub shortcut: Option<String>,
    pub value: bool,
    pub array: bool,
    pub default: Option<String>,
    pub description: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Command {
    pub name: String,
    pub description: String,
    pub arguments: Vec<Argument>,
    pub options: Vec<Opt>,
    pub source: String,
    pub file: String,
}

fn split_description(part: &str) -> (&str, String) {
    match part.split_once(" : ") {
        Some((head, desc)) => (head.trim(), desc.trim().to_owned()),
        None => (part.trim(), String::new()),
    }
}

pub fn parse_signature(signature: &str) -> Command {
    let name = signature
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_owned();
    let mut command = Command {
        name,
        ..Command::default()
    };
    let mut rest = signature;
    while let Some(open) = rest.find('{') {
        let Some(close) = rest[open..].find('}') else {
            break;
        };
        let (head, description) = split_description(&rest[open + 1..open + close]);
        rest = &rest[open + close + 1..];
        if let Some(option) = head.strip_prefix("--") {
            let (spec, default) = match option.split_once('=') {
                Some((s, d)) => (s, Some(d)),
                None => (option, None),
            };
            let (shortcut, name) = match spec.split_once('|') {
                Some((s, n)) => (Some(s.trim().to_owned()), n.trim()),
                None => (None, spec.trim()),
            };
            let array = default == Some("*") || name.ends_with('*');
            command.options.push(Opt {
                name: name.trim_end_matches('*').to_owned(),
                shortcut,
                value: default.is_some(),
                array,
                default: default
                    .filter(|d| !d.is_empty() && *d != "*")
                    .map(str::to_owned),
                description,
            });
        } else {
            let (spec, default) = match head.split_once('=') {
                Some((s, d)) => (s, Some(d.to_owned())),
                None => (head, None),
            };
            let optional = spec.contains('?') || default.is_some();
            command.arguments.push(Argument {
                name: spec
                    .trim_end_matches(['?', '*'])
                    .trim_end_matches('?')
                    .to_owned(),
                required: !optional,
                array: spec.ends_with('*'),
                default: default.filter(|d| !d.is_empty()),
                description,
            });
        }
    }
    command
}

#[derive(Debug, PartialEq)]
enum Element {
    Str(String),
    Null,
    Mode(String),
}

fn definition_arrays(text: &str, function: &str) -> Vec<Vec<Element>> {
    let tokens = tokenize(text, 0, text.len());
    let Some(at) = tokens.windows(2).position(|w| {
        matches!(&w[0].kind, Kind::Ident(f) if f.eq_ignore_ascii_case("function"))
            && matches!(&w[1].kind, Kind::Ident(n) if n.eq_ignore_ascii_case(function))
    }) else {
        return Vec::new();
    };
    let Some(body) = tokens[at..]
        .iter()
        .position(|t| t.kind == Kind::Open('{'))
        .map(|p| at + p)
    else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut brackets = 0i32;
    let mut current: Option<Vec<Element>> = None;
    for t in &tokens[body..] {
        match &t.kind {
            Kind::Open('{') => depth += 1,
            Kind::Close('}') => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            Kind::Open('[') => {
                brackets += 1;
                if brackets == 2 {
                    current = Some(Vec::new());
                }
            }
            Kind::Close(']') => {
                if brackets == 2
                    && let Some(row) = current.take()
                {
                    out.push(row);
                }
                brackets -= 1;
            }
            Kind::Str { value, .. } if brackets == 2 => current
                .get_or_insert_default()
                .push(Element::Str(value.clone())),
            Kind::Ident(i) if brackets == 2 && i.eq_ignore_ascii_case("null") => {
                current.get_or_insert_default().push(Element::Null)
            }
            Kind::Ident(i)
                if brackets == 2
                    && (i.contains("VALUE_")
                        || i.contains("REQUIRED")
                        || i.contains("OPTIONAL")
                        || i.contains("IS_ARRAY")
                        || i.contains("NONE")) =>
            {
                current
                    .get_or_insert_default()
                    .push(Element::Mode(i.clone()))
            }
            _ => {}
        }
    }
    out
}

fn text_of(e: Option<&Element>) -> Option<String> {
    match e {
        Some(Element::Str(s)) => Some(s.clone()),
        _ => None,
    }
}

fn mode_of(row: &[Element]) -> String {
    row.iter()
        .filter_map(|e| {
            if let Element::Mode(m) = e {
                Some(m.as_str())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("|")
}

pub fn parse_command_file(text: &str) -> Option<Command> {
    let tokens = tokenize(text, 0, text.len());
    let assigned = |var: &str| {
        tokens
            .windows(3)
            .find_map(|w| match (&w[0].kind, &w[1].kind, &w[2].kind) {
                (Kind::Var(v), Kind::Assign, Kind::Str { value, .. }) if v == var => {
                    Some(value.clone())
                }
                _ => None,
            })
    };
    let attribute = |param: &str| {
        let at = text.find("AsCommand(")?;
        let args = string_args(&tokenize(text, at, text.len()));
        args.into_iter()
            .find(|a| a.callee.name == "ascommand" && a.param.as_deref() == Some(param))
            .map(|a| a.value)
    };
    let class_attribute = |name: &str| {
        string_args(&tokens)
            .into_iter()
            .find(|a| {
                a.callee.kind == crate::php::CallKind::Attribute
                    && a.callee.name == name
                    && a.arg == 0
                    && !a.in_array
            })
            .map(|a| a.value)
    };
    let mut command =
        if let Some(signature) = assigned("signature").or_else(|| class_attribute("signature")) {
            let mut command = parse_signature(&signature);
            command.description = class_attribute("description").unwrap_or_default();
            command
        } else {
            let name = assigned("name").or_else(|| attribute("name"))?;
            let mut command = Command {
                name,
                ..Command::default()
            };
            for row in definition_arrays(text, "getArguments") {
                let mode = mode_of(&row);
                let strings: Vec<String> = row.iter().filter_map(|e| text_of(Some(e))).collect();
                let Some(name) = strings.first().cloned() else {
                    continue;
                };
                command.arguments.push(Argument {
                    name,
                    required: mode.contains("REQUIRED"),
                    array: mode.contains("IS_ARRAY"),
                    default: None,
                    description: strings.get(1).cloned().unwrap_or_default(),
                });
            }
            for row in definition_arrays(text, "getOptions") {
                let mode = mode_of(&row);
                let Some(name) = text_of(row.first()) else {
                    continue;
                };
                let shortcut = text_of(row.get(1)).filter(|s| s.len() <= 3 && !s.contains(' '));
                let description = row
                    .iter()
                    .skip(if shortcut.is_some() { 2 } else { 1 })
                    .find_map(|e| text_of(Some(e)))
                    .unwrap_or_default();
                command.options.push(Opt {
                    name,
                    shortcut,
                    value: mode.contains("REQUIRED") || mode.contains("OPTIONAL"),
                    array: mode.contains("IS_ARRAY"),
                    default: None,
                    description,
                });
            }
            command
        };
    if command.name.is_empty() {
        return None;
    }
    if command.description.is_empty() {
        command.description = assigned("description")
            .or_else(|| attribute("description"))
            .unwrap_or_default();
    }
    Some(command)
}

fn console_routes(text: &str) -> Vec<Command> {
    let args = string_args(&tokenize(text, 0, text.len()));
    let mut out: Vec<Command> = Vec::new();
    for a in &args {
        if a.callee.class.as_deref() == Some("artisan") && a.callee.name == "command" && a.arg == 0
        {
            out.push(parse_signature(&a.value));
        } else if a.callee.name == "purpose"
            && let Some(last) = out.last_mut()
            && last.description.is_empty()
        {
            last.description = a.value.clone();
        }
    }
    out
}

fn vendor_command_paths(classmap: &str) -> Vec<(String, String)> {
    classmap
        .lines()
        .filter_map(|line| {
            let (class, path) = line.split_once("=>")?;
            let class = class.trim().trim_matches(['\'', ',']);
            if !class.ends_with("Command") {
                return None;
            }
            let path = path.trim().trim_end_matches(',');
            let rel = path.split_once("$vendorDir . '")?.1.trim_end_matches('\'');
            let package: Vec<&str> = rel.trim_start_matches('/').splitn(3, '/').collect();
            let package = format!("{}/{}", package.first()?, package.get(1)?);
            Some((format!("vendor{rel}"), package))
        })
        .collect()
}

pub const GLOBAL_OPTIONS: &[(&str, &str)] = &[
    ("env", "The environment the command should run under"),
    ("no-interaction", "Do not ask any interactive question"),
    ("verbose", "Increase the verbosity of messages"),
    ("quiet", "Do not output any message"),
    ("ansi", "Force ANSI output"),
];

pub fn discover(project: &Project, root: &str) -> Vec<Command> {
    let mut by_name: BTreeMap<String, Command> = BTreeMap::new();
    if let Ok(classmap) = project.read_file("vendor/composer/autoload_classmap.php") {
        for (path, package) in vendor_command_paths(&classmap) {
            let Ok(text) = project.read_file(&path) else {
                continue;
            };
            if text.contains("abstract class") {
                continue;
            }
            if let Some(mut command) = parse_command_file(&text) {
                command.source = package;
                command.file = path.clone();
                by_name.entry(command.name.clone()).or_insert(command);
            }
        }
    }
    for path in find(project, root, "app/**/*.php", 20_000) {
        let Ok(text) = project.read_file(&path) else {
            continue;
        };
        let command_class = crate::index::parse_class(&text)
            .and_then(|c| c.extends)
            .is_some_and(|e| e.ends_with("Command"));
        if !command_class && !text.contains("AsCommand(") && !text.contains("#[Signature(") {
            continue;
        }
        if let Some(mut command) = parse_command_file(&text) {
            command.source = "app".into();
            command.file = crate::project::rel(root, &path).to_owned();
            by_name.insert(command.name.clone(), command);
        }
    }
    if let Ok(text) = project.read_file("routes/console.php") {
        for mut command in console_routes(&text) {
            command.source = "routes/console.php".into();
            command.file = "routes/console.php".into();
            by_name.insert(command.name.clone(), command);
        }
    }
    by_name.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_signatures() {
        let c = parse_signature(
            "mail:send {user : The user id} {queue?} {ids*} {--Q|queue= : Queue name} {--force} {--tag=*} {--delay=5}",
        );
        assert_eq!(c.name, "mail:send");
        let args: Vec<_> = c
            .arguments
            .iter()
            .map(|a| (a.name.as_str(), a.required, a.array))
            .collect();
        assert_eq!(
            args,
            [
                ("user", true, false),
                ("queue", false, false),
                ("ids", true, true)
            ]
        );
        assert_eq!(c.arguments[0].description, "The user id");
        let opts: Vec<_> = c
            .options
            .iter()
            .map(|o| {
                (
                    o.name.as_str(),
                    o.shortcut.as_deref(),
                    o.value,
                    o.array,
                    o.default.as_deref(),
                )
            })
            .collect();
        assert_eq!(
            opts,
            [
                ("queue", Some("Q"), true, false, None),
                ("force", None, false, false, None),
                ("tag", None, true, true, None),
                ("delay", None, true, false, Some("5"))
            ]
        );
    }

    #[test]
    fn parses_classic_and_attribute_commands() {
        let classic = "<?php\nclass ModelMakeCommand extends GeneratorCommand {\n protected $name = 'make:model';\n protected $description = 'Create a new Eloquent model class';\n protected function getOptions()\n {\n  return [\n   ['migration', 'm', InputOption::VALUE_NONE, 'Create a new migration file for the model'],\n   ['table', null, InputOption::VALUE_REQUIRED, 'The table name'],\n  ];\n }\n protected function getArguments() { return [['name', InputArgument::REQUIRED, 'The name of the model']]; }\n}\n";
        let c = parse_command_file(classic).unwrap();
        assert_eq!(
            (c.name.as_str(), c.description.as_str()),
            ("make:model", "Create a new Eloquent model class")
        );
        assert_eq!(c.options[0].shortcut.as_deref(), Some("m"));
        assert!(!c.options[0].value);
        assert!(c.options[1].value);
        assert_eq!(c.options[1].description, "The table name");
        assert_eq!(
            (c.arguments[0].name.as_str(), c.arguments[0].required),
            ("name", true)
        );
        let attr = "<?php\n#[AsCommand(name: 'about', description: 'Display basic information')]\nclass AboutCommand extends Command { protected $signature = 'about {--only=} {--json}'; }\n";
        let c = parse_command_file(attr).unwrap();
        assert_eq!(
            (c.name.as_str(), c.description.as_str(), c.options.len()),
            ("about", "Display basic information", 2)
        );
        let routes = "<?php\nArtisan::command('inspire', function () {})->purpose('Display an inspiring quote');\n";
        assert_eq!(
            console_routes(routes)[0].description,
            "Display an inspiring quote"
        );
        let map = "    'Illuminate\\\\Foundation\\\\Console\\\\ModelMakeCommand' => $vendorDir . '/laravel/framework/src/Illuminate/Foundation/Console/ModelMakeCommand.php',\n";
        assert_eq!(
            vendor_command_paths(map),
            [(
                "vendor/laravel/framework/src/Illuminate/Foundation/Console/ModelMakeCommand.php"
                    .to_owned(),
                "laravel/framework".to_owned()
            )]
        );
    }
}
