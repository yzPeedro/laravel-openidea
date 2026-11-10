use openidea_plugin::Plugin;
use openidea_plugin::mock::MockHost;
use openidea_plugin::views::{ViewEvent, Views};
use openidea_plugin_laravel_openidea::Laravel;
use serde_json::{Map, Value, json};
use std::path::Path;

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

#[test]
#[ignore = "needs LARAVEL_PROJECT and HUB_DUMP"]
fn dumps_hub_responses() {
    let root = std::env::var("LARAVEL_PROJECT").expect("LARAVEL_PROJECT");
    let out = std::env::var("HUB_DUMP").expect("HUB_DUMP");
    let root = Path::new(&root);
    let host = MockHost::install();
    for file in [
        "artisan",
        "composer.json",
        "composer.lock",
        ".env.example",
        "bootstrap/app.php",
        "bootstrap/providers.php",
        "bootstrap/cache/routes-v7.php",
        "vite.config.js",
        "package.json",
        "vendor/composer/autoload_classmap.php",
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
        "database",
        "resources/js",
        "tests",
        "storage/logs",
    ] {
        load(&host, root, &root.join(dir));
    }
    let mut plugin = Laravel::default();
    plugin
        .run_command("refresh-index", &host.invocation(), None)
        .unwrap();
    let ctx = host.invocation();
    let send = |plugin: &mut Laravel, msg: Value| -> Vec<Value> {
        let _ = host.view_messages();
        plugin
            .handle("laravel", ViewEvent::Message(msg.to_string()), &ctx)
            .unwrap();
        host.view_messages()
            .into_iter()
            .filter_map(|(_, m)| serde_json::from_str(&m).ok())
            .collect()
    };
    let mut dump = Map::new();
    dump.insert(
        "overview".into(),
        send(&mut plugin, json!({"type": "overview"}))
            .pop()
            .unwrap_or_default(),
    );
    dump.insert(
        "commands".into(),
        send(&mut plugin, json!({"type": "init"}))
            .pop()
            .unwrap_or_default(),
    );
    let files = send(&mut plugin, json!({"type": "files"}))
        .pop()
        .unwrap_or_default();
    let mut entries = Map::new();
    for f in files["files"].as_array().cloned().unwrap_or_default() {
        let path = f["path"].as_str().unwrap_or_default().to_owned();
        if let Some(m) = send(
            &mut plugin,
            json!({"type": "tail", "path": path, "size": -1}),
        )
        .pop()
        {
            entries.insert(path, m);
        }
    }
    dump.insert("files".into(), files);
    dump.insert("entries".into(), Value::Object(entries));
    let mut sections = Map::new();
    for id in [
        "routes",
        "controllers",
        "requests",
        "api-resources",
        "middleware",
        "models",
        "scopes",
        "observers",
        "casts",
        "enums",
        "database",
        "security",
        "unused",
        "upgrade",
        "factories",
        "views",
        "components",
        "frontend",
        "translations",
        "classes",
        "providers",
        "bindings",
        "rules",
        "macros",
        "config",
        "env",
        "services",
        "policies",
        "queues",
        "schedule",
        "events",
        "listeners",
        "mail",
        "tests",
        "profiles",
        "debug-calls",
        "debug-tools",
        "ecosystem",
    ] {
        let msg = send(&mut plugin, json!({"type": "section", "id": id}))
            .pop()
            .unwrap_or_default();
        assert_eq!(msg["type"], "section", "{id}: {msg}");
        sections.insert(id.into(), msg);
    }
    dump.insert("sections".into(), Value::Object(sections));
    dump.insert(
        "diagram".into(),
        send(&mut plugin, json!({"type": "diagram"}))
            .pop()
            .unwrap_or_default(),
    );
    let web = Path::new(env!("CARGO_MANIFEST_DIR")).join("web");
    let out = Path::new(&out);
    std::fs::create_dir_all(out.join("icons")).unwrap();
    std::fs::copy(web.join("hub.html"), out.join("hub.html")).unwrap();
    for icon in std::fs::read_dir(web.join("icons")).unwrap().flatten() {
        std::fs::copy(icon.path(), out.join("icons").join(icon.file_name())).unwrap();
    }
    std::fs::write(out.join("dump.json"), Value::Object(dump).to_string()).unwrap();
}
