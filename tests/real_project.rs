use openidea_plugin::Plugin;
use openidea_plugin::mock::MockHost;
use openidea_plugin::providers::{Document, Providers};
use openidea_plugin::text::position;
use openidea_plugin_laravel_openidea::Laravel;
use std::path::Path;
use std::time::Instant;

fn load(host: &MockHost, root: &Path, dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            load(host, root, &path);
        } else if let Ok(text) = std::fs::read_to_string(&path) {
            let rel = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            host.add_file(&rel, &text);
        }
    }
}

fn complete(plugin: &mut Laravel, call: &str) -> (usize, u128, Vec<String>) {
    let text = format!("<?php\n{call}('');\n");
    let at = position(&text, text.find("('").unwrap() + 2);
    let doc = Document {
        path: "/project/app/Probe.php".into(),
        language: "php".into(),
        version: 1,
        text,
    };
    let started = Instant::now();
    let items = plugin.completion(&doc, at, None);
    let ms = started.elapsed().as_millis();
    (
        items.len(),
        ms,
        items.iter().map(|i| i.label.clone()).collect(),
    )
}

#[test]
#[ignore = "needs LARAVEL_PROJECT pointing at a Laravel application"]
fn indexes_a_real_project() {
    let root = std::env::var("LARAVEL_PROJECT").expect("LARAVEL_PROJECT");
    let root = Path::new(&root);
    let host = MockHost::install();
    for file in [
        "artisan",
        "composer.json",
        "composer.lock",
        ".env.example",
        "bootstrap/app.php",
        "vite.config.js",
    ] {
        if let Ok(text) = std::fs::read_to_string(root.join(file)) {
            host.add_file(file, &text);
        }
    }
    for dir in [
        "config",
        "routes",
        "resources/views",
        "lang",
        "resources/lang",
        "app",
        "database/migrations",
        "database/seeders",
        "resources/js",
        "vendor/laravel/framework",
    ] {
        load(&host, root, &root.join(dir));
    }
    let mut plugin = Laravel::default();
    let started = Instant::now();
    let summary = plugin
        .run_command("refresh-index", &host.invocation(), None)
        .unwrap()
        .unwrap_or_default();
    println!(
        "full index in {} ms: {summary}",
        started.elapsed().as_millis()
    );
    let probe = |plugin: &mut Laravel, text: &str, needle: &str| {
        let at = position(text, text.find(needle).unwrap() + needle.len());
        let doc = Document {
            path: "/project/app/Probe.php".into(),
            language: "php".into(),
            version: 1,
            text: text.into(),
        };
        let started = Instant::now();
        let labels: Vec<String> = plugin
            .completion(&doc, at, None)
            .into_iter()
            .map(|i| i.label)
            .collect();
        (labels, started.elapsed().as_millis())
    };
    let (rules, ms) = probe(
        &mut plugin,
        "<?php\n$request->validate(['a' => '']);\n",
        "=> '",
    );
    println!("validation rules from vendor: {} in {ms} ms", rules.len());
    let (statics, ms) = probe(
        &mut plugin,
        "<?php\nnamespace App\\Http\\Controllers;\n\nclass A\n{\n    public function b()\n    {\n        Cac\n    }\n}\n",
        "        Cac",
    );
    println!(
        "static classes for Cac: {:?} in {ms} ms",
        statics
            .iter()
            .filter(|l| l.ends_with("::"))
            .collect::<Vec<_>>()
    );
    let (query, ms) = probe(&mut plugin, "<?php\nUser::query()->wh\n", "->wh");
    println!(
        "User::query()->wh: {} methods in {ms} ms, e.g. {:?}",
        query.len(),
        &query[..query.len().min(8)]
    );
    let models: Vec<String> = std::env::var("LARAVEL_MODELS")
        .unwrap_or_else(|_| "User".into())
        .split(',')
        .map(str::to_owned)
        .collect();
    for model in &models {
        let text = format!(
            "<?php
{model}::query()->where('');
"
        );
        let at = position(&text, text.find("('").unwrap() + 2);
        let doc = Document {
            path: "/project/app/Probe.php".into(),
            language: "php".into(),
            version: 1,
            text,
        };
        let cols: Vec<String> = plugin
            .completion(&doc, at, None)
            .into_iter()
            .map(|i| i.label)
            .take(6)
            .collect();
        println!("{model} columns: {cols:?}");
    }
    for call in ["config", "route", "view", "env", "__"] {
        let (count, ms, labels) = complete(&mut plugin, call);
        println!(
            "{call}: {count} items, first call {ms} ms, e.g. {:?}",
            &labels[..labels.len().min(5)]
        );
        if call == "route"
            && let Ok(out) = std::env::var("ROUTES_OUT")
        {
            std::fs::write(out, labels.join("\n") + "\n").unwrap();
        }
        let (_, warm, _) = complete(&mut plugin, call);
        println!("{call}: cached call {warm} ms");
    }
    let mut unknown = Vec::new();
    for (path, text) in host_files(root) {
        let doc = Document {
            path: format!("/project/{path}"),
            language: "php".into(),
            version: 1,
            text,
        };
        let markers = plugin.line_markers(&doc);
        if !markers.is_empty() {
            println!("{path}: {} view markers", markers.len());
        }
        unknown.push(path);
    }
    println!("scanned {} PHP files for markers", unknown.len());
    let opened: Vec<openidea_plugin::Event> = unknown
        .iter()
        .map(|p| openidea_plugin::Event::FileOpened(format!("/project/{p}")))
        .collect();
    plugin.handle_events(&opened, &host.invocation());
    let mut codes: std::collections::BTreeMap<String, usize> = Default::default();
    for (path, items) in host.diagnostics() {
        for d in items {
            *codes.entry(d.code.clone().unwrap_or_default()).or_default() += 1;
            if codes[d.code.as_deref().unwrap_or_default()] <= 3 {
                println!("diag {path}:{} {}", d.range.start.line + 1, d.message);
            }
        }
    }
    println!("diagnostics by code: {codes:?}");
    for f in ["vendor/composer/autoload_classmap.php"] {
        if let Ok(text) = std::fs::read_to_string(root.join(f)) {
            host.add_file(f, &text);
            for line in text
                .lines()
                .filter(|l| l.contains("Command' =>") && l.contains("$vendorDir"))
            {
                if let Some(rel) = line
                    .split("$vendorDir . '")
                    .nth(1)
                    .map(|r| r.trim_end_matches("',"))
                {
                    let path = format!("vendor{rel}");
                    if let Ok(src) = std::fs::read_to_string(root.join(&path)) {
                        host.add_file(&path, &src);
                    }
                }
            }
        }
    }
    for entry in std::fs::read_dir(root.join("storage/logs"))
        .into_iter()
        .flatten()
        .flatten()
    {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.ends_with(".log")
            && let Ok(text) = std::fs::read_to_string(entry.path())
        {
            host.add_file(&format!("storage/logs/{name}"), &text);
        }
    }
    use openidea_plugin::views::{ViewEvent, Views};
    let started = Instant::now();
    plugin
        .handle(
            "artisan",
            ViewEvent::Message(r#"{"type":"init"}"#.into()),
            &host.invocation(),
        )
        .unwrap();
    plugin
        .handle(
            "logs",
            ViewEvent::Message(r#"{"type":"files"}"#.into()),
            &host.invocation(),
        )
        .unwrap();
    for (view, message) in host.view_messages() {
        let value: serde_json::Value = serde_json::from_str(&message).unwrap();
        match value["type"].as_str() {
            Some("commands") => {
                let commands = value["commands"].as_array().unwrap();
                let names: Vec<&str> = commands.iter().filter_map(|c| c["name"].as_str()).collect();
                let make_model = commands.iter().find(|c| c["name"] == "make:model");
                let app: Vec<&str> = commands
                    .iter()
                    .filter(|c| {
                        c["source"] != "laravel/framework"
                            && !c["source"].as_str().unwrap_or_default().contains('/')
                    })
                    .filter_map(|c| c["name"].as_str())
                    .collect();
                println!("app commands: {app:?}");
                println!(
                    "{view}: {} commands in {} ms, runner {}, e.g. {:?}",
                    names.len(),
                    started.elapsed().as_millis(),
                    value["runner"],
                    &names[..names.len().min(8)]
                );
                if let Some(m) = make_model {
                    println!(
                        "make:model options: {}",
                        m["options"].as_array().map_or(0, |o| o.len())
                    );
                }
            }
            Some("files") => println!("{view}: files {}", value["files"]),
            _ => {}
        }
    }
    let _ = host.view_messages();
    plugin
        .handle(
            "laravel",
            ViewEvent::Message(r#"{"type":"overview"}"#.into()),
            &host.invocation(),
        )
        .unwrap();
    for id in [
        "routes",
        "models",
        "database",
        "views",
        "components",
        "config",
        "env",
        "translations",
        "middleware",
        "policies",
        "queues",
        "schedule",
        "events",
        "mail",
        "tests",
        "classes",
        "controllers",
        "requests",
        "api-resources",
        "enums",
        "casts",
        "rules",
        "scopes",
        "observers",
        "listeners",
        "providers",
        "bindings",
        "factories",
        "macros",
        "services",
        "frontend",
        "debug-tools",
        "debug-calls",
        "profiles",
        "ecosystem",
    ] {
        let started = Instant::now();
        plugin
            .handle(
                "laravel",
                ViewEvent::Message(format!(r#"{{"type":"section","id":"{id}"}}"#)),
                &host.invocation(),
            )
            .unwrap();
        let ms = started.elapsed().as_millis();
        let (_, last) = host.view_messages().pop().unwrap();
        let value: serde_json::Value = serde_json::from_str(&last).unwrap();
        let rows = value["rows"].as_array().map_or(0, |r| r.len());
        let first = value["rows"][0]["cells"].to_string();
        println!(
            "section {id}: {rows} rows in {ms} ms, facets {}, first {}",
            value["facets"],
            &first[..first.len().min(110)]
        );
    }
    let model_files: Vec<String> = std::env::var("LARAVEL_MODEL_FILES")
        .unwrap_or_else(|_| "app/Models/User.php".into())
        .split(',')
        .map(str::to_owned)
        .collect();
    for model in &model_files {
        if let Ok(text) = std::fs::read_to_string(root.join(model)) {
            let at = text.find("class ").unwrap_or(0);
            let doc = Document {
                path: format!("/project/{model}"),
                language: "php".into(),
                version: 1,
                text: text.clone(),
            };
            use openidea_plugin::providers::Providers as _;
            let actions = plugin.code_actions(
                &doc,
                openidea_plugin::Range::new(position(&text, at), position(&text, at)),
            );
            for a in actions {
                let edit = &a.edits[0].1[0].text;
                println!("action {}: {}", a.title, &edit[..edit.len().min(400)]);
            }
        }
    }
    if let Ok(probe) = std::env::var("LARAVEL_MEMBER_PROBE")
        && let Some((file, needle)) = probe.split_once("::")
        && let Ok(text) = std::fs::read_to_string(root.join(file)).map(|t| {
            match std::env::var("LARAVEL_MEMBER_INSERT") {
                Ok(extra) => t.replacen(needle, &format!("{needle}{extra}"), 1),
                Err(_) => t,
            }
        })
        && let Some(found) = text.find(needle)
    {
        let doc = Document {
            path: format!("/project/{file}"),
            language: "php".into(),
            version: 1,
            text: text.clone(),
        };
        let extra = std::env::var("LARAVEL_MEMBER_INSERT").unwrap_or_default();
        let items = plugin.completion(
            &doc,
            position(&text, found + needle.len() + extra.len()),
            None,
        );
        println!(
            "members after {needle}: {:?}",
            items
                .iter()
                .map(|i| format!("{} ({})", i.label, i.detail.clone().unwrap_or_default()))
                .collect::<Vec<_>>()
        );
    }
    let overview = host
        .view_messages()
        .into_iter()
        .map(|(_, m)| m)
        .find(|m| m.contains("\"overview\""))
        .unwrap_or_default();
    println!("overview: {}", &overview[..overview.len().min(400)]);
}

fn host_files(root: &Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut stack = vec![
        root.join("app"),
        root.join("routes"),
        root.join("resources/views"),
        root.join("bootstrap/cache"),
    ];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "php")
                && let Ok(text) = std::fs::read_to_string(&path)
            {
                out.push((
                    path.strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/"),
                    text,
                ));
            }
        }
    }
    out
}

fn php_files(root: &Path, dir: &Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            php_files(root, &path, out);
        } else if path.extension().is_some_and(|e| e == "php")
            && !path.to_string_lossy().ends_with(".blade.php")
        {
            let rel = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            out.push(format!("/project/{rel}"));
        }
    }
}

