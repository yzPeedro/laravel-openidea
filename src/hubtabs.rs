use crate::Laravel;
use crate::audit;
use crate::devtools;
use crate::php::{CallKind, StringArg};
use crate::runner;
use openidea_plugin::{Error, Project, host};
use serde_json::{Value, json};
use std::cell::RefCell;

pub const EDITOR: &str = "hub";
pub const DIR: &str = "storage/openidea/hub";
pub const INDEX: &str = "storage/openidea/hub/sections";
pub const WHOLE: &str = "hub";
const PENDING_SECONDS: u64 = 15;

pub const SECTIONS: [&str; 47] = [
    "overview",
    "ecosystem",
    "upgrade",
    "ai",
    "artisan",
    "tinker",
    "sql",
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
    "diagram",
    "cache",
    "factories",
    "views",
    "components",
    "frontend",
    "translations",
    "classes",
    "providers",
    "bindings",
    "rules",
    "security",
    "unused",
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
    "audit",
    "logs",
    "profiles",
    "debug-calls",
    "debug-tools",
];

const CACHE_KEY_METHODS: [&str; 16] = [
    "get",
    "has",
    "missing",
    "pull",
    "put",
    "set",
    "add",
    "forever",
    "forget",
    "remember",
    "rememberforever",
    "sear",
    "increment",
    "decrement",
    "flexible",
    "delete",
];

thread_local! {
    static INSTANCE: RefCell<Option<String>> = const { RefCell::new(None) };
}

pub fn with_instance<T>(instance: Option<String>, f: impl FnOnce() -> T) -> T {
    let previous = INSTANCE.with(|i| i.replace(instance));
    let out = f();
    INSTANCE.with(|i| *i.borrow_mut() = previous);
    out
}

