use super::Entry;
use crate::project::{find, rel};
use openidea_plugin::{Location, Project, Range};

pub fn index(project: &Project, root: &str) -> Vec<Entry> {
    let mut out: Vec<Entry> = find(project, root, "resources/views/**/*.php", 10_000)
        .into_iter()
        .filter_map(|path| {
            let relative = rel(root, &path).replace('\\', "/");
            let key = name(&relative)?;
            Some(Entry {
                key,
                detail: Some(relative),
                location: Location {
                    path,
                    range: Range::default(),
                },
            })
        })
        .collect();
    out.sort_by(|a, b| a.key.cmp(&b.key));
    out
}

pub fn name(relative: &str) -> Option<String> {
    let inner = relative.strip_prefix("resources/views/")?;
    let stem = inner
        .strip_suffix(".blade.php")
        .or_else(|| inner.strip_suffix(".php"))?;
    Some(stem.replace('/', "."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_views_by_their_dotted_path() {
        assert_eq!(
            name("resources/views/users/index.blade.php").as_deref(),
            Some("users.index")
        );
        assert_eq!(
            name("resources/views/welcome.php").as_deref(),
            Some("welcome")
        );
        assert_eq!(name("app/Http/Kernel.php"), None);
    }
}
