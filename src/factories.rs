use crate::casts::{file_of, psr4, resolve_name};
use crate::objects;
use openidea_plugin::Project;

const NOT_STATES: [&str; 6] = [
    "definition",
    "configure",
    "newModel",
    "modelName",
    "new",
    "times",
];

fn declared_factory(model_text: &str) -> Option<String> {
    if let Some(at) = model_text.find("#[UseFactory(") {
        let rest = &model_text[at + "#[UseFactory(".len()..];
        let class = rest.split("::class").next()?.trim();
        if !class.is_empty() && !class.contains([' ', '(', ')']) {
            return Some(class.to_owned());
        }
    }
    let at = model_text.find("function newFactory")?;
    let body = &model_text[at..model_text.len().min(at + 600)];
    let (class, _) = body
        .split_once("::new(")
        .or_else(|| body.split_once("::new ("))?;
    let class = class
        .rsplit(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '\\'))
        .next()?;
    (!class.is_empty()).then(|| class.to_owned())
}

pub fn file(project: &Project, model: &str, model_text: &str) -> Option<(String, String)> {
    let composer = project.read_file("composer.json").unwrap_or_default();
    let map = psr4(&composer);
    if let Some(class) = declared_factory(model_text) {
        let fqn = resolve_name(model_text, &class);
        if let Some(found) = file_of(project, &map, &fqn) {
            return Some(found);
        }
    }
    let path = format!("database/factories/{model}Factory.php");
    if let Ok(text) = project.read_file(&path) {
        return Some((path, text));
    }
    let model_fqn = resolve_name(model_text, model);
    let nested = model_fqn
        .strip_prefix("App\\Models\\")
        .map(|rest| format!("Database\\Factories\\{rest}Factory"))?;
    file_of(project, &map, &nested)
}

pub fn states(factory_text: &str) -> Vec<(String, String)> {
    objects::read(factory_text)
        .map(|m| {
            m.methods
                .into_iter()
                .filter(|(name, _, is_static)| !is_static && !NOT_STATES.contains(&name.as_str()))
                .map(|(name, signature, _)| (name, format!("state{signature}")))
                .collect()
        })
        .unwrap_or_default()
}

pub fn class_offset(text: &str) -> usize {
    text.find("class ").map_or(0, |at| at + "class ".len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_declared_factory() {
        assert_eq!(
            declared_factory("<?php\n#[UseFactory(PostFactory::class)]\nclass Post {}").as_deref(),
            Some("PostFactory")
        );
        assert_eq!(
            declared_factory("<?php\nclass Post {\n    protected static function newFactory()\n    {\n        return \\Database\\Factories\\Blog\\PostFactory::new();\n    }\n}").as_deref(),
            Some("\\Database\\Factories\\Blog\\PostFactory")
        );
        assert_eq!(declared_factory("<?php\nclass Post {}"), None);
    }

    #[test]
    fn lists_states_but_not_the_definition() {
        let text = "<?php\nclass UserFactory extends Factory\n{\n    public function definition(): array { return []; }\n    public function unverified(): static { return $this; }\n    public function admin(bool $super = false): static { return $this; }\n    protected function hidden() {}\n}\n";
        let names: Vec<String> = states(text).into_iter().map(|(n, _)| n).collect();
        assert_eq!(names, ["unverified", "admin"]);
    }
}