pub fn tag(value: &mut Value) {
    if let Some(object) = value.as_object_mut()
        && !object.contains_key("instance")
        && let Some(instance) = INSTANCE.with(|i| i.borrow().clone())
    {
        object.insert("instance".into(), Value::String(instance));
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pending {
    pub section: String,
    pub search: String,
    pub at: u64,
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

pub fn file_name(section: &str) -> String {
    format!("{section}.laravel-hub")
}

pub fn file_path(section: &str) -> String {
    format!("{DIR}/{}", file_name(section))
}

pub fn section_of(path: &str) -> Option<&str> {
    let name = path.rsplit(['/', '\\']).next()?;
    let section = name.strip_suffix(".laravel-hub")?;
    if section == WHOLE {
        return Some("overview");
    }
    SECTIONS.contains(&section).then_some(section)
}

pub(crate) fn wanted() -> String {
    let mut all: Vec<&str> = SECTIONS.to_vec();
    all.push(WHOLE);
    all.join("\n") + "\n"
}

pub fn cache_key(arg: &StringArg) -> Option<&str> {
    if arg.arg != 0 || arg.in_array || arg.is_key || arg.value.is_empty() || arg.concat {
        return None;
    }
    let name = arg.callee.name.as_str();
    let keyed = CACHE_KEY_METHODS.contains(&name);
    let found = match arg.callee.kind {
        CallKind::Static => keyed && arg.callee.class.as_deref() == Some("cache"),
        CallKind::Function => name == "cache",
        CallKind::Method => keyed && arg.callee.class.as_deref() == Some("cache()"),
        _ => false,
    };
    found.then_some(arg.value.as_str())
}

pub fn navigate(section: &str, file: &str, search: Option<&str>) -> Value {
    let mut message = json!({ "type": "navigate", "id": section, "file": file });
    if let Some(search) = search.filter(|s| !s.is_empty()) {
        message["search"] = json!(search);
        if section == "cache" {
            message["cacheKey"] = json!(search);
        }
    }
    message
}

impl Laravel {
    pub(crate) fn sync_hub_files(&mut self, project: &Project, force: bool) -> Result<(), String> {
        let root = project.root().map_err(|e| e.to_string())?;
        let wanted = wanted();
        if project
            .read_file(INDEX)
            .is_ok_and(|current| current == wanted)
        {
            return Ok(());
        }
        if self.hub_synced.contains(&root) && !force {
            return Err("the last attempt in this session failed".into());
        }
        self.hub_synced.insert(root.clone());
        let mode = runner::detect(project);
        let _task = host::begin_task("Laravel: preparing the hub tabs", false);
        let run = mode.php_args(&[
            "-r".into(),
            devtools::script(devtools::INSTALL_PHP),
            "hub".into(),
        ]);
        match runner::run(project, &mode, &run, Some(&wanted)) {
            Ok(out) if out.status == Some(0) => {
                self.hub_synced.remove(&root);
                Ok(())
            }
            Ok(out) => {
                let why = audit::output(&out);
                audit::error("hub tabs", &why);
                Err(why)
            }
            Err(Error::Denied(cap)) => {
                let why = format!("allow {cap} for Laravel OpenIDEA in Settings › Plugins");
                audit::warn("hub tabs", why.clone());
                Err(why)
            }
            Err(e) => {
                audit::failed("hub tabs", &e);
                Err(e.to_string())
            }
        }
    }

    pub(crate) fn remember_hub_filter(&mut self, section: &str, search: &str) {
        self.hub_pending = Some(Pending {
            section: section.to_owned(),
            search: search.to_owned(),
            at: now(),
        });
    }

    pub(crate) fn take_hub_filter(&mut self, section: &str) -> Option<String> {
        let pending = self.hub_pending.take()?;
        if pending.section == section && now().saturating_sub(pending.at) <= PENDING_SECONDS {
            Some(pending.search)
        } else {
            None
        }
    }

    pub(crate) fn hub_tab_focused(&mut self, path: &str) {
        let Some(section) = section_of(path) else {
            return;
        };
        if let Some(search) = self.take_hub_filter(section) {
            let message = navigate(section, path, Some(&search));
            host::post_view_message(EDITOR, &message.to_string())
                .or_warn_quiet("posting a filter to the hub tab");
        }
    }
}

trait Quiet {
    fn or_warn_quiet(self, what: &str);
}

impl<E: std::fmt::Display> Quiet for Result<(), E> {
    fn or_warn_quiet(self, what: &str) {
        if let Err(e) = self {
            audit::warn(what, e.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_hub_files_to_sections() {
        assert_eq!(
            section_of("/p/storage/openidea/hub/cache.laravel-hub"),
            Some("cache")
        );
        assert_eq!(
            section_of("C:\\p\\storage\\openidea\\hub\\hub.laravel-hub"),
            Some("overview")
        );
        assert_eq!(section_of("/p/storage/openidea/hub/nope.laravel-hub"), None);
        assert_eq!(section_of("/p/app/Models/User.php"), None);
        assert!(wanted().contains("\ncache\n") && wanted().ends_with("hub\n"));
        assert_eq!(
            section_of("/p/storage/openidea/hub/model-User.laravel-hub"),
            None
        );
    }

    #[test]
    fn lists_every_section_of_the_hub_page() {
        let page = include_str!("../web/hub.html");
        let start = page.find("const SECTIONS = [").unwrap();
        let end = start + page[start..].find("];").unwrap();
        let ids: Vec<&str> = page[start..end]
            .split("id: \"")
            .skip(1)
            .filter_map(|rest| rest.split('"').next())
            .collect();
        assert_eq!(ids, SECTIONS);
    }

    #[test]
    fn tags_replies_with_the_requesting_instance() {
        let mut outside = json!({ "type": "section" });
        tag(&mut outside);
        assert!(outside.get("instance").is_none());
        let tagged = with_instance(Some("a1".into()), || {
            let mut value = json!({ "type": "section" });
            tag(&mut value);
            value
        });
        assert_eq!(tagged["instance"], "a1");
    }

    #[test]
    fn finds_cache_keys() {
        let key = |src: &str| {
            let arg = crate::php::arg_at(src, false, src.find("'k").unwrap() + 2)?;
            cache_key(&arg).map(str::to_owned)
        };
        assert_eq!(key("<?php\nCache::get('key');").as_deref(), Some("key"));
        assert_eq!(key("<?php\ncache('key');").as_deref(), Some("key"));
        assert_eq!(
            key("<?php\ncache()->remember('key', 60, fn () => 1);").as_deref(),
            Some("key")
        );
        assert_eq!(key("<?php\nCache::store('key');"), None);
        assert_eq!(key("<?php\nConfig::get('key');"), None);
    }
}