#[test]
#[ignore = "needs LARAVEL_PROJECT pointing at a Laravel application"]
fn inspects_a_real_project() {
    let root = std::env::var("LARAVEL_PROJECT").expect("LARAVEL_PROJECT");
    let root = Path::new(&root);
    let host = MockHost::install();
    for file in [
        "artisan",
        "composer.json",
        "composer.lock",
        ".env",
        ".env.example",
    ] {
        if let Ok(text) = std::fs::read_to_string(root.join(file)) {
            host.add_file(file, &text);
        }
    }
    for dir in ["config", "routes", "app", "database/migrations"] {
        load(&host, root, &root.join(dir));
    }
    let mut paths = Vec::new();
    for dir in ["app", "routes"] {
        php_files(root, &root.join(dir), &mut paths);
    }
    paths.push("/project/.env".into());
    if let Ok(probe) = std::env::var("LARAVEL_PROBE") {
        host.add_file("app/OpenideaProbe.php", &probe);
        paths.push("/project/app/OpenideaProbe.php".into());
    }
    let mut plugin = Laravel::default();
    let events: Vec<openidea_plugin::Event> = paths
        .iter()
        .map(|p| openidea_plugin::Event::FileOpened(p.clone()))
        .collect();
    let started = Instant::now();
    plugin.handle_events(&events, &host.invocation());
    println!(
        "inspected {} files in {} ms",
        paths.len(),
        started.elapsed().as_millis()
    );
    for (path, items) in host.diagnostics() {
        for d in items {
            println!(
                "{}:{} {} {}",
                path.trim_start_matches("/project/"),
                d.range.start.line + 1,
                d.code.unwrap_or_default(),
                d.message
            );
        }
    }
}
