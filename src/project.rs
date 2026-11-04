use openidea_plugin::{Position, Project, Range, host};

pub fn project_of(path: &str) -> Option<Project> {
    host::projects()
        .ok()?
        .into_iter()
        .filter(|p| p.root().is_ok_and(|r| path.starts_with(&r)))
        .max_by_key(|p| p.root().map(|r| r.len()).unwrap_or(0))
}

pub fn rel<'a>(root: &str, path: &'a str) -> &'a str {
    path.strip_prefix(root)
        .map_or(path, |r| r.trim_start_matches(['/', '\\']))
}

pub fn abs(root: &str, path: &str) -> String {
    if path.starts_with(['/', '\\']) || path.get(1..2) == Some(":") || path.starts_with(root) {
        path.to_owned()
    } else {
        join(root, path)
    }
}

pub fn join(root: &str, relative: &str) -> String {
    let sep = if root.contains('\\') { '\\' } else { '/' };
    let relative = relative.replace(['/', '\\'], &sep.to_string());
    format!("{}{sep}{relative}", root.trim_end_matches(['/', '\\']))
}

pub fn find(project: &Project, root: &str, glob: &str, limit: u32) -> Vec<String> {
    project
        .find_files(glob, limit)
        .unwrap_or_default()
        .into_iter()
        .map(|p| abs(root, &p))
        .collect()
}

pub fn is_laravel(project: &Project) -> bool {
    project.read_file("artisan").is_ok()
        && project
            .read_file("composer.json")
            .is_ok_and(|c| c.contains("laravel/framework") || c.contains("laravel/laravel"))
}

pub fn laravel_version(project: &Project) -> Option<String> {
    let lock = project.read_file("composer.lock").ok()?;
    let at = lock.find("\"name\": \"laravel/framework\"")?;
    let rest = &lock[at..];
    let v = rest.find("\"version\": \"")? + 12;
    let end = rest[v..].find('"')? + v;
    Some(rest[v..end].trim_start_matches('v').to_owned())
}

pub struct Lines {
    starts: Vec<usize>,
}

impl Lines {
    pub fn new(text: &str) -> Self {
        let mut starts = vec![0];
        starts.extend(text.match_indices('\n').map(|(i, _)| i + 1));
        Self { starts }
    }

    pub fn position(&self, text: &str, at: usize) -> Position {
        let at = at.min(text.len());
        let line = self.starts.partition_point(|&s| s <= at) - 1;
        let start = self.starts[line];
        let character = text[start..at].chars().map(|c| c.len_utf16() as u32).sum();
        Position::new(line as u32, character)
    }

    pub fn range(&self, text: &str, start: usize, end: usize) -> Range {
        Range::new(self.position(text, start), self.position(text, end))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_offsets_to_utf16_positions() {
        let text = "ab\nçé𝄞x\n";
        let lines = Lines::new(text);
        assert_eq!(lines.position(text, 0), Position::new(0, 0));
        assert_eq!(lines.position(text, 3), Position::new(1, 0));
        let x = text.find('x').unwrap();
        assert_eq!(lines.position(text, x), Position::new(1, 4));
        assert_eq!(lines.position(text, text.len()), Position::new(2, 0));
    }

    #[test]
    fn joins_relative_paths() {
        assert_eq!(abs("/p", "config/app.php"), "/p/config/app.php");
        assert_eq!(abs("/p", "/p/config/app.php"), "/p/config/app.php");
        assert_eq!(rel("/p", "/p/routes/web.php"), "routes/web.php");
        let unc = r"\\?\UNC\wsl.localhost\Ubuntu\home\dev\app";
        assert_eq!(join(unc, ".env"), format!(r"{unc}\.env"));
        assert_eq!(abs(unc, "config/app.php"), format!(r"{unc}\config\app.php"));
        assert_eq!(
            rel(unc, &format!(r"{unc}\routes\web.php")),
            r"routes\web.php"
        );
    }
}
